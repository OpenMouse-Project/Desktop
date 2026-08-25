import { useEffect, useState } from "preact/hooks";
import { invoke } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { FileDown } from "lucide-preact";
import { ModeToggle } from "../components/ModeToggle";
import { ResourceMonitor } from "../components/ResourceMonitor";
import { showToast } from "../lib/toast";
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

  async function checkForUpdates() {
    setCheckingForUpdates(true);
    try {
      await invoke("check_for_updates");
    } finally {
      setCheckingForUpdates(false);
    }
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
    </section>
  );
}
