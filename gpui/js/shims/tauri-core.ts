// Stands in for `@tauri-apps/api/core` inside the GPUI app's embedded JS
// runtime, so `src/native-hid/*` runs there unmodified. `invoke` is the same
// command surface the Tauri app exposes (hid_list_interfaces, hid_open, ...),
// served by the Rust side instead of a webview.

declare const __host: { invoke(command: string, argsJson: string): Promise<string> };

export async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const reply = await __host.invoke(command, JSON.stringify(args ?? {}));
  return (reply === "" ? undefined : JSON.parse(reply)) as T;
}
