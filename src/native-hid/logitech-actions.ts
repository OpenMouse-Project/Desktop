// Logitech-specific write actions for the device-control tabs.
//
// Read status is generic across brands (native-hid/scan.ts, brands.ts) since
// every driver's `SupportedClient` shape only declares open/close/
// readStatus — but actually commanding a device (DPI, polling rate,
// lift-off distance, ...) is brand- and feature-specific, with a different
// method set per driver class. Rather than force a shared write interface
// across every brand before any of them has a UI that needs one, this goes
// straight at `LogitechHidppClient`; other brands get their own module here
// once their tabs exist.
//
// Mirrors `connectToInterface()`'s own open -> act -> close pattern (see
// that function's docs in scan.ts for why): holding a HID++ connection open
// with a live reader thread froze the device's own input on real hardware,
// so every write here opens fresh, does exactly one thing, and closes again
// immediately — same as a status read, never a persistent connection.

import { LogitechHidppClient } from "@openmouse/protocol/drivers/logitech/hidpp";
import type { MouseStatus } from "@openmouse/protocol/drivers/mouse-types";
import { TauriHidDevice, type HidInterfaceInfo } from "./tauri-hid-device";
import { withHidOpenLock } from "./hid-open-lock";

const OPEN_TIMEOUT_MS = 10000;
// resolveDeviceIndex() alone can cost as much as scan.ts's
// READ_STATUS_TIMEOUT_MS documents for the full readStatus() walk — it's a
// receiver's own pairing-slot probe (up to 7 candidates * 6000ms), not a
// cost specific to reading every feature. It's a strict subset of that walk,
// so it can never exceed this budget.
const RESOLVE_INDEX_TIMEOUT_MS = 45000;
// A single write, once the index is resolved, is one request/response round
// trip, not the 20-30 of a full readStatus() walk — generous, but far short
// of that budget.
const ACTION_TIMEOUT_MS = 15000;

function withTimeout<T>(promise: Promise<T>, ms: number, label: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`${label} timed out after ${ms}ms`)), ms);
    promise.then(
      (value) => { clearTimeout(timer); resolve(value); },
      (error) => { clearTimeout(timer); reject(error); },
    );
  });
}

export type LiftOffDistance = NonNullable<MouseStatus["liftOffDistance"]>;
export type GamingSurfaceMode = NonNullable<MouseStatus["gamingSurfaceMode"]>;

/**
 * The subset of `LogitechHidppClient` this file actually calls. `open()` and
 * `resolveDeviceIndex()` are marked private in the class's own `.d.ts`
 * (nothing inside mouse-protocol calls them externally) even though every
 * runtime build implements them — `apply.mjs` (plain JS, no compile-time
 * privacy) already calls `open()` this same way, and `brands.ts` casts
 * through `unknown` for the same reason on the read-only path. Doing it once
 * here, not per call site.
 */
export interface WritableLogitechClient {
  open(): Promise<void>;
  close(): Promise<void>;
  /**
   * Every setter reads `this.deviceIndex`, which falls back to the DIRECT
   * index (0xFF) until this has actually run — `readStatus()` is the only
   * public method that calls it on its own. A mouse reached through a
   * receiver answers on a pairing slot, not 0xFF, so skipping this would
   * make a write on a receiver-paired mouse silently target the wrong HID++
   * index (works by coincidence when directly wired, since 0xFF is correct
   * there — exactly the kind of bug that stays invisible until tested
   * wireless). CONFIRMED by reading hidpp.js: readStatus() calls this with
   * no arguments before anything else; every write here does the same.
   */
  resolveDeviceIndex(): Promise<void>;
  setDpi(dpi: number, dpiY?: number): Promise<number>;
  setPollingRate(pollingRateHz: number): Promise<number>;
  setLiftOffDistance(liftOffDistance: LiftOffDistance): Promise<LiftOffDistance>;
  setGamingSurfaceMode(mode: GamingSurfaceMode): Promise<GamingSurfaceMode>;
}

const Client = LogitechHidppClient as unknown as new (device: HIDDevice) => WritableLogitechClient;

async function withClient<T>(
  info: HidInterfaceInfo,
  label: string,
  action: (client: WritableLogitechClient) => Promise<T>,
): Promise<T> {
  // Shared with scan.ts's connectToInterface() — see hid-open-lock.ts. A
  // write racing a concurrent status read (or another write) on the same
  // device would otherwise share hid_open's idempotent-per-device group and
  // get its reply cross-matched with the other call's.
  return withHidOpenLock(info.key, () => withClientLocked(info, label, action));
}

/**
 * Public version of `withClient`, for a caller that needs to run more than
 * one action against a single open()+resolveDeviceIndex() session — e.g.
 * lib/game-profiles.ts applying a profile's DPI and polling rate together.
 * Calling `setDpi()` then `setPollingRate()` back-to-back would pay that
 * open/resolve cost (up to RESOLVE_INDEX_TIMEOUT_MS for a receiver's own
 * pairing-slot probe) twice and double the window for colliding with the
 * background status auto-refresh's own hid-open-lock — noticeably worse
 * for something meant to happen instantly the moment a game launches.
 */
export const withLogitechClient = withClient;

async function withClientLocked<T>(
  info: HidInterfaceInfo,
  label: string,
  action: (client: WritableLogitechClient) => Promise<T>,
): Promise<T> {
  const device = new TauriHidDevice(info);
  const client = new Client(device);
  try {
    await withTimeout(client.open(), OPEN_TIMEOUT_MS, "LogitechHidppClient.open()");
    await withTimeout(client.resolveDeviceIndex(), RESOLVE_INDEX_TIMEOUT_MS, "LogitechHidppClient.resolveDeviceIndex()");
    return await withTimeout(action(client), ACTION_TIMEOUT_MS, label);
  } finally {
    await client.close().catch(() => undefined);
  }
}

/** Returns the DPI the mouse actually applied — the device can clamp/round a requested value. */
export const setDpi = (info: HidInterfaceInfo, dpi: number, dpiY?: number): Promise<number> =>
  withClient(info, "setDpi", (client) => client.setDpi(dpi, dpiY));

export const setPollingRate = (info: HidInterfaceInfo, pollingRateHz: number): Promise<number> =>
  withClient(info, "setPollingRate", (client) => client.setPollingRate(pollingRateHz));

export const setLiftOffDistance = (info: HidInterfaceInfo, liftOffDistance: LiftOffDistance): Promise<LiftOffDistance> =>
  withClient(info, "setLiftOffDistance", (client) => client.setLiftOffDistance(liftOffDistance));

export const setGamingSurfaceMode = (info: HidInterfaceInfo, mode: GamingSurfaceMode): Promise<GamingSurfaceMode> =>
  withClient(info, "setGamingSurfaceMode", (client) => client.setGamingSurfaceMode(mode));
