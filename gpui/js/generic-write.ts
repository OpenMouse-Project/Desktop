// Calls one method on whichever driver answers a device, for the few writes
// src/native-hid/write.ts has no named setter for (it is the Tauri app's file, so
// it is not edited here). The session handling is the same as write.ts's
// `withClient`: the shared open lock with retry, each candidate driver in turn,
// and a few delayed re-runs when a wireless mouse had the command queued rather
// than processed.

import { candidatesForDevice } from "../../src/native-hid/brands";
import { TauriHidDevice, type HidInterfaceInfo } from "../../src/native-hid/tauri-hid-device";
import { isQueuedNotProcessedError, withHidOpenLockRetrying } from "../../src/native-hid/hid-open-lock";

const OPEN_TIMEOUT_MS = 10000;
const ACTION_TIMEOUT_MS = 15000;
const BUSY_RETRY_DELAY_MS = 500;
const BUSY_RETRIES = 6;

const delay = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

function withTimeout<T>(promise: Promise<T>, ms: number, label: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`${label} timed out after ${ms}ms`)), ms);
    promise.then(
      (value) => { clearTimeout(timer); resolve(value); },
      (error) => { clearTimeout(timer); reject(error); },
    );
  });
}

export function callClientMethod(info: HidInterfaceInfo, method: string, ...args: unknown[]): Promise<unknown> {
  if (info.key.startsWith("mock:")) return Promise.resolve(undefined);
  return withHidOpenLockRetrying(info.key, async () => {
    const candidates = candidatesForDevice(info);
    if (candidates.length === 0) throw new Error(`This device's driver doesn't support ${method}.`);
    const attempts: string[] = [];
    for (const candidate of candidates) {
      const client = candidate.create(new TauriHidDevice(info, candidate.preferredCollection)) as any;
      if (!client) continue;
      const name = client.constructor.name || candidate.brand;
      let message = "";
      for (let attempt = 0; attempt < BUSY_RETRIES; attempt += 1) {
        try {
          await withTimeout(client.open(), OPEN_TIMEOUT_MS, `${name}.open()`);
          if (typeof client[method] !== "function") throw new Error(`${method} not supported`);
          const result = await withTimeout(client[method](...args), ACTION_TIMEOUT_MS, `${name}.${method}`);
          await client.close().catch(() => undefined);
          return result;
        } catch (error) {
          await client.close().catch(() => undefined);
          message = error instanceof Error ? error.message : String(error);
          if (!isQueuedNotProcessedError(message)) break;
          if (attempt < BUSY_RETRIES - 1) await delay(BUSY_RETRY_DELAY_MS);
        }
      }
      attempts.push(`${name}: ${message}`);
    }
    throw new Error(`No driver answered ${method}. Tried:\n  ${attempts.join("\n  ")}`);
  });
}
