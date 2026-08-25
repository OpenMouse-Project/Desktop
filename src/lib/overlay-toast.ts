// Sends a toast to the always-on-top overlay window (src/OverlayApp.tsx,
// tauri.conf.json's "overlay" window) instead of — or alongside — this
// app's own in-window toast (lib/toast.ts). For events that matter even
// while this window isn't visible at all, which is the normal case while
// actually playing a game (use-game-watcher.ts's apply/restore): the
// in-window toast only ever renders inside this app's own window, exactly
// the one place nobody's looking at mid-game.
//
// Deliberately NOT an OS notification — most OSes (Windows' Focus Assist
// chief among them) auto-suppress those the instant a game goes
// fullscreen, which is exactly the moment these need to be seen.

import { emitTo } from "@tauri-apps/api/event";

export const OVERLAY_TOAST_EVENT = "overlay-toast";

// A single line, not a title+body pair — kept deliberately minimal (a
// small pill, not a card) so it stays legible at every size preset,
// "Small" included, without wrapping or overflowing the window.
export interface OverlayToastPayload {
  text: string;
  kind: "success" | "error" | "info";
}

export async function showOverlayToast(payload: OverlayToastPayload): Promise<void> {
  try {
    await emitTo("overlay", OVERLAY_TOAST_EVENT, payload);
  } catch {
    // Best-effort — if the overlay window somehow isn't there (older
    // build without it, whatever), this shouldn't be able to break
    // whatever triggered it.
  }
}
