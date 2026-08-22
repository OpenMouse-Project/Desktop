import { ModeToggle } from "../components/ModeToggle";

type AppMode = "bridge" | "full-desktop";

interface Props {
  mode: AppMode;
  onModeChange: (mode: AppMode) => void;
}

export function SettingsPage({ mode, onModeChange }: Props) {
  return (
    <section class="page">
      <h1 class="page-title">Settings</h1>

      <div class="setting-row">
        <div class="setting-label">
          <span class="setting-title">Mode</span>
          <span class="setting-description">
            {mode === "bridge"
              ? "Minimal tray companion — game detection, battery alerts. Closing this window keeps it running in the tray."
              : "Full device configuration — DPI, polling rate, RGB, firmware."}
          </span>
        </div>
        <ModeToggle mode={mode} onChange={onModeChange} />
      </div>

      <div class="setting-row">
        <div class="setting-label">
          <span class="setting-title">Start Bridge Mode at login</span>
          <span class="setting-description">
            Launch the tray companion automatically when you sign in.
          </span>
        </div>
        <label class="switch">
          <input type="checkbox" disabled />
          <span class="switch-track" />
        </label>
      </div>

      <p class="placeholder">
        (placeholder — login-item registration not wired up yet)
      </p>
    </section>
  );
}
