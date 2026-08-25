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
