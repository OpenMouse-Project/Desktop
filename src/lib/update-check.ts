// Module-level "is an update available" store, plus the background poll
// that drives it — same pattern as lib/toast.ts and lib/game-profiles.ts.
// Deliberately NOT tied to a hook/component lifecycle: App.tsx swaps
// FullDesktopView/BridgeView entirely depending on mode (a mode switch
// unmounts the other one completely), so a poll owned by either view's
// component tree would stop the moment the user switched modes. This
// starts its own interval the instant the module is first imported (by
// TitleBar, which both views render) and just keeps running for the life
// of the app process, regardless of which view is currently showing.

import { check, type Update } from "@tauri-apps/plugin-updater";
import { showToast } from "./toast";

const CHECK_INTERVAL_MS = 30 * 60 * 1000; // 30 minutes

export interface UpdateCheckState {
  available: Update | null;
  checking: boolean;
}

let state: UpdateCheckState = { available: null, checking: false };
const listeners = new Set<(state: UpdateCheckState) => void>();

function emit() {
  for (const listener of listeners) listener(state);
}

function setState(patch: Partial<UpdateCheckState>) {
  state = { ...state, ...patch };
  emit();
}

// Only nag once per version — the 30-minute re-check would otherwise
// re-toast the exact same "an update is available" notice every half hour
// for as long as it sits uninstalled.
let notifiedVersion: string | null = null;

/**
 * Runs a real check and updates the shared state (and therefore the
 * TitleBar pill) either way. Errors are the caller's to handle: the
 * background timer below swallows them (a flaky network shouldn't nag the
 * user), while Settings' "Check for Updates" button lets them surface as a
 * toast, since that check was something the user explicitly asked for.
 */
export async function runUpdateCheck(): Promise<Update | null> {
  setState({ checking: true });
  try {
    const update = await check();
    setState({ available: update ?? null, checking: false });
    if (update && update.version !== notifiedVersion) {
      notifiedVersion = update.version;
      showToast(`OpenMouse Desktop v${update.version} is available — check Settings to install.`, "info", 8000);
    }
    return update ?? null;
  } catch (error) {
    setState({ checking: false });
    throw error;
  }
}

function backgroundCheck() {
  void runUpdateCheck().catch(() => {
    // Best-effort — see runUpdateCheck()'s own docs.
  });
}

// Fires once immediately (the old launch-time nudge), then every 30
// minutes for as long as the app runs.
backgroundCheck();
setInterval(backgroundCheck, CHECK_INTERVAL_MS);

export function getUpdateCheckState(): UpdateCheckState {
  return state;
}

/** Returns an unsubscribe function. Immediately calls `listener` with the current state. */
export function subscribeUpdateCheck(listener: (state: UpdateCheckState) => void): () => void {
  listeners.add(listener);
  listener(state);
  return () => listeners.delete(listener);
}
