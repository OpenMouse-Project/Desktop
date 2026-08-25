// Owned once by FullDesktopView (same pattern as useResourceMonitor) rather
// than by GamesPage, so game-launch/close detection — and therefore
// auto-applying/restoring a saved profile — keeps running no matter which
// tab is showing. Games are usually played full-screen with this app tabbed
// away, not sitting open on the Games page, so a poll that only ran while
// GamesPage was mounted would never catch the moment that mattered. Owning
// it here also means ToastHost (also mounted once, in FullDesktopView) can
// actually show the apply/restore notifications below regardless of which
// page the user is looking at.

import { useEffect, useRef, useState } from "preact/hooks";
import { invoke } from "@tauri-apps/api/core";
import type { MouseConnection } from "./use-mouse-connection";
import {
  applyGameProfile,
  describeProfile,
  getGameProfile,
  isProfileMeaningful,
  type GameProfile,
} from "../lib/game-profiles";
import { showToast } from "../lib/toast";

export interface Game {
  id: string;
  artwork?: string;
  name: string;
  steamAppId?: number;
  artworkFallback?: string;
  executables: string[];
}

interface GamesFile {
  games: Game[];
}

export type GamesListState =
  | { status: "loading" }
  | { status: "loaded"; games: Game[] }
  | { status: "error"; message: string };

// How often to re-scan running processes. Games are launched by a human,
// not something that needs sub-second reaction — matches this app's other
// background poll (use-mouse-connection.ts's device auto-refresh) in spirit.
const POLL_INTERVAL_MS = 4000;

/**
 * Whichever game currently "owns" the mouse's live settings — i.e. the most
 * recent game to auto-apply a profile that hasn't closed yet — along with
 * exactly the fields its profile touched, read off the device right before
 * that first override, so closing the game can put them back. The
 * always-on Performance tab controls are the implicit "default" this
 * restores to; nothing here ever saves its own separate default profile.
 *
 * Only one of these is tracked at a time: if a second game launches while
 * the first is still running (alt-tabbing between two), its profile applies
 * on top without capturing a new baseline — the ORIGINAL pre-game settings
 * stay what gets restored, and ownership just passes to the second game, so
 * closing the first one (no longer "in control") does nothing.
 */
interface ActiveOverride {
  gameId: string;
  restore: Pick<GameProfile, "dpi" | "dpiY" | "pollingRateHz">;
}

export function useGameWatcher(connection: MouseConnection) {
  const [list, setList] = useState<GamesListState>({ status: "loading" });
  const [runningProcesses, setRunningProcesses] = useState<Set<string>>(new Set());

  // Refs so the poll interval below (set up once, on mount) always sees the
  // latest games list and mouse connection without needing to tear down and
  // restart the interval every time either changes — same pattern
  // use-mouse-connection.ts uses for connectedRef.
  const gamesRef = useRef<Game[]>([]);
  const connectionRef = useRef(connection);
  connectionRef.current = connection;
  // Which games were already running as of the last scan, so a profile only
  // applies/restores on an edge (just launched / just closed) — not on
  // every single poll tick for as long as a game stays open or closed.
  const previouslyRunningRef = useRef<Set<string>>(new Set());
  const activeOverrideRef = useRef<ActiveOverride | null>(null);

  useEffect(() => {
    fetch("/games.json")
      .then(async (response) => {
        if (!response.ok) {
          throw new Error(`Could not load games (${response.status})`);
        }
        const data = (await response.json()) as GamesFile;
        gamesRef.current = data.games;
        setList({ status: "loaded", games: data.games });
      })
      .catch((reason: unknown) => {
        setList({ status: "error", message: reason instanceof Error ? reason.message : String(reason) });
      });
  }, []);

  useEffect(() => {
    let cancelled = false;

    function isGameRunning(game: Game, names: Set<string>): boolean {
      return game.executables.some((exe) => names.has(exe.toLowerCase()));
    }

    async function applyOnLaunch(game: Game) {
      const profile = getGameProfile(game.id);
      if (!profile?.autoApply || !isProfileMeaningful(profile)) return;
      const { connected, connectedInfo, patchStatus } = connectionRef.current;
      const canControl = connected?.brand === "Logitech" && connectedInfo !== null;
      if (!canControl || !connectedInfo || !connected) return;

      // The first game to take over is the one whose "before" snapshot
      // matters — that's what "default" means here. A second game taking
      // over from a first (still-running) one just changes who's in
      // control, not what gets restored afterward.
      if (!activeOverrideRef.current) {
        activeOverrideRef.current = {
          gameId: game.id,
          restore: {
            dpi: profile.dpi !== undefined ? connected.status.dpi : undefined,
            dpiY: profile.dpiY !== undefined ? connected.status.dpiY : undefined,
            pollingRateHz: profile.pollingRateHz !== undefined ? connected.status.pollingRateHz : undefined,
          },
        };
      } else {
        activeOverrideRef.current.gameId = game.id;
      }

      try {
        await applyGameProfile(connectedInfo, profile, patchStatus);
        showToast(`Applied "${game.name}" profile — ${describeProfile(profile)}.`, "success");
      } catch (error) {
        showToast(`Couldn't apply "${game.name}" profile: ${error instanceof Error ? error.message : String(error)}`, "error");
      }
    }

    async function restoreOnClose(game: Game) {
      const active = activeOverrideRef.current;
      if (!active || active.gameId !== game.id) return;
      activeOverrideRef.current = null;

      const restoreProfile: GameProfile = { ...active.restore, autoApply: false };
      if (!isProfileMeaningful(restoreProfile)) return;

      const { connected, connectedInfo, patchStatus } = connectionRef.current;
      const canControl = connected?.brand === "Logitech" && connectedInfo !== null;
      if (!canControl || !connectedInfo) return;

      try {
        await applyGameProfile(connectedInfo, restoreProfile, patchStatus);
        showToast(`${game.name} closed — restored your default ${describeProfile(restoreProfile)}.`, "info");
      } catch (error) {
        showToast(`Couldn't restore your default settings: ${error instanceof Error ? error.message : String(error)}`, "error");
      }
    }

    async function poll() {
      // No reason to keep scanning while nothing can even see the result —
      // minimized, hidden to the tray, or occluded.
      if (document.hidden) return;
      try {
        const names = await invoke<string[]>("running_process_names");
        if (cancelled) return;
        const nameSet = new Set(names);
        setRunningProcesses(nameSet);

        const games = gamesRef.current;
        const nowRunning = new Set(games.filter((game) => isGameRunning(game, nameSet)).map((game) => game.id));
        const justLaunched = games.filter(
          (game) => nowRunning.has(game.id) && !previouslyRunningRef.current.has(game.id),
        );
        const justClosed = games.filter(
          (game) => !nowRunning.has(game.id) && previouslyRunningRef.current.has(game.id),
        );
        previouslyRunningRef.current = nowRunning;

        for (const game of justClosed) void restoreOnClose(game);
        for (const game of justLaunched) void applyOnLaunch(game);
      } catch {
        // Best-effort — a failed scan just leaves the last known snapshot in
        // place rather than flashing every card to "not running."
      }
    }

    void poll();
    const interval = setInterval(poll, POLL_INTERVAL_MS);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, []);

  return { list, runningProcesses };
}

export type GameWatcher = ReturnType<typeof useGameWatcher>;
