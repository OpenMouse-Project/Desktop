// Cross-grade: switching between the Bridge-only and full Desktop builds.
//
// These are genuinely separate installs (see src-tauri/tauri.bridge.conf.json
// — a different `identifier`, so macOS/Windows/Linux all treat them as
// distinct apps that can coexist). "Auto-apply" — detect what's already on
// disk and only fetch the piece that's missing, then install without extra
// prompts — needs two things this codebase doesn't have yet:
//
//   1. A manifest of what's installed, written by each installer, so a
//      relaunch can tell "Desktop is already here" from "only Bridge is
//      here" without re-downloading to check.
//   2. Permission to fetch a binary and hand it to the OS installer
//      programmatically (the `http` and `fs` Tauri plugins, plus
//      capabilities entries for this window to use them) — right now the
//      app can only reach the network via the updater plugin, which only
//      knows how to update the app it's already running, not fetch a
//      *different* app's installer.
//
// Until that lands, this does the safe subset: send the user to the
// GitHub release so the OS's normal installer flow (which already knows
// how to replace an existing install cleanly) handles it. Swap the body
// of requestDesktopInstall() for a real fetch+install once (1) and (2)
// above exist — the call site (CrossGradePrompt) doesn't need to change.
import { openUrl } from "@tauri-apps/plugin-opener";

const RELEASES_URL = "https://github.com/OpenMouse-Project/Desktop/releases/latest";

export async function requestDesktopInstall(): Promise<void> {
  await openUrl(RELEASES_URL);
}
