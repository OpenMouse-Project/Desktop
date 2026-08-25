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
// within their own budgets below, the same "does it actually answer" bar
// apply.mjs uses.

import type { MouseStatus } from "@openmouse/protocol/drivers/mouse-types";
import { allKnownVendorIds, candidatesForVendorId } from "./brands";
import { listHidInterfaces, TauriHidDevice, type HidInterfaceInfo } from "./tauri-hid-device";
import { withHidOpenLock } from "./hid-open-lock";

// `open()` never touches the wire (see TauriHidDevice.open()/LogitechHidppClient.open())
// — it just claims the HID handle and registers a listener — so it has no
// business sharing a timeout sized for a multi-step protocol exchange.
// Kept short so a genuinely broken open() fails fast.
const OPEN_TIMEOUT_MS = 10000;

// `readStatus()` is a full protocol walk, and two independent costs stack
// inside it, confirmed live against real hardware:
//
// 1. Device-index discovery. For a known Logitech receiver,
//    `resolveDeviceIndex()` (mouse-protocol/src/drivers/logitech/hidpp.ts)
//    probes `hidppDeviceIndexCandidates()` in turn — 7 candidates (pairing
//    slots 0x01-0x06, then the direct index) — and on a non-Bolt receiver
//    each one that doesn't answer burns its own full `REQUEST_TIMEOUT_MS`
//    (6000ms), not the much shorter `BOLT_INDEX_PROBE_TIMEOUT_MS` (800ms)
//    Bolt gets. A receiver whose paired mouse sits on a late slot — or is
//    simply out of range — can legitimately need 7 * 6000ms = 42000ms
//    before `resolveDeviceIndex()` gives up. (A direct wired connection
//    only has 2 candidates, so this cost is small there.)
// 2. The feature walk itself, once resolved. `readStatus()` reads name,
//    firmware, battery, DPI, DPI capabilities, report rate, onboard
//    profiles, analog buttons, haptics, friendly name, hosts, wheel state,
//    mode status — 20-30+ separate request/response round trips on a
//    feature-rich mouse (a PRO X Superlight, live-tested here). Each one
//    still pays the same-vendor-id multi-split fallback in `try_each`
//    (src-tauri/src/hid.rs) before landing on the split that actually
//    answers.
//
// 8000ms, then 45000ms sharing this timeout with `open()` were both tried
// first and both cut `readStatus()` off mid-walk on real hardware: a
// Lightspeed receiver (needs #1's full budget) and a wired PRO X
// Superlight (needs #2's — confirmed making real progress, resolving the
// device name and reading DPI/report-rate data, but still not finished at
// 45000ms). Sized generously enough to cover both costs landing in the same
// connect attempt, not tuned to either one alone.
const READ_STATUS_TIMEOUT_MS = 120000;

export interface CandidateInterface {
  info: HidInterfaceInfo;
  /** Brand(s) whose driver(s) might answer on this interface. */
  brands: string[];
}

export interface ConnectedDevice {
  /** The interface this snapshot came from — `HidInterfaceInfo.key` (stable `vendorId:productId`). */
  key: string;
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
  return withHidOpenLock(info.key, () => connectToInterfaceLocked(info));
}

async function connectToInterfaceLocked(info: HidInterfaceInfo): Promise<ConnectedDevice> {
  const attempts: string[] = [];
  for (const candidate of candidatesForVendorId(info.vendorId, info.productId)) {
    const device = new TauriHidDevice(info);
    const client = new candidate.Client(device);
    try {
      await withTimeout(client.open(), OPEN_TIMEOUT_MS, `${candidate.name}.open()`);
      const status = await withTimeout(client.readStatus(), READ_STATUS_TIMEOUT_MS, `${candidate.name}.readStatus()`);
      await client.close().catch(() => undefined);
      return { key: info.key, brand: candidate.brand, status };
    } catch (error) {
      await client.close().catch(() => undefined);
      const message = error instanceof Error ? error.message : String(error);
      attempts.push(`${candidate.name}: ${message}`);
    }
  }
  throw new Error(`No driver answered on this interface. Tried:\n  ${attempts.join("\n  ")}`);
}
