import { useState } from "preact/hooks";
import { AppSidebar } from "../components/AppSidebar";
import { TitleBar } from "../components/TitleBar";
import { OverviewPage } from "../pages/OverviewPage";
import { GamesPage } from "../pages/GamesPage";
import { SettingsPage } from "../pages/SettingsPage";
import { ToastHost } from "../components/ToastHost";
import { useMouseConnection } from "../hooks/use-mouse-connection";
import { useResourceMonitor } from "../hooks/use-resource-monitor";
import { useGameWatcher } from "../hooks/use-game-watcher";

type AppMode = "bridge" | "full-desktop";
type Page = "overview" | "games" | "settings";

interface Props {
  mode: AppMode;
  onModeChange: (mode: AppMode) => void;
}

export function FullDesktopView({ mode, onModeChange }: Props) {
  const [page, setPage] = useState<Page>("overview");
  const connection = useMouseConnection();
  const resourceMonitor = useResourceMonitor();
  const gameWatcher = useGameWatcher(connection);

  return (
    <div class="full-desktop-shell">
      <TitleBar />
      <div class="full-desktop-view">
        <AppSidebar page={page} onNavigate={setPage} />
        <div class="full-desktop-main">
          <div class="full-desktop-content">
            {page === "overview" ? (
              <OverviewPage connection={connection} activeGameOverride={gameWatcher.activeOverride} />
            ) : page === "games" ? (
              <GamesPage connection={connection} watcher={gameWatcher} />
            ) : (
              <SettingsPage mode={mode} onModeChange={onModeChange} resourceMonitor={resourceMonitor} />
            )}
          </div>
        </div>
      </div>
      <ToastHost />
    </div>
  );
}
