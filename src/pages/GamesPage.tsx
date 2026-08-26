import { useEffect, useState } from "preact/hooks";
import { SlidersHorizontal, Zap, Download } from "lucide-preact";
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
  const installedGames = games.filter((g) => g.installed);
  const notInstalledGames = games.filter((g) => !g.installed);

  // Full page, not a modal over the grid — same pattern OverviewPage uses
  // for its list/device split, so editing a profile gets the same room a
  // device dashboard gets (hero art, device picker, DPI/rate controls)
  // instead of being squeezed into a small overlay.
  if (editingGame) {
    return (
      <GameProfilePanel
        key={editingGame.id}
        game={editingGame}
        connection={connection}
        onClose={() => setEditingGame(null)}
      />
    );
  }

  return (
    <section class="page page-games">
      <h1 class="page-title">Games</h1>
      {loading && <p class="page-description">Scanning for installed games...</p>}
      {error && <p class="games-error">{error}</p>}
      {!loading && !error && games.length === 0 && (
        <p class="page-description">No games found on this system.</p>
      )}
      {!loading && !error && installedGames.length > 0 && (
        <>
          <h2 class="games-section-title">Installed ({installedGames.length})</h2>
          <div class="games-grid">
            {installedGames.map((game) => (
              <GameCard
                key={game.id}
                game={game}
                running={isRunning(game)}
                profile={profiles[game.id]}
                onEditProfile={() => setEditingGame(game)}
              />
            ))}
          </div>
        </>
      )}
      {!loading && !error && notInstalledGames.length > 0 && (
        <>
          <h2 class="games-section-title games-section-more">More Games ({notInstalledGames.length})</h2>
          <div class="games-grid games-grid-muted">
            {notInstalledGames.map((game) => (
              <GameCard
                key={game.id}
                game={game}
                running={isRunning(game)}
                profile={profiles[game.id]}
                onEditProfile={() => setEditingGame(game)}
              />
            ))}
          </div>
        </>
      )}
    </section>
  );
}

function GameCard({
  game,
  running,
  profile,
  onEditProfile,
}: {
  game: Game;
  running: boolean;
  profile?: GameProfile;
  onEditProfile: () => void;
}) {
  return (
    <article
      class={`game-card ${running ? "detected" : ""} ${game.installed ? "" : "game-card-not-installed"}`}
      key={game.id}
      onClick={onEditProfile}
      role="button"
      tabIndex={0}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onEditProfile();
        }
      }}
    >
      <div class="game-card-art-wrap">
        {game.artworkSecondary ? (
          <div class="game-card-art-split">
            <img src={game.artwork} alt="" loading="lazy" />
            <img src={game.artworkSecondary} alt="" loading="lazy" />
          </div>
        ) : game.steamAppId || game.artwork ? (
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
        {game.installed && !running && (
          <span class="game-card-installed-badge">
            <Download size={10} aria-hidden="true" /> Installed
          </span>
        )}
      </div>
      <div class="game-card-content">
        <div class="game-card-title-row">
          <h2>{game.name}</h2>
          <span class="game-card-profile-button" title="Configure a device profile for this game" aria-hidden="true">
            {profile?.autoApply && <Zap size={11} class="game-card-profile-auto-icon" aria-hidden="true" />}
            <SlidersHorizontal size={13} />
          </span>
        </div>
      </div>
    </article>
  );
}
