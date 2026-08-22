import { useState } from "preact/hooks";
import { Settings, X } from "lucide-preact";
import { SettingsPage } from "../pages/SettingsPage";

type AppMode = "bridge" | "full-desktop";
type Page = "status" | "settings";

interface Props {
  mode: AppMode;
  onModeChange: (mode: AppMode) => void;
}

export function BridgeView({ mode, onModeChange }: Props) {
  const [page, setPage] = useState<Page>("status");

  return (
    <div class="bridge-view">
      <header class="bridge-header">
        <span class="brand-mark" aria-hidden="true">
          Ʃ
        </span>
        <span class="brand-name">OpenMouse Bridge</span>
        <button
          class="bridge-settings-button"
          title="Settings"
          onClick={() => setPage(page === "settings" ? "status" : "settings")}
        >
          {page === "settings" ? <X size={14} /> : <Settings size={14} />}
        </button>
      </header>

      {page === "settings" ? (
        <div class="bridge-settings">
          <SettingsPage mode={mode} onModeChange={onModeChange} />
        </div>
      ) : (
        <>
          <div class="bridge-status">
            <p class="bridge-status-title">No mice detected</p>
            <p class="bridge-status-hint">Watching for supported devices…</p>
          </div>

          <div class="bridge-list">
            <div class="bridge-list-row">
              <span>Game detection</span>
              <span class="pill pill-off">Off</span>
            </div>
            <div class="bridge-list-row">
              <span>Battery alerts</span>
              <span class="pill pill-off">Off</span>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
