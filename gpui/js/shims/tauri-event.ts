// Stands in for `@tauri-apps/api/event`. Rust pushes events (the HID reader
// threads' `hid-input-report`) through `globalThis.__dispatch`.

export type UnlistenFn = () => void;

interface TauriEvent<T> {
  event: string;
  payload: T;
}

const listeners = new Map<string, Set<(event: TauriEvent<any>) => void>>();

(globalThis as any).__dispatch = (event: string, payloadJson: string): void => {
  const set = listeners.get(event);
  if (!set) return;
  const payload = JSON.parse(payloadJson);
  for (const listener of [...set]) listener({ event, payload });
};

export async function listen<T>(
  event: string,
  handler: (event: TauriEvent<T>) => void,
): Promise<UnlistenFn> {
  let set = listeners.get(event);
  if (!set) listeners.set(event, (set = new Set()));
  set.add(handler);
  return () => {
    set!.delete(handler);
  };
}
