// Razer-specific write actions for the device-control tabs.
//
// Mirrors logitech-actions.ts's pattern: every write opens a fresh
// connection, does exactly one thing, and closes again immediately.
// Holding a HID connection open with a live reader froze the device's
// own input on real hardware (confirmed on multiple brands).

import { RazerViperMiniHidClient } from "@openmouse/protocol/drivers/razer/viper-mini-hid";
import type { MouseLighting } from "@openmouse/protocol/drivers/mouse-types";
import { TauriHidDevice, type HidInterfaceInfo } from "./tauri-hid-device";
import { withHidOpenLockRetrying } from "./hid-open-lock";

const OPEN_TIMEOUT_MS = 10000;
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

/** The subset of `RazerViperMiniHidClient` this file actually calls. */
export interface WritableRazerClient {
  open(): Promise<void>;
  close(): Promise<void>;
  setDpi(dpi: number, dpiY?: number): Promise<number>;
  setPollingRate(pollingRateHz: number): Promise<number>;
  setLighting(lighting: MouseLighting): Promise<MouseLighting>;
}

const Client = RazerViperMiniHidClient as unknown as new (device: HIDDevice) => WritableRazerClient;

async function withClient<T>(
  info: HidInterfaceInfo,
  label: string,
  action: (client: WritableRazerClient) => Promise<T>,
): Promise<T> {
  return withHidOpenLockRetrying(info.key, () => withClientLocked(info, label, action));
}

async function withClientLocked<T>(
  info: HidInterfaceInfo,
  label: string,
  action: (client: WritableRazerClient) => Promise<T>,
): Promise<T> {
  const device = new TauriHidDevice(info);
  const client = new Client(device);
  try {
    await withTimeout(client.open(), OPEN_TIMEOUT_MS, "RazerViperMiniHidClient.open()");
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

export const setLighting = (info: HidInterfaceInfo, lighting: MouseLighting): Promise<MouseLighting> =>
  withClient(info, "setLighting", (client) => client.setLighting(lighting));
