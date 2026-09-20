// Owned once by FullDesktopView (same pattern as useResourceMonitor) rather
// than by GamesPage, so game-launch/close detection — and therefore
// auto-applying/restoring a saved profile — keeps running no matter which
// tab is showing. Games are usually played full-screen with this app tabbed
// away, not sitting open on the Games page, so a poll that only ran while
// GamesPage was mounted would never catch the moment that mattered.
//
// Every apply/restore fires BOTH the in-window toast (ToastHost, mounted
// once in FullDesktopView — useless the moment this window isn't visible,
// i.e. exactly while a game is actually running) AND the always-on-top
// overlay toast (lib/overlay-toast.ts, its own separate window — see
// OverlayApp.tsx) that's visible even then.

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
import { showOverlayToast } from "../lib/overlay-toast";

export interface Game {
  id: string;
  artwork?: string;
  name: string;
  steamAppId?: number;
  /** Epic manifest `AppName` — Epic's own stable per-title slug. */
  epicId?: string;
  /** Slug derived from the Riot Client's install path (e.g. "valorant",
   *  "league_of_legends") — see scan_riot() in games.rs. */
  riotId?: string;
  artworkFallback?: string;
  /** Second cover to show side-by-side with `artwork` on the card — for a
   *  card that represents two titles sharing one install/executable set
   *  (e.g. League of Legends / TFT). */
  artworkSecondary?: string;
  executables: string[];
  installed?: boolean;
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
// Slower cadence while the window is hidden: minimized, hidden to the tray,
// or covered by a fullscreen game. Same reasoning as use-mouse-connection.ts's
// own hidden cadence — this hook's whole reason for living above the page
// (see the module docs) is to act while the app is tabbed away, and the
// overlay toast exists for the case where this window isn't visible, so a
// hidden window must not stop launch/close detection; it just doesn't need
// to look as often. Skipping outright was a real bug: with the app closed to
// the tray — which is how it's meant to be used while gaming — both the
// launch and the quit were missed, so the profile never applied and the
// pre-game settings were never restored.
const HIDDEN_POLL_INTERVAL_MS = 15_000;

// Hosted straight out of this (public) repo via jsDelivr's GitHub CDN, so
// adding/editing a game is just a commit+push to games.json — no app
// rebuild or release. jsDelivr fronts raw.githubusercontent.com with real
// caching (~12-24h TTL, purgeable), unlike raw GitHub's tight unauthenticated
// rate limits. Falls back to the bundled /games.json (shipped in public/,
// same file) if the network's unavailable or the CDN is unreachable, so the
// Games page still works offline / on first run before any fetch succeeds.
const REMOTE_GAMES_URL = "https://cdn.jsdelivr.net/gh/OpenMouse-Project/Desktop@main/public/games.json";
const LOCAL_GAMES_URL = "/games.json";

// Best-effort — recorded into the same ring buffer Settings' "Download
// Logs" button exports, so a report of "my new game isn't showing up" is
// diagnosable (remote fetch failed vs. served stale-but-successful vs.
// fell back to the bundled copy) without needing DevTools on a production
// build.
function logLine(line: string) {
  void invoke("log_line", { line: `[games] ${line}` }).catch(() => {});
}

async function fetchGamesFile(): Promise<GamesFile> {
  try {
    // `cache: "no-store"` bypasses WebView2/WebKit's own local HTTP cache,
    // which otherwise honors the CDN's Cache-Control max-age and keeps
    // serving whatever it first fetched for up to a week — independent of,
    // and surviving past, any jsDelivr edge purge. This is the one request
    // per launch this hook makes; the CDN's own edge caching still does the
    // real work of not hammering the origin repo.
    const response = await fetch(REMOTE_GAMES_URL, { cache: "no-store" });
    if (!response.ok) throw new Error(`Could not load games (${response.status})`);
    const data = (await response.json()) as GamesFile;
    logLine(`loaded ${data.games.length} games from remote CDN (${REMOTE_GAMES_URL})`);
    return data;
  } catch (error) {
    // Remote fetch failed (offline, DNS, CDN hiccup) — fall back to the
    // copy bundled with the app itself.
    const message = error instanceof Error ? error.message : String(error);
    logLine(`remote fetch failed (${message}), falling back to bundled ${LOCAL_GAMES_URL}`);
    const response = await fetch(LOCAL_GAMES_URL);
    if (!response.ok) throw new Error(`Could not load games (${response.status})`);
    const data = (await response.json()) as GamesFile;
    logLine(`loaded ${data.games.length} games from bundled local copy`);
    return data;
  }
}

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
  gameName: string;
  restore: Pick<GameProfile, "dpi" | "dpiY" | "pollingRateHz" | "liftOffDistance" | "gamingSurfaceMode">;
}

/** What OverviewPage needs to know: a profile is in control, and whose. */
export interface ActiveGameOverride {
  gameId: string;
  gameName: string;
}

export function useGameWatcher(connection: MouseConnection) {
  const [list, setList] = useState<GamesListState>({ status: "loading" });
  const [runningProcesses, setRunningProcesses] = useState<Set<string>>(new Set());
  // Mirrors activeOverrideRef below for rendering — OverviewPage locks the
  // Performance tab and shows which game is in control off this. The ref
  // stays the source of truth for the async poll logic (avoids stale
  // closures the way every other piece of state here does); this is purely
  // the render-triggering copy of it.
  const [activeOverride, setActiveOverride] = useState<ActiveGameOverride | null>(null);

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
  // When the last scan actually ran, so the hidden cadence above is measured
  // from real scans rather than from interval ticks.
  const lastScanAtRef = useRef(0);
  // Games whose launch edge was seen while no device was controllable — see
  // `applyOnLaunch`'s return value. Retried by `poll()` until they land (or
  // the game closes first), instead of being dropped for the whole session.
  const pendingLaunchRef = useRef<Set<string>>(new Set());

  useEffect(() => {
    fetchGamesFile()
      .then(async (data) => {
        // Collect each launcher's known IDs from the games list
        const knownSteamIds = data.games
          .filter((g) => g.steamAppId)
          .map((g) => Number(g.steamAppId));
        const knownEpicIds = data.games.filter((g) => g.epicId).map((g) => g.epicId!);
        const knownRiotIds = data.games.filter((g) => g.riotId).map((g) => g.riotId!);

        // Scan system for installed games via Steam/Epic/Riot
        const scanResult = await invoke<{
          installed_steam_ids: number[];
          installed_epic_ids: string[];
          installed_riot_ids: string[];
        }>("scan_installed_games", { knownSteamIds, knownEpicIds, knownRiotIds });

        const installedSteamSet = new Set(scanResult.installed_steam_ids);
        const installedEpicSet = new Set(scanResult.installed_epic_ids);
        const installedRiotSet = new Set(scanResult.installed_riot_ids);
        const gamesWithInstallStatus = data.games.map((game) => ({
          ...game,
          installed:
            (!!game.steamAppId && installedSteamSet.has(Number(game.steamAppId))) ||
            (!!game.epicId && installedEpicSet.has(game.epicId)) ||
            (!!game.riotId && installedRiotSet.has(game.riotId)),
        }));

        // Installed games first, then not-installed
        const installed = gamesWithInstallStatus.filter((g) => g.installed);
        const notInstalled = gamesWithInstallStatus.filter((g) => !g.installed);
        const allGames = [...installed, ...notInstalled];

        gamesRef.current = allGames;
        setList({ status: "loaded", games: allGames });
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

    // Fires both notification surfaces together rather than duplicating
    // the call at every site below — see the module docs on why both
    // exist. Same text for both: the overlay toast is a single truncated
    // line (OverlayToastPayload's own docs), so there's no separate
    // title/body split to make here either.
    function notify(text: string, kind: "success" | "error" | "info") {
      showToast(text, kind);
      void showOverlayToast({ text, kind });
    }

    /**
     * Applies a launched game's profile. Returns `false` only when the launch
     * couldn't be acted on *yet* — no device is controllable at this instant
     * (mouse asleep, receiver pulled, nothing connected) — which is the
     * caller's cue to keep it pending and retry rather than count the game as
     * handled. A profile that isn't set to auto-apply, or a write that fails,
     * both count as handled: retrying those would just repeat the same no-op
     * or the same error toast every few seconds.
     */
    async function applyOnLaunch(game: Game): Promise<boolean> {
      const profile = getGameProfile(game.id);
      if (!profile?.autoApply || !isProfileMeaningful(profile)) return true;
      const { connected, connectedInfo, patchStatus } = connectionRef.current;
      if (!connectedInfo || !connected) return false;

      // The first game to take over is the one whose "before" snapshot
      // matters — that's what "default" means here. A second game taking
      // over from a first (still-running) one just changes who's in
      // control, not what gets restored afterward.
      const restore = activeOverrideRef.current
        ? activeOverrideRef.current.restore
        : {
            dpi: profile.dpi !== undefined ? connected.status.dpi : undefined,
            // Y is snapshotted whenever X is, not only when the profile names
            // a Y: LogitechHidppClient.setDpi(dpi, dpiY = dpi) defaults Y to
            // X, so a dpi-only profile overwrites Y too. Leaving it out of
            // the snapshot would restore the pre-game *X* onto Y afterwards,
            // silently changing the Y axis on a separate-axes mouse.
            dpiY: profile.dpi !== undefined ? connected.status.dpiY : undefined,
            pollingRateHz: profile.pollingRateHz !== undefined ? connected.status.pollingRateHz : undefined,
            liftOffDistance: profile.liftOffDistance !== undefined ? connected.status.liftOffDistance ?? undefined : undefined,
            gamingSurfaceMode: profile.gamingSurfaceMode !== undefined ? connected.status.gamingSurfaceMode ?? undefined : undefined,
          };

      try {
        await applyGameProfile(connectedInfo, profile, patchStatus);
        // Only actually take over — locking the Performance tab, showing
        // the "in control" badge — once the write is confirmed to have
        // reached the mouse. A failed apply shouldn't lock the UI for
        // something that never actually happened.
        activeOverrideRef.current = { gameId: game.id, gameName: game.name, restore };
        setActiveOverride({ gameId: game.id, gameName: game.name });
        notify(`Applied "${game.name}" profile — ${describeProfile(profile)}.`, "success");
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        notify(`Couldn't apply "${game.name}" profile: ${message}`, "error");
      }
      return true;
    }

    /**
     * Puts the pre-game settings back when the game that took the mouse over
     * closes. Returns `false` when there's no controllable device at this
     * instant, leaving `activeOverrideRef` set so the caller retries — the
     * override must NOT be cleared before the restore has actually landed:
     * clearing it early (which is what this did) both marked the UI as
     * restored while the mouse still had the game's DPI/polling/lift-off and
     * threw away the only record of what to put back.
     */
    async function restoreOnClose(gameId: string, gameName: string): Promise<boolean> {
      const active = activeOverrideRef.current;
      if (!active || active.gameId !== gameId) return true;

      const { connectedInfo, patchStatus } = connectionRef.current;
      if (!connectedInfo) return false;

      const clear = () => {
        activeOverrideRef.current = null;
        setActiveOverride(null);
      };

      const restoreProfile: GameProfile = { ...active.restore, autoApply: false };
      // Nothing worth writing back (the profile's fields were all unset) —
      // the override is done either way.
      if (!isProfileMeaningful(restoreProfile)) {
        clear();
        return true;
      }

      try {
        await applyGameProfile(connectedInfo, restoreProfile, patchStatus);
        clear();
        notify(`${gameName} closed — restored your default ${describeProfile(restoreProfile)}.`, "info");
      } catch (error) {
        // A write that genuinely failed is surfaced rather than retried: the
        // retry below is only for "no device to write to yet", and repeating
        // a real error would just re-toast it every poll.
        clear();
        const message = error instanceof Error ? error.message : String(error);
        notify(`Couldn't restore your default settings: ${message}`, "error");
      }
      return true;
    }

    async function poll() {
      // Hidden (minimized, tray, occluded by a fullscreen game) slows the
      // scan down instead of stopping it — see HIDDEN_POLL_INTERVAL_MS.
      if (document.hidden && Date.now() - lastScanAtRef.current < HIDDEN_POLL_INTERVAL_MS) return;
      lastScanAtRef.current = Date.now();
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

        // A `false` return means "no device to write to right now" — the
        // override stays set and the catch-up below retries it.
        for (const game of justClosed) await restoreOnClose(game.id, game.name);
        for (const game of justLaunched) {
          if (!(await applyOnLaunch(game))) pendingLaunchRef.current.add(game.id);
        }

        // Catch-up for work that had no controllable device at the moment it
        // was due (mouse asleep, receiver pulled, nothing connected yet).
        // Without this each case was simply lost for the rest of the session:
        // a launch never applied its profile, and a closed game's override
        // was dropped along with the only record of what to restore.
        const pending = activeOverrideRef.current;
        if (pending && !nowRunning.has(pending.gameId)) {
          await restoreOnClose(pending.gameId, pending.gameName);
        }
        if (pendingLaunchRef.current.size > 0) {
          for (const game of games) {
            if (!pendingLaunchRef.current.has(game.id)) continue;
            // The game exited before a device showed up — nothing to apply.
            if (!nowRunning.has(game.id)) {
              pendingLaunchRef.current.delete(game.id);
              continue;
            }
            // applyOnLaunch reads the "before" state off the device at the
            // moment it runs, so a retry captures whatever the mouse is set
            // to when it first becomes reachable again — the best available
            // answer, since there was nothing to read at the launch edge.
            if (await applyOnLaunch(game)) pendingLaunchRef.current.delete(game.id);
          }
        }
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

  return { list, runningProcesses, activeOverride };
}

export type GameWatcher = ReturnType<typeof useGameWatcher>;
