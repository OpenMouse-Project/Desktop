// Shared constants for editing a Logitech HID++ mouse's live-writable
// settings — used by both the always-on device control tab
// (components/DevicePerformanceTab.tsx) and the per-game profile editor
// (components/GameProfilePanel.tsx), so the two don't drift out of sync on
// what counts as a "preset" DPI or a valid surface mode.

import type { GamingSurfaceMode } from "../native-hid/logitech-actions";

// Common sensitivity steps — same list the webapp offers, not every value
// this sensor can actually hit (custom DPI inputs are for that).
export const DPI_PRESETS = [400, 800, 1600, 3200, 6400, 8000];
// Logitech HERO 2-class sensors (PRO X Superlight 2, G502 X family, ...)
// this driver has been tested against top out at 32000 DPI. Not every mouse
// on this driver actually goes that high — the device is the source of
// truth: setDpi()'s return value is what actually gets displayed, this is
// just the input's outer bound.
export const DPI_MIN = 50;
export const DPI_MAX = 32000;
export const DPI_STEP = 50;

export const GAMING_SURFACE_MODES: GamingSurfaceMode[] = ["On", "Off", "Auto"];
