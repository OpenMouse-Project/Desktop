// Per-game device profiles — a saved DPI/polling-rate configuration a user
// can define once per game (pages/GamesPage.tsx, components/GameProfilePanel.tsx)
// and have pushed to the connected mouse either on demand ("Apply now") or
// automatically the instant that game's process is detected running
// (hooks/use-game-watcher.ts's rising-edge check against
// running_process_names). Persisted the same way as native-hid/device-store.ts
// — plain localStorage, keyed by games.json's own `id`, not a source of
// truth for anything the device itself reports.
//
// Deliberately just DPI and polling rate, not every writable setting
// DevicePerformanceTab exposes (lift-off distance, gaming surface mode,
// ...) — those two are the only ones close to universally supported across
// gaming mice; a profile built around a setting most devices don't have
// would be misleading more often than useful.
//
// A field being `undefined` means "this profile doesn't touch that setting,"
// not "set it to nothing" — a profile can be DPI-only, or rate-only.
// applyGameProfile() below skips whatever isn't set.

import type { MouseStatus } from "@openmouse/protocol/drivers/mouse-types";
import type { HidInterfaceInfo } from "../native-hid/tauri-hid-device";
import { setDpi, setPollingRate } from "../native-hid/logitech-actions";

const STORAGE_KEY = "openmouse:game-profiles";

export interface GameProfile {
  dpi?: number;
  dpiY?: number;
  pollingRateHz?: number;
  /** Push this profile to the connected mouse the moment the game is detected running. */
  autoApply: boolean;
}

type ProfileMap = Record<string, GameProfile>;
type Listener = (profiles: ProfileMap) => void;

function load(): ProfileMap {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    const parsed: unknown = JSON.parse(raw);
    return parsed && typeof parsed === "object" ? (parsed as ProfileMap) : {};
  } catch {
    // Private-browsing-style storage blocks, corrupted JSON, whatever — this
    // is a convenience, not a source of truth, so just start empty.
    return {};
  }
}

let profiles: ProfileMap = load();
const listeners = new Set<Listener>();

function persist() {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(profiles));
  } catch {
    // Best-effort — losing saved profiles just means re-entering them.
  }
}

function emit() {
  for (const listener of listeners) listener(profiles);
}

export function getGameProfile(gameId: string): GameProfile | undefined {
  return profiles[gameId];
}

export function saveGameProfile(gameId: string, profile: GameProfile): void {
  profiles = { ...profiles, [gameId]: profile };
  persist();
  emit();
}

export function clearGameProfile(gameId: string): void {
  if (!(gameId in profiles)) return;
  const next = { ...profiles };
  delete next[gameId];
  profiles = next;
  persist();
  emit();
}

/** Returns an unsubscribe function. Immediately calls `listener` with the current map. */
export function subscribeGameProfiles(listener: Listener): () => void {
  listeners.add(listener);
  listener(profiles);
  return () => listeners.delete(listener);
}

/** True if a profile actually overrides at least one setting. */
export function isProfileMeaningful(profile: GameProfile | undefined): profile is GameProfile {
  if (!profile) return false;
  return profile.dpi !== undefined || profile.pollingRateHz !== undefined;
}

/** Short human-readable summary of what a profile actually changes, for the apply notification. */
export function describeProfile(profile: GameProfile): string {
  const parts: string[] = [];
  if (profile.dpi !== undefined) {
    parts.push(profile.dpiY !== undefined && profile.dpiY !== profile.dpi
      ? `${profile.dpi}×${profile.dpiY} DPI`
      : `${profile.dpi} DPI`);
  }
  if (profile.pollingRateHz !== undefined) parts.push(`${profile.pollingRateHz} Hz`);
  return parts.join(", ");
}

/**
 * Pushes every setting a profile actually overrides to the connected mouse,
 * one write at a time — same one-write-at-a-time shape as
 * DevicePerformanceTab's individual controls, just run back-to-back instead
 * of one per user click. Each setter already opens/closes its own
 * short-lived connection and is serialized by hid-open-lock.ts, so running
 * them in sequence (not in parallel) is what keeps this safe.
 *
 * Applies every field independently rather than stopping at the first
 * failure — a mouse that rejects one setting (e.g. an unsupported polling
 * rate) shouldn't also block the DPI change that would have worked fine.
 * Failures are collected and thrown together so the caller can show one
 * toast instead of a flurry of them.
 */
export async function applyGameProfile(
  info: HidInterfaceInfo,
  profile: GameProfile,
  onApplied: (patch: Partial<MouseStatus>) => void,
): Promise<void> {
  const errors: string[] = [];

  async function run(label: string, action: () => Promise<void>) {
    try {
      await action();
    } catch (error) {
      errors.push(`${label}: ${error instanceof Error ? error.message : String(error)}`);
    }
  }

  if (profile.dpi !== undefined) {
    await run("DPI", async () => {
      const applied = await setDpi(info, profile.dpi!, profile.dpiY);
      onApplied({ dpi: applied });
    });
  }
  if (profile.pollingRateHz !== undefined) {
    await run("Polling rate", async () => {
      const applied = await setPollingRate(info, profile.pollingRateHz!);
      onApplied({ pollingRateHz: applied });
    });
  }

  if (errors.length > 0) throw new Error(errors.join("; "));
}
