import { useState } from "preact/hooks";
import { Sidebar } from "../components/Sidebar";
import { Header } from "../components/Header";
import { TitleBar } from "../components/TitleBar";
import { OverviewPage } from "../pages/OverviewPage";
import { SettingsPage } from "../pages/SettingsPage";

type AppMode = "bridge" | "full-desktop";
type Page = "overview" | "settings";

interface Props {
  mode: AppMode;
  onModeChange: (mode: AppMode) => void;
}

export function FullDesktopView({ mode, onModeChange }: Props) {
  const [page, setPage] = useState<Page>("overview");

  return (
    <div class="full-desktop-shell">
      <TitleBar />
      <div class="full-desktop-view">
        <Sidebar />
        <div class="full-desktop-main">
          <Header page={page} onNavigate={setPage} />
          <div class="full-desktop-content">
            {page === "overview" ? (
              <OverviewPage />
            ) : (
              <SettingsPage mode={mode} onModeChange={onModeChange} />
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
