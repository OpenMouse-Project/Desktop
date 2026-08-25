// User-configurable corner for the always-on-top game-switch toast window
// (src/OverlayApp.tsx). Plain localStorage, matching this app's other
// small preference stores (native-hid/device-store.ts) — and specifically
// NOT scoped to one window: Settings (running in "main") saves here,
// OverlayApp (running in "overlay") reads it on every show. Both windows
// load the same origin, so localStorage is already shared between them
// without any IPC needed.
//
// Size used to be user-configurable too (Small/Medium/Large); removed on
// request in favor of always using what "Large" was — one less decision,
// and the single-line pill (OverlayToastPayload's own docs) barely changed
// height between presets anyway, so size was really only ever about width.

export type OverlayCorner = "top-left" | "top-right" | "bottom-left" | "bottom-right";

export interface OverlaySettings {
  corner: OverlayCorner;
}

const STORAGE_KEY = "openmouse:overlay-settings";

const DEFAULT_SETTINGS: OverlaySettings = { corner: "bottom-right" };

/** Display labels for the Settings picker — also the source of truth for which corner values are valid. */
export const CORNER_LABELS: Record<OverlayCorner, string> = {
  "top-left": "Top left",
  "top-right": "Top right",
  "bottom-left": "Bottom left",
  "bottom-right": "Bottom right",
};

/** Physical pixel size of the overlay window — OverlayApp resizes the actual OS window to match. */
export const OVERLAY_SIZE = { width: 420, height: 58 };

/** Gap from the screen edge, in physical pixels — same for every corner. */
export const OVERLAY_MARGIN = 24;

export function getOverlaySettings(): OverlaySettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULT_SETTINGS;
    const parsed = JSON.parse(raw) as Partial<OverlaySettings>;
    return {
      corner: parsed.corner && parsed.corner in CORNER_LABELS ? parsed.corner : DEFAULT_SETTINGS.corner,
    };
  } catch {
    // Private-browsing-style storage blocks, corrupted JSON, whatever —
    // this is a convenience, not a source of truth, so just fall back.
    return DEFAULT_SETTINGS;
  }
}

export function saveOverlaySettings(settings: OverlaySettings): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  } catch {
    // Best-effort — losing a saved preference just means it resets to default.
  }
}
