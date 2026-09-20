// Theme management for the whole app (SettingsPage + App root + Overlay).
//
// Two kinds of theme, like Vencord:
//   1. Presets — a name (Emerald / Violet / Ice / Mono) that activates a
//      matching `[data-theme="..."]` block in App.css overriding the design
//      tokens (:root vars). The default "Matt" theme is the app's normal CSS
//      with no override block.
//   2. Custom CSS — free-form stylesheet the user pastes, injected into a
//      dedicated <style id="custom-theme-css"> element so it wins over the
//      base rules without touching them (it's the last thing in <head>).
//
// All of it is keyed off <html data-theme="emerald"> plus the custom-CSS
// <style>. State lives in localStorage so it survives restarts and is shared
// across the main + overlay windows (same origin).

import { emitTo } from "@tauri-apps/api/event";

const PREF_THEME = "openmouse.theme";
const PREF_CUSTOM_CSS = "openmouse.theme.custom-css";
const STYLE_ID = "om-custom-theme-css";

/**
 * Fired at the "overlay" window whenever the main window saves a new theme,
 * so the always-on-top game-alert overlay (OverlayApp.tsx) picks it up
 * immediately instead of only the next time it's recreated. Both windows
 * load `initTheme()` from the same localStorage on startup, which covers a
 * fresh launch — but the overlay is a long-lived background window that
 * doesn't reload, so a theme change made while it's already running never
 * reached its own separate `document` without this.
 */
export const THEME_CHANGE_EVENT = "openmouse-theme-changed";

export interface ThemePreset {
  id: string;
  label: string;
  description: string;
}

export const THEME_PRESETS: ThemePreset[] = [
  { id: "default", label: "Matt", description: "The app's default look — warm orange accent on a dark shell." },
  { id: "emerald", label: "Emerald", description: "Green-tinted dark theme — surfaces, borders and accents." },
  { id: "violet", label: "Violet", description: "Violet-tinted dark theme for a cooler, calmer feel." },
  { id: "ice", label: "Ice", description: "Icy blue-tinted dark theme." },
  { id: "mono", label: "Mono", description: "Monochrome dark theme — pure neutrals, no color." },
  { id: "light", label: "Light", description: "The whole app flips to a light shell with dark text." },
];

export interface ThemeState {
  presetId: string;
  customCss: string;
}

export function getThemeState(): ThemeState {
  return {
    presetId: localStorage.getItem(PREF_THEME) ?? "default",
    customCss: localStorage.getItem(PREF_CUSTOM_CSS) ?? "",
  };
}

export function saveThemeState(state: ThemeState): void {
  localStorage.setItem(PREF_THEME, state.presetId);
  if (state.customCss) {
    localStorage.setItem(PREF_CUSTOM_CSS, state.customCss);
  } else {
    localStorage.removeItem(PREF_CUSTOM_CSS);
  }
  applyTheme(state);
  // Best-effort — if the overlay window somehow isn't there (older build,
  // whatever), the main window's own theme change above still applied fine.
  void emitTo("overlay", THEME_CHANGE_EVENT, state).catch(() => {});
}

/** Push a theme state onto the DOM (root `data-theme` + a custom-CSS <style>). */
export function applyTheme(state: ThemeState): void {
  const root = document.documentElement;
  root.dataset.theme = state.presetId === "default" ? "default" : state.presetId;

  let style = document.getElementById(STYLE_ID) as HTMLStyleElement | null;
  if (!state.customCss) {
    style?.remove();
    return;
  }
  if (!style) {
    style = document.createElement("style");
    style.id = STYLE_ID;
    document.head.appendChild(style);
  }
  style.textContent = state.customCss;
}

/** Apply the persisted theme on startup — called once before first render. */
export function initTheme(): void {
  applyTheme(getThemeState());
}