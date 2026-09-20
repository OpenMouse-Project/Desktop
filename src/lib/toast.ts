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
  /** Renders an indeterminate progress bar: the toast reports work in
   *  progress rather than a finished event, and does not auto-dismiss. */
  loading?: boolean;
}

type Listener = (toasts: ToastMessage[]) => void;

let toasts: ToastMessage[] = [];
let nextId = 1;
const listeners = new Set<Listener>();

function emit() {
  for (const listener of listeners) listener(toasts);
}

/** Returns the toast's id, so it can be updated or dismissed later. */
export function showToast(text: string, kind: ToastKind = "info", durationMs = 4000): number {
  const id = nextId++;
  toasts = [...toasts, { id, kind, text }];
  emit();
  setTimeout(() => dismissToast(id), durationMs);
  return id;
}

/**
 * A toast for work in progress: no auto-dismiss, and a progress bar. Used by
 * the connect path, where a device that needs a few seconds of retries used to
 * look like a click that did nothing — which is what made people click again.
 */
export function showProgressToast(text: string): number {
  const id = nextId++;
  toasts = [...toasts, { id, kind: "info", text, loading: true }];
  emit();
  return id;
}

/**
 * Rewrites a toast in place — for one that reports a long-running action and
 * then its outcome. Dismissing and re-adding would flicker, and re-animate the
 * toast's own entrance.
 */
export function updateToast(
  id: number | null,
  patch: { text?: string; kind?: ToastKind; loading?: boolean; durationMs?: number },
): void {
  if (id === null) return;
  toasts = toasts.map((toast) => (toast.id === id ? { ...toast, ...patch } : toast));
  emit();
  const duration = patch.durationMs;
  if (duration !== undefined) setTimeout(() => dismissToast(id), duration);
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
