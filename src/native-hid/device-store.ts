// Remembers the last device that actually answered `connectToInterface()`,
// so the next launch can reconnect to it automatically instead of making the
// user find it in the list and click Connect again. Nothing here talks to
// the wire — this is just a name tag persisted in the webview's own
// localStorage, matched back against a fresh `listCandidateInterfaces()`
// scan by `HidInterfaceInfo.key` (a stable `vendorId:productId` string —
// see hid.rs's `interface_key()` — so it survives being plugged into a
// different USB port or restarting the app).

import type { HidInterfaceInfo } from "./tauri-hid-device";

const STORAGE_KEY = "openmouse:last-device";
/**
 * `vendorId:productId` → the name the device reported about itself once it
 * answered. A receiver enumerates as itself ("USB Receiver", `046d:c54d`)
 * and only reveals the mouse behind it — "PRO X SUPERLIGHT 2c", with its own
 * artwork — after a connect reads it over HID++. The list reads this so a
 * device that has ever been connected shows what it actually is, instead of
 * the dongle it arrives as. Nothing here opens anything: see scan.ts's own
 * rule about never touching a device during a plain enumeration.
 */
const NAMES_KEY = "openmouse:device-names";

export interface RememberedDevice {
  key: string;
  vendorId: number;
  productId: number;
  productString: string;
  brand: string;
}

/** The last device a connect actually succeeded on, if any. */
export function getRememberedDevice(): RememberedDevice | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    return typeof parsed?.key === "string" ? (parsed as RememberedDevice) : null;
  } catch {
    // Private-browsing-style storage blocks, corrupted JSON, whatever — this
    // is a convenience, not a source of truth, so just skip auto-reconnect.
    return null;
  }
}

/** What this interface turned out to be the last time it answered, if ever. */
export function getDeviceName(key: string): string | null {
  try {
    const raw = localStorage.getItem(NAMES_KEY);
    if (!raw) return null;
    const names = JSON.parse(raw) as Record<string, unknown>;
    const name = names?.[key];
    return typeof name === "string" && name.length > 0 ? name : null;
  } catch {
    return null;
  }
}

/** Records the self-reported name for `key` — a receiver's real mouse. */
export function rememberDeviceName(key: string, name: string): void {
  if (!key || !name) return;
  try {
    const raw = localStorage.getItem(NAMES_KEY);
    const names = raw ? (JSON.parse(raw) as Record<string, string>) : {};
    if (names[key] === name) return;
    names[key] = name;
    localStorage.setItem(NAMES_KEY, JSON.stringify(names));
  } catch {
    // Best-effort, same as rememberDevice: losing this only means the list
    // shows the interface's own name until the next connect.
  }
}

export function rememberDevice(info: HidInterfaceInfo, brand: string): void {
  try {
    const record: RememberedDevice = {
      key: info.key,
      vendorId: info.vendorId,
      productId: info.productId,
      productString: info.productString,
      brand,
    };
    localStorage.setItem(STORAGE_KEY, JSON.stringify(record));
  } catch {
    // Best-effort — losing the remembered device just means the next
    // launch falls back to manual connect.
  }
}
