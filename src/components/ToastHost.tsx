import { useEffect, useState } from "preact/hooks";
import { CheckCircle2, Info, X, XCircle } from "lucide-preact";
import { dismissToast, subscribeToasts, type ToastMessage } from "../lib/toast";

const ICONS = { success: CheckCircle2, error: XCircle, info: Info };

/** Mount once near the app root — every module calls `showToast()` directly, no provider needed. */
export function ToastHost() {
  const [toasts, setToasts] = useState<ToastMessage[]>([]);

  useEffect(() => subscribeToasts(setToasts), []);

  if (toasts.length === 0) return null;

  return (
    <div class="toast-host">
      {toasts.map((toast) => {
        const Icon = ICONS[toast.kind];
        return (
          <div class={`toast toast-${toast.kind}`} key={toast.id}>
            <Icon class="toast-icon" size={16} aria-hidden="true" />
            <span class="toast-text">{toast.text}</span>
            <button class="toast-dismiss" onClick={() => dismissToast(toast.id)} aria-label="Dismiss">
              <X size={13} />
            </button>
          </div>
        );
      })}
    </div>
  );
}
