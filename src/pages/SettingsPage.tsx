import { useEffect, useState } from "preact/hooks";
import { invoke } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { relaunch } from "@tauri-apps/plugin-process";
import type { Update } from "@tauri-apps/plugin-updater";
import { Bell, Copy, FileDown, ScrollText } from "lucide-preact";
import { ResourceMonitor } from "../components/ResourceMonitor";
import { ChangelogModal } from "../components/ChangelogModal";
import { UpdateAvailableModal } from "../components/UpdateAvailableModal";
import { showToast } from "../lib/toast";
import { runUpdateCheck } from "../lib/update-check";
import { showOverlayToast } from "../lib/overlay-toast";
import { CORNER_LABELS, getOverlaySettings, saveOverlaySettings, type OverlayCorner, type OverlaySettings } from "../lib/overlay-settings";
import { getVersion } from "@tauri-apps/api/app";
import { disable as disableAutostart, enable as enableAutostart, isEnabled as isAutostartEnabled } from "@tauri-apps/plugin-autostart";
import type { ResourceMonitorData } from "../hooks/use-resource-monitor";
import {
  applyDynamicVibrancy,
  getDynamicVibrancy,
  getThemeState,
  saveDynamicVibrancy,
  saveThemeState,
  THEME_PRESETS,
  type ThemeState,
} from "../lib/themes";
import {
  applyPanelBlur,
  applyWindowOpacity,
  getPanelBlur,
  getWindowOpacity,
  savePanelBlur,
  saveWindowOpacity,
} from "../lib/window-opacity";

import type { MouseConnection } from "../hooks/use-mouse-connection";
import {
  buildStreamOverlayUrl,
  disableStreamOverlay,
  enableStreamOverlay,
  getStreamOverlayDeviceKey,
  getStreamOverlayFields,
  getStreamOverlayUrl,
  isStreamOverlayEnabled,
  saveStreamOverlayDeviceKey,
  saveStreamOverlayFields,
  type StreamOverlayFields,
} from "../lib/stream-overlay";


const DISCORD_RPC_PREFERENCE = "openmouse.discord-rpc.enabled";

// Short abbreviations, not the icons this had before (disliked) or the
// full "Top left"/"Bottom right" labels (too wide to reliably fit 4 in one
// row) — the full name is still there, as a tooltip. A plain flex row, not
// .segmented-group (a fixed 3-column
// grid built for 3-option pickers like Gaming Surface) — CONFIRMED the
// cause of a real bug: reusing it here wrapped the 4th corner onto its own
// row regardless of how narrow the buttons were.
const CORNER_OPTIONS: { corner: OverlayCorner; abbr: string }[] = [
  { corner: "top-left", abbr: "TL" },
  { corner: "top-right", abbr: "TR" },
  { corner: "bottom-left", abbr: "BL" },
  { corner: "bottom-right", abbr: "BR" },
];

interface Props {
  resourceMonitor: ResourceMonitorData;
  connection: MouseConnection;
}

export function SettingsPage({ resourceMonitor, connection }: Props) {
  const [exporting, setExporting] = useState(false);
  const [streamOverlayDeviceKey, setStreamOverlayDeviceKey] = useState<string | null>(() => getStreamOverlayDeviceKey());
  const overlayDeviceCandidates = connection.list.status === "loaded" ? connection.list.candidates : [];
  const [autostartEnabled, setAutostartEnabled] = useState(false);
  const [autostartError, setAutostartError] = useState("");
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
  const [theme, setTheme] = useState<ThemeState>(() => getThemeState());
  const [dynamicVibrancy, setDynamicVibrancyState] = useState<number>(() => getDynamicVibrancy());
  const [windowOpacity, setWindowOpacity] = useState<number>(() => getWindowOpacity());
  const [panelBlur, setPanelBlur] = useState<number>(() => getPanelBlur());
  const [themeEditorOpen, setThemeEditorOpen] = useState(false);
  const [streamOverlayEnabled, setStreamOverlayEnabled] = useState(() => isStreamOverlayEnabled());
  const [streamOverlayUrl, setStreamOverlayUrl] = useState<string | null>(null);
  const [streamOverlayError, setStreamOverlayError] = useState("");
  const [streamOverlayCopied, setStreamOverlayCopied] = useState(false);
  const [streamOverlayFields, setStreamOverlayFields] = useState<StreamOverlayFields>(() => getStreamOverlayFields());

  useEffect(() => {
    void getVersion().then(setVersion);
  }, []);

  useEffect(() => {
    isAutostartEnabled()
      .then(setAutostartEnabled)
      .catch((error) => setAutostartError(error instanceof Error ? error.message : String(error)));
  }, []);

  // Restarts the server on app launch if the user had it on last session —
  // the preference (localStorage) persists across restarts but the actual
  // tiny_http server (stream_overlay.rs) does not.
  useEffect(() => {
    if (!isStreamOverlayEnabled()) return;
    enableStreamOverlay()
      .then(setStreamOverlayUrl)
      .catch((error) => setStreamOverlayError(error instanceof Error ? error.message : String(error)));
  }, []);

  async function toggleStreamOverlay(enabled: boolean) {
    setStreamOverlayError("");
    setStreamOverlayEnabled(enabled);
    try {
      if (enabled) {
        setStreamOverlayUrl(await enableStreamOverlay());
      } else {
        await disableStreamOverlay();
        setStreamOverlayUrl(null);
      }
    } catch (error) {
      setStreamOverlayEnabled(!enabled);
      setStreamOverlayError(error instanceof Error ? error.message : String(error));
    }
  }

  async function copyStreamOverlayUrl() {
    const base = streamOverlayUrl ?? (await getStreamOverlayUrl().catch(() => null));
    if (!base) return;
    try {
      await navigator.clipboard.writeText(buildStreamOverlayUrl(base, streamOverlayFields));
      setStreamOverlayCopied(true);
      setTimeout(() => setStreamOverlayCopied(false), 1500);
    } catch {
      // Clipboard permission denied or unavailable — the URL is still
      // shown as plain text, so the user can select and copy it manually.
    }
  }

  function toggleStreamOverlayField(field: keyof StreamOverlayFields, value: boolean) {
    const next = { ...streamOverlayFields, [field]: value };
    setStreamOverlayFields(next);
    saveStreamOverlayFields(next);
  }

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
    void showOverlayToast({ text: "Test notification — this is what a game-switch alert looks like.", kind: "info" });
  }

  async function toggleAutostart(enabled: boolean) {
    setAutostartError("");
    setAutostartEnabled(enabled);
    try {
      if (enabled) await enableAutostart();
      else await disableAutostart();
    } catch (error) {
      setAutostartEnabled(!enabled);
      setAutostartError(error instanceof Error ? error.message : String(error));
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

      <div class="settings-section">
      <h2 class="settings-section-title">General</h2>
      <div class="setting-row">
        <div class="setting-label">
          <span class="setting-title discord-setting-title">
            Launch on startup
            {autostartError && (
              <span
                class="setting-error-badge"
                data-tooltip={autostartError}
                aria-label={`Launch on startup error: ${autostartError}`}
                tabIndex={0}
              >
                Error
              </span>
            )}
          </span>
          <span class="setting-description">
            Start OpenMouse automatically when you log in, minimized to the tray.
          </span>
        </div>
        <label class="switch">
          <input
            type="checkbox"
            checked={autostartEnabled}
            onChange={(event) => void toggleAutostart(event.currentTarget.checked)}
          />
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
      </div>

      <div class="settings-section">
      <h2 class="settings-section-title">Appearance</h2>
      <div class="setting-row setting-row-block">
        <div class="setting-label">
          <span class="setting-title">Theme</span>
          <span class="setting-description">
            Pick a preset accent or drop in your own CSS for a fully custom look.
          </span>
        </div>

        <div class="theme-preset-picker">
          {THEME_PRESETS.map((preset) => (
            <button
              key={preset.id}
              class={`theme-preset-swatch ${theme.presetId === preset.id ? "active" : ""}`}
              data-theme-preview={preset.id}
              title={preset.id === "dynamic" ? undefined : preset.description}
              aria-label={preset.label}
              onClick={() => {
                const next = { ...theme, presetId: preset.id };
                setTheme(next);
                saveThemeState(next);
                showToast(`Theme set to ${preset.label}.`, "success");
              }}
            >
              <span class="theme-preset-acc" aria-hidden="true" />
              <span class="theme-preset-label">{preset.label}</span>
              {preset.id === "dynamic" && (
                <span
                  class="info-badge"
                  data-tooltip={preset.description}
                  aria-label={preset.description}
                  onClick={(event) => event.stopPropagation()}
                >
                  i
                </span>
              )}
            </button>
          ))}
        </div>

        <div class="theme-custom">
          <button
            class="rescan-button"
            aria-expanded={themeEditorOpen}
            onClick={() => setThemeEditorOpen((open) => !open)}
          >
            {themeEditorOpen ? "Hide custom CSS" : "Custom CSS"}
            <span class="theme-custom-status">{theme.customCss ? "active" : "off"}</span>
          </button>

          {themeEditorOpen && (
            <div class="theme-custom-editor">
              <textarea
                class="theme-css-input"
                rows={8}
                spellcheck={false}
                placeholder={"/* Paste custom CSS here. It overrides everything. */\n/* e.g. a new accent: */\n:root {\n  --accent: #ff6b6b;\n}"}
                value={theme.customCss}
                onInput={(e) => setTheme({ ...theme, customCss: (e.target as HTMLTextAreaElement).value })}
              />
              <div class="theme-custom-actions">
                <span class="setting-description">
                  Applies live and is remembered across restarts (both windows).
                </span>
                <div>
                  <button
                    class="rescan-button"
                    disabled={theme.customCss.length === 0}
                    onClick={() => {
                      const next = { ...theme, customCss: "" };
                      setTheme(next);
                      saveThemeState(next);
                      showToast("Custom CSS cleared.", "info");
                    }}
                  >
                    Clear
                  </button>
                  <button
                    class="connect-button"
                    onClick={() => {
                      saveThemeState(theme);
                      showToast("Custom CSS applied.", "success");
                    }}
                  >
                    Apply
                  </button>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>

      <div class="setting-row setting-row-block">
        <div class="setting-label">
          <span class="setting-title">Window effects</span>
          <span class="setting-description">
            Transparency, blur, and (with the Dynamic theme) color intensity — all in one place.
          </span>
        </div>

        <div class="window-effects-group">
          <div class="window-effects-item">
            <span class="setting-eyebrow">Transparency</span>
            <div class="window-opacity-control">
              <input
                type="range"
                min={40}
                max={100}
                step={1}
                value={windowOpacity}
                onInput={(event) => {
                  const value = Number(event.currentTarget.value);
                  setWindowOpacity(value);
                  applyWindowOpacity(value);
                }}
                onChange={(event) => saveWindowOpacity(Number(event.currentTarget.value))}
              />
              <span class="window-opacity-value">{windowOpacity}%</span>
            </div>
            <span class="setting-description">Let a blurred view of your desktop show through the window — lower is more see-through.</span>
          </div>

          <div class="window-effects-item">
            <span class="setting-eyebrow">Blur</span>
            <div class="window-opacity-control">
              <input
                type="range"
                min={0}
                max={80}
                step={2}
                value={panelBlur}
                onInput={(event) => {
                  const value = Number(event.currentTarget.value);
                  setPanelBlur(value);
                  applyPanelBlur(value);
                }}
                onChange={(event) => savePanelBlur(Number(event.currentTarget.value))}
              />
              <span class="window-opacity-value">{panelBlur}px</span>
            </div>
            <span class="setting-description">Extra softening on top of the window's own native blur — low is crisp and nearly transparent, high washes it into a solid frosted haze.</span>
          </div>

          {theme.presetId === "dynamic" && (
            <div class="window-effects-item">
              <span class="setting-eyebrow">Vibrancy</span>
              <div class="window-opacity-control">
                <input
                  type="range"
                  min={0}
                  max={150}
                  step={1}
                  value={dynamicVibrancy}
                  onInput={(event) => {
                    const value = Number(event.currentTarget.value);
                    setDynamicVibrancyState(value);
                    applyDynamicVibrancy(value);
                  }}
                  onChange={(event) => saveDynamicVibrancy(Number(event.currentTarget.value))}
                />
                <span class="window-opacity-value">{dynamicVibrancy}%</span>
              </div>
              <span class="setting-description">How saturated the Dynamic theme's wallpaper-derived colors are — 0 is neutral gray, 100 is as calibrated, above that pushes further.</span>
            </div>
          )}
        </div>
      </div>
      </div>

      <div class="settings-section">
      <h2 class="settings-section-title">Updates</h2>
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
      </div>

      <div class="settings-section">
      <h2 class="settings-section-title">Streaming &amp; Overlays</h2>
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
            <div class="overlay-corner-picker">
              {CORNER_OPTIONS.map(({ corner, abbr }) => (
                <button
                  key={corner}
                  class={corner === overlaySettings.corner ? "active" : ""}
                  title={CORNER_LABELS[corner]}
                  aria-label={CORNER_LABELS[corner]}
                  onClick={() => updateOverlaySettings({ corner })}
                >
                  {abbr}
                </button>
              ))}
            </div>
          </div>
          <button class="rescan-button" onClick={testOverlay}>
            <Bell size={14} /> Test
          </button>
        </div>
      </div>

      <div class="setting-row setting-row-block">
        <div class="setting-label">
          <span class="setting-title discord-setting-title">
            OBS overlay
            {streamOverlayError && (
              <span
                class="setting-error-badge"
                data-tooltip={streamOverlayError}
                aria-label={`OBS overlay error: ${streamOverlayError}`}
                tabIndex={0}
              >
                Error
              </span>
            )}
          </span>
          <span class="setting-description">
            Runs a small local page showing your connected mouse — add it in OBS as a Browser Source.
          </span>
        </div>
        <div class="overlay-settings-controls">
          <label class="switch">
            <input
              type="checkbox"
              checked={streamOverlayEnabled}
              onChange={(event) => void toggleStreamOverlay(event.currentTarget.checked)}
            />
            <span class="switch-track" />
          </label>

          {streamOverlayEnabled && streamOverlayUrl && (
            <>
              {overlayDeviceCandidates.length > 1 && (
                <div class="overlay-settings-row">
                  <span class="setting-eyebrow">Mouse</span>
                  <select
                    class="overlay-device-select"
                    value={streamOverlayDeviceKey ?? ""}
                    onChange={(event) => {
                      const key = event.currentTarget.value || null;
                      setStreamOverlayDeviceKey(key);
                      saveStreamOverlayDeviceKey(key);
                    }}
                  >
                    <option value="">Whichever mouse is active</option>
                    {overlayDeviceCandidates.map((candidate) => (
                      <option key={candidate.info.key} value={candidate.info.key}>
                        {candidate.info.productString || candidate.brands.join(" / ")}
                      </option>
                    ))}
                  </select>
                </div>
              )}

              <div class="overlay-settings-row">
                <span class="setting-eyebrow">Fields</span>
                <div class="stream-overlay-field-picker">
                  <label>
                    <input
                      type="checkbox"
                      checked={streamOverlayFields.name}
                      onChange={(e) => toggleStreamOverlayField("name", e.currentTarget.checked)}
                    />
                    Device name
                  </label>
                  <label>
                    <input
                      type="checkbox"
                      checked={streamOverlayFields.dpi}
                      onChange={(e) => toggleStreamOverlayField("dpi", e.currentTarget.checked)}
                    />
                    DPI
                  </label>
                  <label>
                    <input
                      type="checkbox"
                      checked={streamOverlayFields.polling}
                      onChange={(e) => toggleStreamOverlayField("polling", e.currentTarget.checked)}
                    />
                    Polling rate
                  </label>
                </div>
              </div>

              <div class="overlay-settings-row">
                <span class="setting-eyebrow">Preview</span>
                <div class="stream-overlay-preview">
                  <iframe
                    key={buildStreamOverlayUrl(streamOverlayUrl, streamOverlayFields)}
                    src={buildStreamOverlayUrl(streamOverlayUrl, streamOverlayFields)}
                    title="OBS overlay preview"
                  />
                </div>
              </div>

              <div class="overlay-settings-row">
                <span class="setting-eyebrow">Browser Source URL</span>
                <code>{buildStreamOverlayUrl(streamOverlayUrl, streamOverlayFields)}</code>
                <button class="rescan-button" onClick={() => void copyStreamOverlayUrl()}>
                  <Copy size={14} /> {streamOverlayCopied ? "Copied!" : "Copy"}
                </button>
              </div>
            </>
          )}
        </div>
      </div>
      </div>

      <div class="settings-section">
      <h2 class="settings-section-title">Diagnostics</h2>
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
      </div>

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
