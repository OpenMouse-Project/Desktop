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
// Real report-descriptor parsing isn't implemented here (the Rust side
// doesn't parse the HID report descriptor into collections) — `collections`
// stays empty. Callers must not rely on collections-based `isSupported()`
// gating (most driver classes' static `isSupported()` checks `collections`
// and will always return false against this adapter); construct the known
// driver class directly instead — see `src/native-hid/brands.ts`.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface HidInterfaceInfo {
  key: string;
  vendorId: number;
  productId: number;
  interfaceNumber: number;
  productString: string;
  manufacturerString: string;
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

export class TauriHidDevice implements HIDDevice {
  readonly vendorId: number;
  readonly productId: number;
  readonly productName: string;
  readonly collections: readonly HIDCollectionInfo[] = [];
  opened = false;

  private readonly interfaceNumber: number;
  private readonly listeners = new Set<(event: HIDInputReportEvent) => void>();
  private unlisten: UnlistenFn | null = null;

  constructor(info: HidInterfaceInfo) {
    this.vendorId = info.vendorId;
    this.productId = info.productId;
    this.interfaceNumber = info.interfaceNumber;
    this.productName = info.productString;
  }

  async open(): Promise<void> {
    if (this.opened) return;
    await invoke("hid_open", {
      vendorId: this.vendorId,
      productId: this.productId,
      interfaceNumber: this.interfaceNumber,
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
    await invoke("hid_close", {
      vendorId: this.vendorId,
      productId: this.productId,
      interfaceNumber: this.interfaceNumber,
    });
    this.opened = false;
  }

  async sendReport(reportId: number, data: BufferSource): Promise<void> {
    await invoke("hid_send_report", {
      vendorId: this.vendorId,
      productId: this.productId,
      interfaceNumber: this.interfaceNumber,
      reportId,
      data: Array.from(toBytes(data)),
    });
  }

  async sendFeatureReport(reportId: number, data: BufferSource): Promise<void> {
    await invoke("hid_send_feature_report", {
      vendorId: this.vendorId,
      productId: this.productId,
      interfaceNumber: this.interfaceNumber,
      reportId,
      data: Array.from(toBytes(data)),
    });
  }

  async receiveFeatureReport(reportId: number): Promise<DataView> {
    const bytes = await invoke<number[]>("hid_get_feature_report", {
      vendorId: this.vendorId,
      productId: this.productId,
      interfaceNumber: this.interfaceNumber,
      reportId,
      length: 64,
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
    return `${vendor}:${product}:${this.interfaceNumber}`;
  }
}
