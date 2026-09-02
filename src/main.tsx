import { render } from "preact";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { OverlayApp } from "./OverlayApp";
import { initTheme } from "./lib/themes";
import "./App.css";

// Apply the saved theme (preset + custom CSS) before first paint so there's
// no unstyled flash and both windows share the same look.
initTheme();

// Both windows (tauri.conf.json's "main" and "overlay") load this exact
// same entry point — there's no separate route/URL for the overlay, just a
// label check, so a single build serves both without needing a second
// index.html or a client-side router this app otherwise has no use for.
const isOverlay = getCurrentWindow().label === "overlay";

render(isOverlay ? <OverlayApp /> : <App />, document.getElementById("root")!);
