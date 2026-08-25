import { useState } from "preact/hooks";
import { Sidebar } from "../components/Sidebar";
import { Header } from "../components/Header";
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
  // Shared with Sidebar/Header so the whole shell reflects one connection,
  // not just whichever page happens to be showing.
  const connection = useMouseConnection();
  // Owned here, not by SettingsPage/ResourceMonitor, so the poll keeps
  // running and history/stats keep accumulating across page navigation —
  // see use-resource-monitor.ts's own docs.
  const resourceMonitor = useResourceMonitor();
  // Also owned here, not by GamesPage — see use-game-watcher.ts's own docs
  // on why launch/close detection needs to keep running across page
  // navigation, same reasoning as the resource monitor above.
  const gameWatcher = useGameWatcher(connection);

  return (
    <div class="full-desktop-shell">
      <TitleBar />
      <div class="full-desktop-view">
        <Sidebar
          connected={connection.connected}
          onSelectDevice={() => {
            setPage("overview");
            connection.viewConnectedDevice();
          }}
        />
        <div class="full-desktop-main">
          <Header page={page} onNavigate={setPage} connected={connection.connected} />
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
