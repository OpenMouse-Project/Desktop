// Window transparency slider (Settings' Appearance section). The main
// window is already OS-transparent (tauri.conf.json's "transparent": true,
// there for the rounded-corner shell — see App.css's body/full-desktop-shell
// comment) but every surface that actually covers the window paints a fully
// opaque background, so none of that transparency has ever been visible.
// This exposes it: an opacity percentage applied to those surfaces'
// backgrounds via color-mix, so a lower value genuinely shows the desktop
// behind the app rather than just a different app color.
//
// Floored at 40%, not 0%: below that, text and controls stop reading as a
// window and start reading as broken — a floor keeps "very transparent"
// from becoming "unusable," the same reasoning most window managers' own
// opacity sliders use.

const PREF_KEY = "openmouse.window-opacity";
const DEFAULT_OPACITY = 100;
const MIN_OPACITY = 40;

export function getWindowOpacity(): number {
  const raw = localStorage.getItem(PREF_KEY);
  const value = raw ? Number(raw) : DEFAULT_OPACITY;
  return Number.isFinite(value) ? Math.min(100, Math.max(MIN_OPACITY, value)) : DEFAULT_OPACITY;
}

export function applyWindowOpacity(percent: number): void {
  const clamped = Math.min(100, Math.max(MIN_OPACITY, percent));
  document.documentElement.style.setProperty("--window-opacity", `${clamped}%`);
}

export function saveWindowOpacity(percent: number): void {
  const clamped = Math.min(100, Math.max(MIN_OPACITY, percent));
  localStorage.setItem(PREF_KEY, String(clamped));
  applyWindowOpacity(clamped);
}

/** Apply the persisted opacity on startup — called once before first render, same as initTheme(). */
export function initWindowOpacity(): void {
  applyWindowOpacity(getWindowOpacity());
}

// Blur amount, layered on top of the opacity above. The native vibrancy
// material (tauri.conf.json's windowEffects — macOS "sidebar"/Windows
// "mica") does the actual blur-behind-the-window and has no adjustable
// radius through its public API on either platform, so this is a second,
// independent blur: a plain CSS backdrop-filter on the app's own outer
// containers, blurring whatever the native layer already put behind them.
// It's additive, not a substitute — even at 0px here, the native layer is
// still blurring; this controls how much *extra* softening sits on top.

const BLUR_PREF_KEY = "openmouse.window-blur";
const DEFAULT_BLUR = 0;
const MAX_BLUR = 24;

export function getPanelBlur(): number {
  const raw = localStorage.getItem(BLUR_PREF_KEY);
  const value = raw ? Number(raw) : DEFAULT_BLUR;
  return Number.isFinite(value) ? Math.min(MAX_BLUR, Math.max(0, value)) : DEFAULT_BLUR;
}

export function applyPanelBlur(px: number): void {
  const clamped = Math.min(MAX_BLUR, Math.max(0, px));
  document.documentElement.style.setProperty("--panel-blur", `${clamped}px`);
}

export function savePanelBlur(px: number): void {
  const clamped = Math.min(MAX_BLUR, Math.max(0, px));
  localStorage.setItem(BLUR_PREF_KEY, String(clamped));
  applyPanelBlur(clamped);
}

/** Apply the persisted blur on startup — called once before first render, same as initWindowOpacity(). */
export function initPanelBlur(): void {
  applyPanelBlur(getPanelBlur());
}
