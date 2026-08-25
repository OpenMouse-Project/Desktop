import { useEffect, useState } from "preact/hooks";
import { invoke } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { relaunch } from "@tauri-apps/plugin-process";
import type { Update } from "@tauri-apps/plugin-updater";
import { Bell, FileDown, ScrollText } from "lucide-preact";
import { ModeToggle } from "../components/ModeToggle";
import { ResourceMonitor } from "../components/ResourceMonitor";
import { ChangelogModal } from "../components/ChangelogModal";
import { UpdateAvailableModal } from "../components/UpdateAvailableModal";
import { showToast } from "../lib/toast";
import { runUpdateCheck } from "../lib/update-check";
import { showOverlayToast } from "../lib/overlay-toast";
import {
  CORNER_LABELS,
  SIZE_LABELS,
  getOverlaySettings,
  saveOverlaySettings,
  type OverlayCorner,
  type OverlaySettings,
  type OverlaySize,
} from "../lib/overlay-settings";
import { getVersion } from "@tauri-apps/api/app";
import type { ResourceMonitorData } from "../hooks/use-resource-monitor";


type AppMode = "bridge" | "full-desktop";
const DISCORD_RPC_PREFERENCE = "openmouse.discord-rpc.enabled";

interface Props {
  mode: AppMode;
  onModeChange: (mode: AppMode) => void;
  /** Omitted in Bridge Mode's compact popover — no room for sparkline charts there. */
  resourceMonitor?: ResourceMonitorData;
}

export function SettingsPage({ mode, onModeChange, resourceMonitor }: Props) {
  const [exporting, setExporting] = useState(false);
  const [checkingForUpdates, setCheckingForUpdates] = useState(false);
  const [version, setVersion] = useState("");
  const [discordEnabled, setDiscordEnabled] = useState(
    () => localStorage.getItem(DISCORD_RPC_PREFERENCE) === "true",
  );
  const [discordError, setDiscordError] = useState("");
  const [showChangelog, setShowChangelog] = useState(false);
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);
  const [installingUpdate, setInstallingUpdate] = useState(false);
  const [overlaySettings, setOverlaySettings] = useState<OverlaySettings>(() => getOverlaySettings());

  useEffect(() => {
    void getVersion().then(setVersion);
  }, []);

  // Every hid.rs diagnostic line (device open/close, HID++ traffic, decoded
  // errors — see src-tauri/src/applog.rs) is captured in a ring buffer as it
  // happens, not just printed to whatever terminal `npm run tauri dev`
  // happened to be running in. This writes that buffer to a real file and
  // reveals it, so a bug report has something concrete to attach.
  async function downloadLogs() {
    setExporting(true);
    try {
      const path = await invoke<string>("export_logs");
      await revealItemInDir(path);
      showToast(`Logs saved to ${path}`, "success");
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error), "error");
    } finally {
      setExporting(false);
    }
  }

  // Reuses lib/update-check.ts's shared check (also driving the TitleBar
  // pill and the 30-minute background poll) rather than calling the plugin
  // directly, so a manual click here doesn't run a second, disconnected
  // check that could disagree with what the pill is showing.
  //
  // Only checks here — CONFIRMED a real problem, not hypothetical: this
  // used to go straight from check() into downloadAndInstall() + relaunch()
  // the instant it found anything, so clicking a button labeled "check"
  // could close the app out from under the user a few seconds later with no
  // chance to back out. Finding an update now just opens
  // UpdateAvailableModal; installing is that modal's own explicit button.
  async function checkForUpdates() {
    setCheckingForUpdates(true);
    try {
      const update = await runUpdateCheck();
      if (!update) {
        showToast("You're up to date.", "info");
        return;
      }
      setPendingUpdate(update);
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error), "error");
    } finally {
      setCheckingForUpdates(false);
    }
  }

  async function installPendingUpdate() {
    if (!pendingUpdate) return;
    setInstallingUpdate(true);
    try {
      await pendingUpdate.downloadAndInstall();
      showToast("Update installed — restarting…", "success");
      await relaunch();
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error), "error");
      setInstallingUpdate(false);
    }
  }

  // OverlayApp.tsx re-reads this from localStorage on every show (both
  // windows share the same origin) — a change here takes effect on the
  // very next game-switch toast, no restart needed.
  function updateOverlaySettings(patch: Partial<OverlaySettings>) {
    const next = { ...overlaySettings, ...patch };
    setOverlaySettings(next);
    saveOverlaySettings(next);
  }

  function testOverlay() {
    void showOverlayToast({
      title: "Test notification",
      body: "This is what a game-switch alert looks like.",
      kind: "info",
    });
  }

  async function toggleDiscordRpc(enabled: boolean) {
    setDiscordError("");
    setDiscordEnabled(enabled);

    try {
      if (enabled) {
        await invoke("enable");
        localStorage.setItem(DISCORD_RPC_PREFERENCE, "true");
      } else {
        await invoke("disable");
        localStorage.removeItem(DISCORD_RPC_PREFERENCE);
      }
    } catch (error) {
      try {
        await invoke("disable");
      } catch {
        // The original connection error is the useful message for the panel.
      }
      setDiscordError(
        enabled
          ? `Discord RPC is enabled but unavailable. Could not connect: ${error instanceof Error ? error.message : String(error)}`
          : error instanceof Error
            ? error.message
            : String(error),
      );
    }
  }

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
          <input type="checkbox" />
          <span class="switch-track" />
        </label>
      </div>

      <div class="setting-row">
        <div class="setting-label">
          <span class="setting-title discord-setting-title">
            Discord Rich Presence
            {discordError && (
              <span
                class="setting-error-badge"
                data-tooltip={discordError}
                aria-label={`Discord RPC error: ${discordError}`}
                tabIndex={0}
              >
                Error
              </span>
            )}
          </span>
          <span class="setting-description">
            Show your current device and game in Discord. (Requires Discord to be running.)
          </span>
        </div>
        <label class="switch">
          <input
            type="checkbox"
            checked={discordEnabled}
            onChange={(event) => void toggleDiscordRpc(event.currentTarget.checked)}
          />
          <span class="switch-track" />
        </label>
      </div>

      <div class="setting-row">
        <div class="setting-label">
          <span class="setting-title">Theme</span>
          <span class="setting-description">
            Personalize the app's appearance.
          </span>
        </div>
        <label class="theme-dropdown">
          <select>
            <option>Emerald</option>
            <option>Violet</option>
            <option>Ice</option>
            <option>Emerald</option>
            <option>Mono</option>
          </select>
        </label>
      </div>

      <hr class="settings-divider" />

      <div class="setting-row">
        <div class="setting-label">
          <span class="setting-title">Check for Updates</span>
          
          <span class="setting-description">
            Check for a new version of the app.
            
          </span>
          <span class="settings-version">Current Version: v{version}</span>
        </div>
        
        <button class="rescan-button" onClick={() => void checkForUpdates()} disabled={checkingForUpdates}>
          {checkingForUpdates ? "Checking..." : "Check Now"}
        </button>

      </div>

      <div class="setting-row">
        <div class="setting-label">
          <span class="setting-title">Changelog</span>
          <span class="setting-description">
            What's changed in each version.
          </span>
        </div>
        <button class="rescan-button" onClick={() => setShowChangelog(true)}>
          <ScrollText size={14} /> View
        </button>
      </div>

      <div class="setting-row setting-row-block">
        <div class="setting-label">
          <span class="setting-title">Game-switch overlay</span>
          <span class="setting-description">
            A small always-on-top notice when a game profile applies or restores — visible even while a game is fullscreen.
          </span>
        </div>
        <div class="overlay-settings-controls">
          <div class="overlay-settings-row">
            <span class="setting-eyebrow">Position</span>
            <div class="segmented-group">
              {(Object.keys(CORNER_LABELS) as OverlayCorner[]).map((corner) => (
                <button
                  key={corner}
                  class={corner === overlaySettings.corner ? "active" : ""}
                  onClick={() => updateOverlaySettings({ corner })}
                >
                  {CORNER_LABELS[corner]}
                </button>
              ))}
            </div>
          </div>
          <div class="overlay-settings-row">
            <span class="setting-eyebrow">Size</span>
            <div class="segmented-group">
              {(Object.keys(SIZE_LABELS) as OverlaySize[]).map((size) => (
                <button
                  key={size}
                  class={size === overlaySettings.size ? "active" : ""}
                  onClick={() => updateOverlaySettings({ size })}
                >
                  {SIZE_LABELS[size]}
                </button>
              ))}
            </div>
          </div>
          <button class="rescan-button" onClick={testOverlay}>
            <Bell size={14} /> Test
          </button>
        </div>
      </div>

      <div class="setting-row">
        <div class="setting-label">
          <span class="setting-title">Logs</span>
          <span class="setting-description">
            Save this session's device connection logs to a file, for troubleshooting or a bug report.
          </span>
        </div>
        <button class="rescan-button" onClick={() => void downloadLogs()} disabled={exporting}>
          <FileDown size={14} /> {exporting ? "Saving…" : "Download Logs"}
        </button>
      </div>

      {resourceMonitor && (
        <div class="setting-row setting-row-block">
          <div class="setting-label">
            <span class="setting-title">Resource usage</span>
            <span class="setting-description">
              This app's own CPU and memory use.
            </span>
          </div>
          <ResourceMonitor data={resourceMonitor} />
        </div>
      )}

      {showChangelog && <ChangelogModal onClose={() => setShowChangelog(false)} />}

      {pendingUpdate && (
        <UpdateAvailableModal
          update={pendingUpdate}
          installing={installingUpdate}
          onInstall={() => void installPendingUpdate()}
          onDismiss={() => setPendingUpdate(null)}
        />
      )}
    </section>
  );
}
