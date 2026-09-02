// Per-game device profiles — a saved DPI/polling-rate configuration a user
// can define once per game (pages/GamesPage.tsx, components/GameProfilePanel.tsx)
// and have pushed to the connected mouse either on demand ("Apply now") or
// automatically the instant that game's process is detected running
// (hooks/use-game-watcher.ts's rising-edge check against
// running_process_names). Persisted the same way as native-hid/device-store.ts
// — plain localStorage, keyed by games.json's own `id`, not a source of
// truth for anything the device itself reports.
//
// Covers every writable setting DevicePerformanceTab exposes — DPI,
// polling rate, lift-off distance, and gaming surface mode — not just
// DPI/rate. Lift-off and surface mode are guarded behind the same
// `status.supportedLiftOffDistances`/`status.gamingSurfaceMode` presence
// checks DevicePerformanceTab itself uses (GameProfilePanel.tsx), so a
// profile only ever offers to set what the connected device actually
// reports supporting.
//
// A field being `undefined` means "this profile doesn't touch that setting,"
// not "set it to nothing" — a profile can be DPI-only, or rate-only.
// applyGameProfile() below skips whatever isn't set.

import type { MouseStatus } from "@openmouse/protocol/drivers/mouse-types";
import type { HidInterfaceInfo } from "../native-hid/tauri-hid-device";
import { withLogitechClient } from "../native-hid/logitech-actions";
import {
  setDpi,
  setGamingSurfaceMode,
  setLiftOffDistance,
  setPollingRate,
  type GamingSurfaceMode,
  type LiftOffDistance,
} from "../native-hid/write";

const STORAGE_KEY = "openmouse:game-profiles";

export interface GameProfile {
  dpi?: number;
  dpiY?: number;
  pollingRateHz?: number;
  liftOffDistance?: LiftOffDistance;
  gamingSurfaceMode?: GamingSurfaceMode;
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
  return (
    profile.dpi !== undefined ||
    profile.pollingRateHz !== undefined ||
    profile.liftOffDistance !== undefined ||
    profile.gamingSurfaceMode !== undefined
  );
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
  if (profile.liftOffDistance !== undefined) parts.push(`${profile.liftOffDistance} lift-off`);
  if (profile.gamingSurfaceMode !== undefined) parts.push(`surface ${profile.gamingSurfaceMode}`);
  return parts.join(", ");
}

/**
 * Pushes every setting a profile actually overrides to the connected mouse.
 *
 * Logitech covers WANTS a single shared session: its writes must resolve the
 * HID++ device index first, and paying open/resolve twice back-to-back for a
 * profile that sets both DPI and polling rate roughly doubles how long a game
 * launch takes to actually reach the mouse, plus the collision window with the
 * background status auto-refresh's own hid-open-lock (use-mouse-connection.ts,
 * every 5s). So Logitech goes through withLogitechClient once.
 *
 * Every other brand has no index to resolve (see write.ts's docs), so each
 * field is applied with its own short-lived generic open -> act -> close
 * write — the same cost DevicePerformanceTab's individual controls pay.
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
  const isLogitech = info.vendorId === 0x046d;

  async function run(label: string, action: () => Promise<void>) {
    try {
      await action();
    } catch (error) {
      errors.push(`${label}: ${error instanceof Error ? error.message : String(error)}`);
    }
  }

  if (isLogitech) {
    await withLogitechClient(info, "applyGameProfile", async (client) => {
      if (profile.dpi !== undefined) {
        await run("DPI", async () => {
          const applied = await client.setDpi(profile.dpi!, profile.dpiY);
          onApplied({ dpi: applied });
        });
      }
      if (profile.pollingRateHz !== undefined) {
        await run("Polling rate", async () => {
          const applied = await client.setPollingRate(profile.pollingRateHz!);
          onApplied({ pollingRateHz: applied });
        });
      }
      if (profile.liftOffDistance !== undefined) {
        await run("Lift-off distance", async () => {
          const applied = await client.setLiftOffDistance(profile.liftOffDistance!);
          onApplied({ liftOffDistance: applied });
        });
      }
      if (profile.gamingSurfaceMode !== undefined) {
        await run("Gaming surface", async () => {
          const applied = await client.setGamingSurfaceMode(profile.gamingSurfaceMode!);
          onApplied({ gamingSurfaceMode: applied });
        });
      }
    });
  } else {
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
    if (profile.liftOffDistance !== undefined) {
      await run("Lift-off distance", async () => {
        const applied = await setLiftOffDistance(info, profile.liftOffDistance!);
        onApplied({ liftOffDistance: applied });
      });
    }
    if (profile.gamingSurfaceMode !== undefined) {
      await run("Gaming surface", async () => {
        const applied = await setGamingSurfaceMode(info, profile.gamingSurfaceMode!);
        onApplied({ gamingSurfaceMode: applied });
      });
    }
  }

  if (errors.length > 0) throw new Error(errors.join("; "));
}
