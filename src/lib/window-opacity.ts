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
