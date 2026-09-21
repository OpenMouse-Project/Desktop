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
const DYNAMIC_SIGNATURE_KEY = "openmouse.theme.dynamic-wallpaper-signature";
/** The raw sampled hex, separate from the generated CSS — so the vibrancy
 *  slider can regenerate the palette instantly (no wallpaper re-sample). */
const DYNAMIC_HEX_KEY = "openmouse.theme.dynamic-accent-hex";
const DYNAMIC_VIBRANCY_KEY = "openmouse.theme.dynamic-vibrancy";
/** 100 = the ramp's own calibrated saturations, unscaled (see DARK_RAMP/
 *  LIGHT_RAMP's doc comment) — not "maximum possible," just "as designed." */
const DYNAMIC_VIBRANCY_DEFAULT = 100;
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

/** How long the transition CSS (see withThemeTransition) stays applied — must match App.css's `.theme-transition` duration. */
const THEME_TRANSITION_MS = 380;
const THEME_TRANSITION_CLASS = "theme-transition";

/**
 * Runs `apply` with a temporary class on <html> that App.css gives a
 * transition to (background/border/text color, ~380ms) — plain preset
 * switches (and a live wallpaper re-sample) otherwise jump instantly, since
 * nothing about a CSS custom-property change is animatable on its own; the
 * transition has to live on the properties that actually read those
 * variables, scoped to a short window around the change rather than left on
 * permanently, which would fight every other hover/focus transition in the
 * app for the rest of the session.
 */
function withThemeTransition(apply: () => void): void {
  const root = document.documentElement;
  root.classList.add(THEME_TRANSITION_CLASS);
  apply();
  window.setTimeout(() => root.classList.remove(THEME_TRANSITION_CLASS), THEME_TRANSITION_MS);
}

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
  withThemeTransition(() => applyTheme(state));
  // Best-effort — if the overlay window somehow isn't there (older build,
  // whatever), the main window's own theme change above still applied fine.
  void emitTo("overlay", THEME_CHANGE_EVENT, state).catch(() => {});
  // Picking "Dynamic" applies the cached/fallback accent immediately (see
  // applyTheme above) and kicks off a real sample right away, rather than
  // waiting for the next launch or an unrelated refresh to land one.
  if (state.presetId === "dynamic") void refreshDynamicAccent(true);
}

function hexToRgb(hex: string): [number, number, number] {
  const clean = hex.replace("#", "");
  return [
    parseInt(clean.slice(0, 2), 16) || 0,
    parseInt(clean.slice(2, 4), 16) || 0,
    parseInt(clean.slice(4, 6), 16) || 0,
  ];
}

/** Returns [hue 0-360, saturation 0-100, lightness 0-100]. */
function rgbToHsl(r: number, g: number, b: number): [number, number, number] {
  const rn = r / 255;
  const gn = g / 255;
  const bn = b / 255;
  const max = Math.max(rn, gn, bn);
  const min = Math.min(rn, gn, bn);
  const l = (max + min) / 2;
  const d = max - min;
  if (d === 0) return [0, 0, l * 100];
  const s = d / (1 - Math.abs(2 * l - 1));
  let h: number;
  if (max === rn) h = ((gn - bn) / d) % 6;
  else if (max === gn) h = (bn - rn) / d + 2;
  else h = (rn - gn) / d + 4;
  h *= 60;
  if (h < 0) h += 360;
  return [h, s * 100, l * 100];
}

function hslToHex(h: number, s: number, l: number): string {
  const sn = s / 100;
  const ln = l / 100;
  const c = (1 - Math.abs(2 * ln - 1)) * sn;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = ln - c / 2;
  let [r, g, b] = [0, 0, 0];
  if (h < 60) [r, g, b] = [c, x, 0];
  else if (h < 120) [r, g, b] = [x, c, 0];
  else if (h < 180) [r, g, b] = [0, c, x];
  else if (h < 240) [r, g, b] = [0, x, c];
  else if (h < 300) [r, g, b] = [x, 0, c];
  else [r, g, b] = [c, 0, x];
  const toHex = (v: number) => Math.round((v + m) * 255).toString(16).padStart(2, "0");
  return `#${toHex(r)}${toHex(g)}${toHex(b)}`;
}

/**
 * Every hand-tuned preset (Emerald/Violet/Ice/Mono in App.css) redefines the
 * *whole* surface palette — ink, sidebar, card, raised, control, hover,
 * line, line-strong, text, bright, dim, muted, faint — not just the accent,
 * each one holding roughly the accent's own hue at a fixed saturation/
 * lightness. These ramps are that same pattern, calibrated by converting
 * Violet's actual token values to HSL: its hue holds ~250-260° across every
 * token while lightness climbs from ~3.5% (ink) to ~97% (text), so building
 * any hue through the same ramp reproduces that same "hand-tuned dark
 * theme" look for whatever hue a wallpaper happens to produce.
 */
const DARK_RAMP: { key: string; s: number; l: number }[] = [
  { key: "--ink", s: 33, l: 3.5 },
  { key: "--sidebar", s: 31, l: 5 },
  { key: "--card", s: 28, l: 7 },
  { key: "--raised", s: 28, l: 10 },
  { key: "--control", s: 30, l: 12 },
  { key: "--hover", s: 29, l: 15.5 },
  { key: "--line", s: 25, l: 20 },
  { key: "--line-strong", s: 24, l: 25 },
];

const LIGHT_RAMP: { key: string; s: number; l: number }[] = [
  { key: "--text", s: 40, l: 97 },
  { key: "--bright", s: 45, l: 94 },
  { key: "--dim", s: 22, l: 78 },
  { key: "--muted", s: 16, l: 61 },
  { key: "--faint", s: 13, l: 54 },
];

/**
 * `#rrggbb` -> a `:root[data-theme="dynamic"]` block overriding the whole
 * surface palette (see the ramp comment above), not just the accent — the
 * earlier version only set `--ui-accent`/`--ui-accent-ink`/`--ui-accent-soft`,
 * which don't exist anywhere in this app (its real tokens are `--accent`/
 * `--accent-ink`; there's no "-soft" variant at all). CONFIRMED: that meant
 * selecting Dynamic never visibly changed anything outside its own swatch
 * preview dot, which reads a separate variable this function also sets.
 */
/**
 * `vibrancy` (0-150, see the slider in Settings) scales every saturation in
 * the ramp — and the accent's own — by `vibrancy/100`, clamped to a valid
 * 0-100 HSL saturation. 100 reproduces the ramp exactly as calibrated; below
 * that mutes the whole palette toward gray (0 = fully neutral, hue
 * irrelevant); above it pushes past the calibrated intensity, up to a hard
 * ceiling at 100 saturation. The accent itself is included, not just the
 * surface ramp — a "vibrancy" slider that left the one color most people
 * actually look at untouched would feel like it wasn't doing anything.
 */
function accentCssFromHex(hex: string, vibrancy: number): string {
  const [r, g, b] = hexToRgb(hex);
  const [hue, saturation, lightness] = rgbToHsl(r, g, b);
  const scale = vibrancy / 100;
  const scaleSat = (s: number) => Math.min(100, Math.max(0, s * scale));
  const luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
  const accentInk = luminance > 0.55 ? hslToHex(hue, scaleSat(30), 8) : hslToHex(hue, scaleSat(15), 94);
  const accent = hslToHex(hue, scaleSat(saturation), lightness);
  const declarations = [...DARK_RAMP, ...LIGHT_RAMP]
    .map(({ key, s, l }) => `${key}: ${hslToHex(hue, scaleSat(s), l)};`)
    .join(" ");
  // Two rules: an unconditional `--dynamic-accent-preview`, which App.css's
  // swatch picker reads for the Dynamic option's own preview dot regardless
  // of which theme is actually active (so it can be previewed before being
  // selected), and the `[data-theme="dynamic"]` palette override, which only
  // takes effect once Dynamic actually is the active theme.
  return `:root { --dynamic-accent-preview: ${accent}; } `
    + `:root[data-theme="dynamic"] { ${declarations} --accent: ${accent}; --accent-ink: ${accentInk}; }`;
}

export function getDynamicVibrancy(): number {
  const raw = localStorage.getItem(DYNAMIC_VIBRANCY_KEY);
  const value = raw ? Number(raw) : DYNAMIC_VIBRANCY_DEFAULT;
  return Number.isFinite(value) ? Math.min(150, Math.max(0, value)) : DYNAMIC_VIBRANCY_DEFAULT;
}

/**
 * Regenerates the Dynamic theme's CSS from the last sampled wallpaper color
 * at the new vibrancy — instantly, no wallpaper re-sample or Tauri call
 * needed, since vibrancy is purely a saturation multiplier applied after
 * the color's already been extracted.
 *
 * No transition here (unlike a preset switch or a real wallpaper update):
 * this runs on every tick of a slider drag, and re-triggering a 380ms CSS
 * transition that often would fight itself rather than look smooth — the
 * live drag itself already reads as continuous motion.
 */
export function applyDynamicVibrancy(percent: number): void {
  const clamped = Math.min(150, Math.max(0, percent));
  const hex = localStorage.getItem(DYNAMIC_HEX_KEY) ?? DYNAMIC_FALLBACK_HEX;
  applyDynamicStyle(accentCssFromHex(hex, clamped));
}

/** Persists the vibrancy slider's value and broadcasts it to the overlay. Call on release, not on every drag tick — see applyDynamicVibrancy for the live-preview half. */
export function saveDynamicVibrancy(percent: number): void {
  const clamped = Math.min(150, Math.max(0, percent));
  localStorage.setItem(DYNAMIC_VIBRANCY_KEY, String(clamped));
  applyDynamicVibrancy(clamped);
  const hex = localStorage.getItem(DYNAMIC_HEX_KEY) ?? DYNAMIC_FALLBACK_HEX;
  localStorage.setItem(DYNAMIC_CACHE_KEY, accentCssFromHex(hex, clamped));
  void emitTo("overlay", THEME_CHANGE_EVENT, getThemeState()).catch(() => {});
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
 *
 * Unless `force`, this checks wallpaper.rs's cheap `wallpaper_signature`
 * (a file path + mtime, no image decode) first and returns immediately if
 * it matches the last one seen. CONFIRMED as a real, user-visible freeze
 * without this: startDynamicAccentWatcher's focus listener means every
 * alt-tab back into the app was spawning osascript AND fully decoding +
 * resizing the wallpaper file — genuinely slow for a large photo, and it
 * was paying that cost every single time, wallpaper unchanged or not.
 * `force` is for the two callers who need a guaranteed fresh sample
 * regardless of signature — the cold-start refresh (main.tsx) and picking
 * "Dynamic" in Settings — since a stale color there would look like the
 * feature doing nothing.
 */
export async function refreshDynamicAccent(force = false): Promise<void> {
  try {
    if (!force) {
      const signature = await invoke<string>("wallpaper_signature");
      if (signature === localStorage.getItem(DYNAMIC_SIGNATURE_KEY)) return;
      localStorage.setItem(DYNAMIC_SIGNATURE_KEY, signature);
    }
    const hex = await invoke<string>("wallpaper_accent_color");
    const css = accentCssFromHex(hex, getDynamicVibrancy());
    // Skip if the wallpaper's color hasn't actually moved since the last
    // sample — both to avoid rewriting identical CSS when the signature
    // check above was skipped (force) or the file changed without its
    // dominant color actually changing, and so a same-color re-sample
    // doesn't needlessly fire the transition on a page nothing is changing.
    if (css === localStorage.getItem(DYNAMIC_CACHE_KEY)) return;
    withThemeTransition(() => applyDynamicStyle(css));
    localStorage.setItem(DYNAMIC_CACHE_KEY, css);
    localStorage.setItem(DYNAMIC_HEX_KEY, hex);
    void emitTo("overlay", THEME_CHANGE_EVENT, getThemeState()).catch(() => {});
  } catch {
    // Best-effort — whatever was already showing (cache, or the fallback
    // green from applyTheme) stays exactly as good a guess as before.
  }
}

/**
 * Keeps the Dynamic theme in sync with the desktop wallpaper while the app
 * is running, not just at launch — there's no OS-level "wallpaper changed"
 * event this reaches for cross-platform, so it polls instead: a plain
 * interval, plus an immediate check on window focus so switching back to
 * the app after changing wallpapers elsewhere doesn't sit stale.
 *
 * 2 seconds, not something more conservative like 60s: each tick is a cheap
 * signature check (wallpaper.rs's wallpaper_signature — a path + mtime, no
 * image decode) running on Tauri's blocking-executor thread pool
 * (wallpaper.rs's spawn_blocking), not the UI thread, so polling often costs
 * nothing the user can feel; the wallpaper's real color only gets
 * re-sampled on the rare tick where that signature actually changed. This
 * is deliberately snappy — a user who just changed their wallpaper wants to
 * see the theme follow within a couple seconds, not wonder if it's stuck.
 *
 * Call once from the main window only (see refreshDynamicAccent's own doc
 * comment for why); safe to call unconditionally regardless of which preset
 * is actually active, same as refreshDynamicAccent itself.
 */
export function startDynamicAccentWatcher(): void {
  const POLL_MS = 2_000;
  window.setInterval(() => void refreshDynamicAccent(), POLL_MS);
  window.addEventListener("focus", () => void refreshDynamicAccent());
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
  applyDynamicStyle(
    localStorage.getItem(DYNAMIC_CACHE_KEY) ?? accentCssFromHex(DYNAMIC_FALLBACK_HEX, getDynamicVibrancy()),
  );

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