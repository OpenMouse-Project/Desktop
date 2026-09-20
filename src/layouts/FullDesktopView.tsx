import { useEffect, useState } from "preact/hooks";
import { AppSidebar } from "../components/AppSidebar";
import { TitleBar } from "../components/TitleBar";
import { OverviewPage } from "../pages/OverviewPage";
import { GamesPage } from "../pages/GamesPage";
import { SettingsPage } from "../pages/SettingsPage";
import { SupportedPage } from "../pages/SupportedPage";
import { ToastHost } from "../components/ToastHost";
import { useMouseConnection } from "../hooks/use-mouse-connection";
import { useResourceMonitor } from "../hooks/use-resource-monitor";
import { useGameWatcher } from "../hooks/use-game-watcher";
import { enableStreamOverlay, isStreamOverlayEnabled } from "../lib/stream-overlay";

type Page = "overview" | "games" | "supported" | "settings";

export function FullDesktopView() {
  const [page, setPage] = useState<Page>("overview");
  const connection = useMouseConnection();
  const resourceMonitor = useResourceMonitor();
  const gameWatcher = useGameWatcher(connection);

  // The OBS overlay's on/off preference persists across restarts, but the
  // tiny_http server it runs on (stream_overlay.rs) does not — and the only
  // other place that started it was SettingsPage's own mount effect, which
  // doesn't fire at launch. So an OBS Browser Source was left pointing at a
  // dead URL after every relaunch until the user happened to open Settings,
  // showing a blank/error source on stream. Starting it here, next to the
  // other app-lifetime hooks, means it's live the moment the app is. Calling
  // it again from Settings is harmless (stream_overlay_start is idempotent).
  useEffect(() => {
    if (!isStreamOverlayEnabled()) return;
    void enableStreamOverlay().catch(() => {});
  }, []);

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
            ) : page === "supported" ? (
              <SupportedPage />
            ) : (
              <SettingsPage resourceMonitor={resourceMonitor} connection={connection} />
            )}
          </div>
        </div>
      </div>
      <ToastHost />
    </div>
  );
}
