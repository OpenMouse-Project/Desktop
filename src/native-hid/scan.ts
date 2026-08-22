// Scans every HID interface whose vendor id matches a known brand, and
// tries each brand's candidate driver classes against it in turn — mirrors
// `probe()`/`main()` in OpenMouse-Bridge's `native-hid/src/apply.mjs`, minus
// the Node subprocess: this runs directly in the Tauri webview against
// `TauriHidDevice`.
//
// `isSupported()` gating is skipped deliberately (see brands.ts) — a
// candidate is accepted once its `open()` and `readStatus()` both succeed
// within `PROBE_TIMEOUT_MS`, the same "does it actually answer" bar
// apply.mjs uses.

import type { MouseStatus } from "@openmouse/protocol/drivers/mouse-types";
import { allKnownVendorIds, candidatesForVendorId, type SupportedClient } from "./brands";
import { listHidInterfaces, TauriHidDevice, type HidInterfaceInfo } from "./tauri-hid-device";

const PROBE_TIMEOUT_MS = 3000;

export interface ConnectedDevice {
  brand: string;
  client: SupportedClient;
  device: TauriHidDevice;
  status: MouseStatus;
}

function withTimeout<T>(promise: Promise<T>, ms: number, label: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`${label} timed out after ${ms}ms`)), ms);
    promise.then(
      (value) => { clearTimeout(timer); resolve(value); },
      (error) => { clearTimeout(timer); reject(error); },
    );
  });
}

async function probe(
  info: HidInterfaceInfo,
  brand: string,
  candidateName: string,
  Client: new (device: HIDDevice) => SupportedClient,
): Promise<ConnectedDevice> {
  const device = new TauriHidDevice(info);
  const client = new Client(device);
  try {
    await withTimeout(client.open(), PROBE_TIMEOUT_MS, `${candidateName}.open()`);
    const status = await withTimeout(client.readStatus(), PROBE_TIMEOUT_MS, `${candidateName}.readStatus()`);
    return { brand, client, device, status };
  } catch (error) {
    await client.close().catch(() => undefined);
    throw error;
  }
}

/**
 * Tries every known brand's candidates against every matching HID
 * interface currently connected, and returns the first one that answers.
 * Resolves to `null` (not a rejection) when nothing does — that's the
 * expected "no supported mouse plugged in" case, not an error.
 */
export async function scanForDevice(): Promise<ConnectedDevice | null> {
  const interfaces = await listHidInterfaces(allKnownVendorIds());
  const attempts: string[] = [];
  for (const info of interfaces) {
    for (const candidate of candidatesForVendorId(info.vendorId)) {
      try {
        return await probe(info, candidate.brand, candidate.name, candidate.Client);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        attempts.push(`${candidate.name} on ${info.productString || "unknown device"}: ${message}`);
      }
    }
  }
  if (attempts.length > 0) {
    console.info(`[native-hid] no device answered. Tried:\n  ${attempts.join("\n  ")}`);
  }
  return null;
}
