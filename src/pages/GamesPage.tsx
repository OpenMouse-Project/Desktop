import { useEffect, useState } from "preact/hooks";
import { SlidersHorizontal, Zap } from "lucide-preact";
import type { MouseConnection } from "../hooks/use-mouse-connection";
import type { Game, GameWatcher } from "../hooks/use-game-watcher";
import { subscribeGameProfiles, type GameProfile } from "../lib/game-profiles";
import { GameProfilePanel } from "../components/GameProfilePanel";

export type { Game };

interface Props {
  connection: MouseConnection;
  watcher: GameWatcher;
}

export function GamesPage({ connection, watcher }: Props) {
  const { list, runningProcesses } = watcher;
  const [profiles, setProfiles] = useState<Record<string, GameProfile>>({});
  const [editingGame, setEditingGame] = useState<Game | null>(null);

  // Saved profiles live in module-level storage (lib/game-profiles.ts), not
  // component state — subscribe so this page re-renders (the auto-apply
  // badge below) whenever GameProfilePanel saves or clears one.
  useEffect(() => subscribeGameProfiles(setProfiles), []);

  function isRunning(game: Game): boolean {
    return game.executables.some((exe) => runningProcesses.has(exe.toLowerCase()));
  }

  const loading = list.status === "loading";
  const error = list.status === "error" ? list.message : "";
  const games = list.status === "loaded" ? list.games : [];

  return (
    <section class="page">
      <h1 class="page-title">Games</h1>
      {loading && <p class="page-description">Loading games...</p>}
      {error && <p class="games-error">{error}</p>}
      {!loading && !error && games.length === 0 && (
        <p class="page-description">No games have been added yet.</p>
      )}
      {!loading && !error && games.length > 0 && (
        <div class="games-grid">
          {games.map((game) => {
            const running = isRunning(game);
            return (
              <article class={`game-card ${running ? "detected" : ""}`} key={game.id}>
                <div class="game-card-art-wrap">
                  {game.steamAppId || game.artwork ? (
                    <img
                      class="game-card-art"
                      src={
                        game.artwork ??
                        `https://cdn.cloudflare.steamstatic.com/steam/apps/${game.steamAppId}/library_600x900_2x.jpg`
                      }
                      alt=""
                      loading="lazy"
                      onError={(event) => {
                        if (game.artworkFallback && event.currentTarget.src !== game.artworkFallback) {
                          event.currentTarget.src = game.artworkFallback;
                        } else {
                          event.currentTarget.style.display = "none";
                        }
                      }}
                    />
                  ) : (
                    <div class="game-card-mark" aria-hidden="true">
                      {game.name.slice(0, 1)}
                    </div>
                  )}
                  {running && (
                    <span class="game-card-running-badge">
                      <span class="game-card-running-dot" aria-hidden="true" /> Running
                    </span>
                  )}
                </div>
                <div class="game-card-content">
                  <div class="game-card-title-row">
                    <h2>{game.name}</h2>
                    <button
                      class="game-card-profile-button"
                      title="Configure a device profile for this game"
                      onClick={() => setEditingGame(game)}
                    >
                      {profiles[game.id]?.autoApply && <Zap size={11} class="game-card-profile-auto-icon" aria-hidden="true" />}
                      <SlidersHorizontal size={13} />
                    </button>
                  </div>
                  <span class="game-card-executables">
                    {game.executables.join(" · ")}
                  </span>
                </div>
              </article>
            );
          })}
        </div>
      )}

      {editingGame && (
        // Keyed on the game id so clicking a different card's profile
        // button while the panel is already open re-mounts it with a fresh
        // draft instead of reusing the previous game's typed-in state.
        <GameProfilePanel key={editingGame.id} game={editingGame} connection={connection} onClose={() => setEditingGame(null)} />
      )}
    </section>
  );
}
