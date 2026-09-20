// Frontend side of the OBS overlay (src-tauri/src/stream_overlay.rs) — a
// local HTTP server the user points an OBS Browser Source at, showing the
// currently-connected mouse's name, DPI and polling rate, live. This module
// owns the enabled/disabled preference and which fields are turned on
// (plain localStorage, matching lib/overlay-settings.ts) and the thin
// invoke() wrappers around the Rust commands; SettingsPage.tsx drives it —
// its live preview is a real iframe of the same page OBS loads, so what
// the user builds there is exactly what ends up on stream — and
// use-mouse-connection.ts feeds it live status the same way it feeds
// tray.rs.

import { invoke } from "@tauri-apps/api/core";

const ENABLED_KEY = "openmouse:stream-overlay-enabled";
const FIELDS_KEY = "openmouse:stream-overlay-fields";

export interface StreamOverlayDeviceStatus {
  name: string;
  dpi: number | null;
  pollingRateHz: number | null;
}

/** Which parts of the overlay are turned on. Baked into the Browser Source URL as query params, not sent to the server as state. */
export interface StreamOverlayFields {
  name: boolean;
  dpi: boolean;
  polling: boolean;
}

const DEFAULT_FIELDS: StreamOverlayFields = { name: true, dpi: true, polling: true };

export function isStreamOverlayEnabled(): boolean {
  try {
    return localStorage.getItem(ENABLED_KEY) === "true";
  } catch {
    return false;
  }
}

function saveEnabled(enabled: boolean): void {
  try {
    if (enabled) localStorage.setItem(ENABLED_KEY, "true");
    else localStorage.removeItem(ENABLED_KEY);
  } catch {
    // Best-effort — worst case the toggle doesn't survive a restart.
  }
}

export function getStreamOverlayFields(): StreamOverlayFields {
  try {
    const raw = localStorage.getItem(FIELDS_KEY);
    if (!raw) return DEFAULT_FIELDS;
    const parsed = JSON.parse(raw) as Partial<StreamOverlayFields>;
    return {
      name: parsed.name ?? DEFAULT_FIELDS.name,
      dpi: parsed.dpi ?? DEFAULT_FIELDS.dpi,
      polling: parsed.polling ?? DEFAULT_FIELDS.polling,
    };
  } catch {
    return DEFAULT_FIELDS;
  }
}

export function saveStreamOverlayFields(fields: StreamOverlayFields): void {
  try {
    localStorage.setItem(FIELDS_KEY, JSON.stringify(fields));
  } catch {
    // Best-effort — losing a saved preference just means it resets to default.
  }
}

/** Appends each field as `0`/`1` to the base overlay URL — same query string the page itself reads to decide what to draw (see stream_overlay.rs's OVERLAY_HTML). */
export function buildStreamOverlayUrl(baseUrl: string, fields: StreamOverlayFields): string {
  const params = new URLSearchParams({
    name: fields.name ? "1" : "0",
    dpi: fields.dpi ? "1" : "0",
    polling: fields.polling ? "1" : "0",
  });
  return `${baseUrl}?${params.toString()}`;
}

/** Starts the local server (idempotent) and persists the on/off preference. Returns the bare URL (no field query params). */
export async function enableStreamOverlay(): Promise<string> {
  const url = await invoke<string>("stream_overlay_start");
  saveEnabled(true);
  return url;
}

export async function disableStreamOverlay(): Promise<void> {
  await invoke("stream_overlay_stop");
  saveEnabled(false);
}

/** Current URL if the server's already running, without starting it. */
export function getStreamOverlayUrl(): Promise<string | null> {
  return invoke<string | null>("stream_overlay_status");
}

export function pushStreamOverlayStatus(status: StreamOverlayDeviceStatus | null): Promise<void> {
  return invoke("stream_overlay_set_device_status", { status }).then(() => undefined);
}
