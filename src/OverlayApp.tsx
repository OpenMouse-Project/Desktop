import { useEffect, useRef, useState } from "preact/hooks";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, PhysicalPosition, PhysicalSize, primaryMonitor } from "@tauri-apps/api/window";
import { OVERLAY_MARGIN, OVERLAY_SIZE, getOverlaySettings, type OverlayCorner } from "./lib/overlay-settings";
import { OVERLAY_TOAST_EVENT, type OverlayToastPayload } from "./lib/overlay-toast";

const DISPLAY_MS = 4500;

/**
 * Where the top-left corner of the overlay window should land, given which
 * corner it should hug — against the monitor's WORK AREA, not its full
 * size. CONFIRMED the fix for a real bug: `Monitor.size`/`.position` cover
 * the entire physical screen, taskbar included, so a "bottom" corner
 * computed against it landed partly (or fully) underneath the Windows
 * taskbar instead of above it. `Monitor.workArea` is the region Windows
 * itself already excludes taskbars/docks from.
 */
function computePosition(corner: OverlayCorner, workAreaSize: { width: number; height: number }, workAreaPos: { x: number; y: number }): PhysicalPosition {
  const { width, height } = OVERLAY_SIZE;
  const left = workAreaPos.x + OVERLAY_MARGIN;
  const right = workAreaPos.x + workAreaSize.width - width - OVERLAY_MARGIN;
  const top = workAreaPos.y + OVERLAY_MARGIN;
  const bottom = workAreaPos.y + workAreaSize.height - height - OVERLAY_MARGIN;
  switch (corner) {
    case "top-left":
      return new PhysicalPosition(left, top);
    case "top-right":
      return new PhysicalPosition(right, top);
    case "bottom-left":
      return new PhysicalPosition(left, bottom);
    case "bottom-right":
    default:
      return new PhysicalPosition(right, bottom);
  }
}

/**
 * Runs inside the dedicated "overlay" window (tauri.conf.json), not the
 * main app window — main.tsx branches on the window label to render this
 * instead of <App/>. Starts invisible; every show resizes/repositions the
 * actual OS window to match whatever corner the user last chose in
 * Settings (lib/overlay-settings.ts) before revealing it, so a preference
 * change there takes effect on the very next toast without needing a
 * restart.
 */
export function OverlayApp() {
  const [toast, setToast] = useState<OverlayToastPayload | null>(null);
  const hideTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  useEffect(() => {
    const win = getCurrentWindow();

    async function present(payload: OverlayToastPayload) {
      const settings = getOverlaySettings();
      try {
        const monitor = await primaryMonitor();
        if (monitor) {
          await win.setSize(new PhysicalSize(OVERLAY_SIZE.width, OVERLAY_SIZE.height));
          await win.setPosition(computePosition(settings.corner, monitor.workArea.size, monitor.workArea.position));
        }
      } catch {
        // Best-effort — worst case it shows at whatever size/position it
        // last had rather than not showing at all.
      }
      setToast(payload);
      await win.show();

      if (hideTimerRef.current) clearTimeout(hideTimerRef.current);
      hideTimerRef.current = setTimeout(() => {
        setToast(null);
        void win.hide();
      }, DISPLAY_MS);
    }

    const unlisten = listen<OverlayToastPayload>(OVERLAY_TOAST_EVENT, (event) => {
      void present(event.payload);
    });

    return () => {
      void unlisten.then((fn) => fn());
      if (hideTimerRef.current) clearTimeout(hideTimerRef.current);
    };
  }, []);

  if (!toast) return null;

  return (
    <div class={`overlay-toast overlay-toast-${toast.kind}`}>
      <span class="overlay-toast-dot" aria-hidden="true" />
      <span class="overlay-toast-text">{toast.text}</span>
    </div>
  );
}
