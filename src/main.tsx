import { render } from "preact";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { OverlayApp } from "./OverlayApp";
import { initTheme, refreshDynamicAccent, startDynamicAccentWatcher } from "./lib/themes";
import { initPanelBlur, initWindowOpacity } from "./lib/window-opacity";
import "./App.css";

// Apply the saved theme (preset + custom CSS) before first paint so there's
// no unstyled flash and both windows share the same look.
initTheme();

// Both windows (tauri.conf.json's "main" and "overlay") load this exact
// same entry point — there's no separate route/URL for the overlay, just a
// label check, so a single build serves both without needing a second
// index.html or a client-side router this app otherwise has no use for.
const isOverlay = getCurrentWindow().label === "overlay";

// The opacity slider only makes sense for the main window — the overlay is
// its own small always-on-top toast, not something a user resizes/sits in.
if (!isOverlay) {
  initWindowOpacity();
  initPanelBlur();
}

// Re-sample the wallpaper on the main window's own launch, in case it
// changed since the cached accent color initTheme() just applied — the
// overlay never does this itself (see themes.ts's refreshDynamicAccent doc),
// it only ever picks up what the main window broadcasts.
if (!isOverlay) {
  void refreshDynamicAccent(true);
  startDynamicAccentWatcher();
}

render(isOverlay ? <OverlayApp /> : <App />, document.getElementById("root")!);
