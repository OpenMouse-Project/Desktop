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
import { invoke } from "@tauri-apps/api/core";

const PREF_THEME = "openmouse.theme";
const PREF_CUSTOM_CSS = "openmouse.theme.custom-css";
const STYLE_ID = "om-custom-theme-css";
const DYNAMIC_STYLE_ID = "om-dynamic-theme-css";
const DYNAMIC_CACHE_KEY = "openmouse.theme.dynamic-accent-css";
/** Same default green the app ships with everywhere else, used until the
 *  first wallpaper sample lands (or forever, on a platform without one). */
const DYNAMIC_FALLBACK_HEX = "#5dde89";

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
  {
    id: "dynamic",
    label: "Dynamic",
    description: "Pulls its accent color from your current desktop wallpaper, like Android's Material You. "
      + "macOS and Windows only — there's no single place a wallpaper lives across Linux desktop "
      + "environments, so it falls back to the default green there.",
  },
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
  // Picking "Dynamic" applies the cached/fallback accent immediately (see
  // applyTheme above) and kicks off a real sample right away, rather than
  // waiting for the next launch or an unrelated refresh to land one.
  if (state.presetId === "dynamic") void refreshDynamicAccent();
}

/**
 * `#rrggbb` -> a `:root[data-theme="dynamic"]` block overriding just the
 * accent tokens — the "dynamic" preset otherwise sits on the same base
 * tokens as "default" (no `[data-theme="dynamic"]` block in App.css means
 * the bare `:root` values apply), so this is the entire visual difference.
 * The ink color is a plain luminance check rather than anything fancier:
 * this only ever needs to pick readable text on top of a solid swatch of
 * the accent, not model how humans perceive color.
 */
function accentCssFromHex(hex: string): string {
  const clean = hex.replace("#", "");
  const r = parseInt(clean.slice(0, 2), 16) || 0;
  const g = parseInt(clean.slice(2, 4), 16) || 0;
  const b = parseInt(clean.slice(4, 6), 16) || 0;
  const luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
  const ink = luminance > 0.55 ? "#0c0d0f" : "#f5f6f8";
  // Two rules: an unconditional `--dynamic-accent-preview`, which App.css's
  // swatch picker reads for the Dynamic option's own preview dot regardless
  // of which theme is actually active (so it can be previewed before being
  // selected), and the `[data-theme="dynamic"]` accent override, which only
  // takes effect once Dynamic actually is the active theme.
  return `:root { --dynamic-accent-preview: ${hex}; } `
    + `:root[data-theme="dynamic"] { --ui-accent: ${hex}; --ui-accent-ink: ${ink}; `
    + `--ui-accent-soft: color-mix(in srgb, ${hex} 16%, transparent); }`;
}

function applyDynamicStyle(css: string): void {
  let style = document.getElementById(DYNAMIC_STYLE_ID) as HTMLStyleElement | null;
  if (!style) {
    style = document.createElement("style");
    style.id = DYNAMIC_STYLE_ID;
    document.head.appendChild(style);
  }
  style.textContent = css;
}

/**
 * Re-samples the current wallpaper (macOS/Windows only — see wallpaper.rs)
 * and updates the "dynamic" preset's accent color, caching the result so
 * the next launch doesn't flash the fallback green before this finishes.
 *
 * Calls into Tauri, so this must only ever run in the main window: the
 * "overlay" window's capability (capabilities/overlay.json) doesn't grant
 * it any app commands, only the few window/event ones it actually needs.
 * The overlay never calls this itself — it just re-reads the cache
 * `applyTheme` already checks, whenever the main window's own
 * THEME_CHANGE_EVENT broadcast (below) tells it to.
 *
 * Runs regardless of whether "dynamic" is the *active* preset — the Theme
 * picker's Dynamic swatch shows the real computed color as its own preview
 * dot even while some other theme is selected, so it needs a real sample
 * to show, not just a placeholder until the user actually picks it.
 */
export async function refreshDynamicAccent(): Promise<void> {
  try {
    const hex = await invoke<string>("wallpaper_accent_color");
    const css = accentCssFromHex(hex);
    applyDynamicStyle(css);
    localStorage.setItem(DYNAMIC_CACHE_KEY, css);
    void emitTo("overlay", THEME_CHANGE_EVENT, getThemeState()).catch(() => {});
  } catch {
    // Best-effort — whatever was already showing (cache, or the fallback
    // green from applyTheme) stays exactly as good a guess as before.
  }
}

/** Push a theme state onto the DOM (root `data-theme` + a custom-CSS <style>). */
export function applyTheme(state: ThemeState): void {
  const root = document.documentElement;
  root.dataset.theme = state.presetId === "default" ? "default" : state.presetId;

  // Kept applied regardless of which preset is active: its accent-override
  // half only takes effect under `[data-theme="dynamic"]` (a no-op selector
  // otherwise), but its preview-dot variable needs to stay live so the
  // Dynamic swatch shows the real computed color even while browsing other
  // presets, not just after switching to it.
  applyDynamicStyle(localStorage.getItem(DYNAMIC_CACHE_KEY) ?? accentCssFromHex(DYNAMIC_FALLBACK_HEX));

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