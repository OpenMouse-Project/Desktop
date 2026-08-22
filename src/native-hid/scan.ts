// Two separate operations, deliberately kept apart:
//
// - `listCandidateInterfaces()` only enumerates HID interfaces (a plain
//   hidapi device-list refresh) — it never opens anything, so it's safe to
//   call freely and never risks the mouse-freeze bug a previous version of
//   this file had from opening devices during a blind auto-scan.
// - `connectToInterface()` actually opens ONE specific interface the user
//   picked and tries each of its candidate driver classes in turn — mirrors
//   `probe()`/`main()` in OpenMouse-Bridge's `native-hid/src/apply.mjs`,
//   minus the Node subprocess: this runs directly in the Tauri webview
//   against `TauriHidDevice`.
//
// `isSupported()` gating is skipped deliberately (see brands.ts) — a
// candidate is accepted once its `open()` and `readStatus()` both succeed
// within `PROBE_TIMEOUT_MS`, the same "does it actually answer" bar
// apply.mjs uses.

import type { MouseStatus } from "@openmouse/protocol/drivers/mouse-types";
import { allKnownVendorIds, candidatesForVendorId } from "./brands";
import { listHidInterfaces, TauriHidDevice, type HidInterfaceInfo } from "./tauri-hid-device";

// Generous: Logitech's HID++ driver alone can legitimately take several
// seconds per candidate. `resolveDeviceIndex()` probes every receiver
// pairing slot in turn (a Bolt/Unifying receiver can report several paired
// devices), and each slot's own exchange timeout
// (`REQUEST_TIMEOUT_MS`/`BOLT_INDEX_PROBE_TIMEOUT_MS` in
// mouse-protocol/src/drivers/logitech/hidpp.ts) is 6000ms on its own — a
// receiver with a few empty/foreign slots can burn through several of those
// before reaching the right one. A shorter probe timeout here (3000ms was
// tried first) makes `readStatus()` get cut off mid-probe and reads as "no
// device", not as a real rejection — Logitech mice went undetected because
// of this, not because the driver itself failed.
const PROBE_TIMEOUT_MS = 8000;

export interface CandidateInterface {
  info: HidInterfaceInfo;
  /** Brand(s) whose driver(s) might answer on this interface. */
  brands: string[];
}

export interface ConnectedDevice {
  brand: string;
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

/**
 * Every currently-connected HID interface that at least one known brand's
 * driver might answer on. Read-only — nothing here opens a device.
 */
export async function listCandidateInterfaces(): Promise<CandidateInterface[]> {
  const interfaces = await listHidInterfaces(allKnownVendorIds());
  return interfaces
    .map((info) => ({
      info,
      brands: [...new Set(candidatesForVendorId(info.vendorId, info.productId).map((candidate) => candidate.brand))],
    }))
    .filter((candidate) => candidate.brands.length > 0);
}

/**
 * Opens one specific interface (as picked from `listCandidateInterfaces()`),
 * tries each of its candidate driver classes in turn until one answers, and
 * closes it again immediately after reading its status once — this never
 * stays open in the background. Reproduced on real hardware: leaving a
 * device open with a continuously-running background reader (needed for
 * request/response protocols that reply via input reports, not feature
 * reports) froze the device's own input for as long as the connection
 * stayed open, on more than one brand/collection — not tied to a specific
 * usage page after all. Read once, release immediately, and re-open only
 * for an explicit user action (a manual refresh, or eventually a write)
 * until that's understood well enough to hold a connection open safely.
 * Throws with the attempted candidates' errors when none answer.
 */
export async function connectToInterface(info: HidInterfaceInfo): Promise<ConnectedDevice> {
  const attempts: string[] = [];
  for (const candidate of candidatesForVendorId(info.vendorId, info.productId)) {
    const device = new TauriHidDevice(info);
    const client = new candidate.Client(device);
    try {
      await withTimeout(client.open(), PROBE_TIMEOUT_MS, `${candidate.name}.open()`);
      const status = await withTimeout(client.readStatus(), PROBE_TIMEOUT_MS, `${candidate.name}.readStatus()`);
      await client.close().catch(() => undefined);
      return { brand: candidate.brand, status };
    } catch (error) {
      await client.close().catch(() => undefined);
      const message = error instanceof Error ? error.message : String(error);
      attempts.push(`${candidate.name}: ${message}`);
    }
  }
  throw new Error(`No driver answered on this interface. Tried:\n  ${attempts.join("\n  ")}`);
}
