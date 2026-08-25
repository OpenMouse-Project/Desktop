// Tiny module-level toast store — a plain pub/sub, not context, matching
// this app's existing pattern of module-level state for things that need to
// survive/outlive individual component instances (see
// native-hid/hid-open-lock.ts). Any module can call `showToast()` without
// needing a hook or a provider wrapping it; `<ToastHost/>` (mounted once,
// see components/ToastHost.tsx) is the only thing that actually subscribes.

export type ToastKind = "success" | "error" | "info";

export interface ToastMessage {
  id: number;
  kind: ToastKind;
  text: string;
}

type Listener = (toasts: ToastMessage[]) => void;

let toasts: ToastMessage[] = [];
let nextId = 1;
const listeners = new Set<Listener>();

function emit() {
  for (const listener of listeners) listener(toasts);
}

export function showToast(text: string, kind: ToastKind = "info", durationMs = 4000): void {
  const id = nextId++;
  toasts = [...toasts, { id, kind, text }];
  emit();
  setTimeout(() => dismissToast(id), durationMs);
}

export function dismissToast(id: number): void {
  toasts = toasts.filter((toast) => toast.id !== id);
  emit();
}

/** Returns an unsubscribe function. Immediately calls `listener` with the current list. */
export function subscribeToasts(listener: Listener): () => void {
  listeners.add(listener);
  listener(toasts);
  return () => listeners.delete(listener);
}
