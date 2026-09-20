// Wraps Tauri's native `hid_*` commands (src-tauri/src/hid.rs) so together
// they satisfy the WebHID `HIDDevice` interface mouse-protocol's driver
// classes are written against (see
// node_modules/@openmouse/protocol/dist/drivers/webhid.d.ts, an ambient-type
// declaration only — this file is the runtime implementation of that same
// shape for a Tauri webview).
//
// This lets Full Desktop Mode reuse OpenMouse's actual, hardware-verified
// driver classes unmodified: the drivers only ever call the methods below,
// never touch Tauri directly, and don't know they're not really in a
// browser. It mirrors OpenMouse-Bridge's
// `native-hid/src/hid-device-adapter.mjs`, which does the same job for a
// Node host instead of a webview, backed by node-hid instead of Tauri's
// Rust `hidapi` commands.
//
// `collections` is real: src-tauri/src/hid_descriptor.rs parses each
// interface's HID report descriptor and hid.rs merges the group's collections
// into `HidInterface.collections`, in WebHID's own shape. That matters because
// the protocol library's drivers gate on it — their static `isSupported(device)`
// reads `device.collections` — so `@openmouse/protocol/drivers/registry`'s own
// `DEVICE_DRIVERS` can match devices here exactly as it does in a browser,
// instead of the app carrying a hand-copied registry.
//
// One device here == every HID collection sharing a (vendor id, product
// id) pair, opened together (see hid.rs's module docs for why this is
// keyed on vendor+product alone, not also an interface number).

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface HidInterfaceInfo {
  key: string;
  vendorId: number;
  productId: number;
  productString: string;
  manufacturerString: string;
  /**
   * The interface group's HID collections and the report ids they declare,
   * parsed from each split's report descriptor by the Rust side. WebHID's own
   * shape, because the protocol drivers' `isSupported(device)` reads exactly
   * this to decide whether they can drive the device.
   */
  collections: HIDCollectionInfo[];
}

interface HidInputReportPayload {
  key: string;
  reportId: number;
  data: number[];
}

function toBytes(data: BufferSource): Uint8Array {
  return data instanceof ArrayBuffer
    ? new Uint8Array(data)
    : new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
}

export async function listHidInterfaces(vendorIds: number[]): Promise<HidInterfaceInfo[]> {
  return await invoke<HidInterfaceInfo[]>("hid_list_interfaces", { vendorIds });
}

/**
 * The HID collection a brand's driver actually talks on, when it is a
 * *standard* one. The bridge opens every collection of a (vendor, product)
 * pair as one group and tries them in a fixed order, which is wrong for a
 * device whose control channel is an ordinary collection — Razer's is
 * "the interface whose only collection is Generic Desktop Mouse" per its
 * driver's own docs. Without this the request goes to whichever collection
 * ranks first (the consumer-control one, on a DeathAdder V3 HyperSpeed), which
 * accepts the write, answers an all-zero packet, and pins the route there.
 */
export interface PreferredCollection {
  usagePage: number;
  usage: number;
}

export class TauriHidDevice implements HIDDevice {
  readonly vendorId: number;
  readonly productId: number;
  readonly productName: string;
  readonly collections: readonly HIDCollectionInfo[];
  opened = false;

  private readonly listeners = new Set<(event: HIDInputReportEvent) => void>();
  private readonly preferred?: PreferredCollection;
  private unlisten: UnlistenFn | null = null;

  constructor(info: HidInterfaceInfo, preferred?: PreferredCollection) {
    this.vendorId = info.vendorId;
    this.productId = info.productId;
    this.productName = info.productString;
    this.collections = info.collections ?? [];
    this.preferred = preferred;
  }

  async open(): Promise<void> {
    if (this.opened) return;
    await invoke("hid_open", {
      vendorId: this.vendorId,
      productId: this.productId,
      preferredUsagePage: this.preferred?.usagePage,
      preferredUsage: this.preferred?.usage,
    });
    this.unlisten = await listen<HidInputReportPayload>("hid-input-report", (event) => {
      if (event.payload.key !== this.key()) return;
      const bytes = Uint8Array.from(event.payload.data);
      // HIDInputReportEvent extends DOM's Event in the ambient WebHID types,
      // but no driver here reads Event's own fields (bubbles, target, ...) —
      // only device/reportId/data — so a real Event is not worth
      // constructing. Cast through `unknown` to skip that structural check.
      const report = {
        device: this,
        reportId: event.payload.reportId,
        data: new DataView(bytes.buffer),
      } as unknown as HIDInputReportEvent;
      for (const listener of this.listeners) listener(report);
    });
    this.opened = true;
  }

  async close(): Promise<void> {
    if (!this.opened) return;
    this.unlisten?.();
    this.unlisten = null;
    await invoke("hid_close", { vendorId: this.vendorId, productId: this.productId });
    this.opened = false;
  }

  async sendReport(reportId: number, data: BufferSource): Promise<void> {
    await invoke("hid_send_report", {
      vendorId: this.vendorId,
      productId: this.productId,
      reportId,
      data: Array.from(toBytes(data)),
    });
  }

  async sendFeatureReport(reportId: number, data: BufferSource): Promise<void> {
    await invoke("hid_send_feature_report", {
      vendorId: this.vendorId,
      productId: this.productId,
      reportId,
      data: Array.from(toBytes(data)),
    });
  }

  async receiveFeatureReport(reportId: number): Promise<DataView> {
    // WebHID returns whatever the device sends regardless of the HID
    // descriptor.  Razer's protocol uses 90-byte feature reports that are
    // *not* declared in the descriptor (see @openmouse/protocol's razer/codec),
    // so the buffer must be large enough to hold the full response.  90 bytes
    // covers every known vendor that uses undocumented feature reports; other
    // brands' shorter reports simply return fewer bytes, which is fine.
    const bytes = await invoke<number[]>("hid_get_feature_report", {
      vendorId: this.vendorId,
      productId: this.productId,
      reportId,
      length: 90,
    });
    return new DataView(Uint8Array.from(bytes).buffer);
  }

  async receiveInputReport(reportId: number): Promise<DataView> {
    const isWindows = navigator.userAgent.includes("Win") || navigator.platform.includes("Win");
    if (!isWindows) {
      // The Intellimouse firmware is quirky: it responds to GET_REPORT(Feature) 
      // but not GET_REPORT(Input) over the control pipe.
      // On Windows, get_input_report works because Windows caches the interrupt response.
      // On Linux/macOS, get_input_report sends a real GET_REPORT(Input) and returns all 0s.
      // Fortunately, Linux/macOS don't block get_feature_report, so we just use that.
      return this.receiveFeatureReport(reportId);
    }
    const bytes = await invoke<number[]>("hid_get_input_report", {
      vendorId: this.vendorId,
      productId: this.productId,
      reportId,
      length: 90,
    });
    return new DataView(Uint8Array.from(bytes).buffer);
  }

  addEventListener(type: "inputreport", listener: (event: HIDInputReportEvent) => void): void {
    if (type !== "inputreport") return;
    this.listeners.add(listener);
  }

  removeEventListener(type: "inputreport", listener: (event: HIDInputReportEvent) => void): void {
    if (type !== "inputreport") return;
    this.listeners.delete(listener);
  }

  private key(): string {
    const vendor = this.vendorId.toString(16).padStart(4, "0");
    const product = this.productId.toString(16).padStart(4, "0");
    return `${vendor}:${product}`;
  }
}
