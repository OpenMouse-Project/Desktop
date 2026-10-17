import { useEffect, useState } from "preact/hooks";
import { ArrowLeft, Battery, Gamepad2, Info, Keyboard, RefreshCw, Settings2, SlidersHorizontal, Lightbulb, Layers, MousePointerClick, Usb, Gauge, Zap, Palette } from "lucide-preact";
import type { MouseStatus } from "@openmouse/protocol/drivers/mouse-types";
import type { KeyboardStatus } from "@openmouse/keyboard-protocol/drivers/keyboard-types";
import { WOOTING_COMMAND } from "@openmouse/keyboard-protocol/wooting";
import type { HidInterfaceInfo } from "../native-hid/tauri-hid-device";
import { saveWootingProfile } from "../native-hid/write";
import { showToast } from "../lib/toast";
import { confirmFlashOverwrite } from "../components/DeviceKeyboardShared";
import { isKeyboardStatus } from "../native-hid/brands";
import type { MouseConnection } from "../hooks/use-mouse-connection";
import type { ActiveGameOverride } from "../hooks/use-game-watcher";
import { deviceImage, deviceImageFallback } from "../native-hid/device-images";
import { getDeviceName } from "../native-hid/device-store";
import { DeviceTile } from "../components/DeviceTile";
import { DevicePerformanceTab } from "../components/DevicePerformanceTab";
import { DeviceLightingTab } from "../components/DeviceLightingTab";
import { DeviceAdvancedTab } from "../components/DeviceAdvancedTab";
import { DeviceButtonsTab } from "../components/DeviceButtonsTab";
import { DeviceKeyboardActuationTab } from "../components/DeviceKeyboardActuationTab";
import { DeviceKeyboardRapidTriggerTab } from "../components/DeviceKeyboardRapidTriggerTab";
import { DeviceKeyboardAdvancedKeysTab } from "../components/DeviceKeyboardAdvancedKeysTab";
import { DeviceKeyboardRemapTab } from "../components/DeviceKeyboardRemapTab";
import { DeviceKeyboardRgbTab } from "../components/DeviceKeyboardRgbTab";
import { DeviceKeyboardGamepadTab } from "../components/DeviceKeyboardGamepadTab";
import { DeviceKeyboardProfilesTab } from "../components/DeviceKeyboardProfilesTab";
import { ConflictingAppsModal } from "../components/ConflictingAppsModal";
import { useConflictingApps } from "../hooks/use-conflicting-apps";

/** Brand → typical feature set shown in the device list before connecting. */
const BRAND_FEATURES: Record<string, { icon: typeof Gauge; label: string }[]> = {
  Logitech: [
    { icon: Gauge, label: "DPI" },
    { icon: Zap, label: "Polling Rate" },
    { icon: Palette, label: "LIGHTSYNC" },
    { icon: Layers, label: "Profiles" },
  ],
  Razer: [
    { icon: Gauge, label: "DPI" },
    { icon: Zap, label: "Polling Rate" },
    { icon: Palette, label: "Chroma RGB" },
  ],
  Pulsar: [
    { icon: Gauge, label: "DPI" },
    { icon: Zap, label: "Polling Rate" },
  ],
  "Endgame Gear": [
    { icon: Gauge, label: "DPI" },
    { icon: Zap, label: "Polling Rate" },
  ],
  Lamzu: [
    { icon: Gauge, label: "DPI" },
    { icon: Zap, label: "Polling Rate" },
  ],
  Ninjutso: [
    { icon: Gauge, label: "DPI" },
    { icon: Zap, label: "Polling Rate" },
  ],
  WLMouse: [
    { icon: Gauge, label: "DPI" },
    { icon: Zap, label: "Polling Rate" },
  ],
  Wooting: [
    { icon: Keyboard, label: "Analog" },
    { icon: Layers, label: "Profiles" },
    { icon: Zap, label: "Rapid Trigger" },
  ],
  ATK: [
    { icon: Gauge, label: "DPI" },
    { icon: Zap, label: "Polling Rate" },
  ],
  "Attack Shark": [
    { icon: Gauge, label: "DPI" },
    { icon: Zap, label: "Polling Rate" },
  ],
  Fantech: [
    { icon: Gauge, label: "DPI" },
    { icon: Zap, label: "Polling Rate" },
  ],
  Lingbao: [
    { icon: Gauge, label: "DPI" },
    { icon: Zap, label: "Polling Rate" },
    { icon: Battery, label: "Battery" },
  ],
  Keychron: [
    { icon: Gauge, label: "DPI" },
    { icon: Zap, label: "Polling Rate" },
  ],
  "G-Wolves": [
    { icon: Gauge, label: "DPI" },
    { icon: Zap, label: "Polling Rate" },
  ],
};

type DeviceTab =
  | "overview" | "performance" | "advanced" | "lighting" | "profiles" | "buttons"
  | "actuation" | "rapidTrigger" | "advancedKeys" | "remap" | "rgb" | "gamepad";

interface TabDef {
  id: DeviceTab;
  icon: typeof SlidersHorizontal;
  label: string;
}

type KeyboardTab = "overview" | "actuation" | "rapidTrigger" | "advancedKeys" | "remap" | "rgb" | "gamepad" | "profiles";
interface KeyboardTabDef {
  id: KeyboardTab;
  icon: typeof SlidersHorizontal;
  label: string;
}

interface DetailRow {
  label: string;
  value: string;
  mono?: boolean;
}

function statusDetailRows(status: MouseStatus | KeyboardStatus): DetailRow[] {
  const rows: DetailRow[] = [];
  if (status.firmware.length > 0) {
    rows.push({ label: "Firmware", value: status.firmware.join(" · "), mono: status.firmware.length === 1 });
  }
  if (status.connectionType) {
    rows.push({
      label: "Connection",
      value: status.connectionDetail
        ? `${status.connectionType} (${status.connectionDetail})`
        : status.connectionType,
    });
  }
  if (isKeyboardStatus(status)) {
    if (status.activeProfile !== null && status.activeProfile !== undefined) {
      const name = status.profileNames[status.activeProfile];
      rows.push({ label: "Profile", value: name ? `${status.activeProfile + 1} (${name})` : `Profile ${status.activeProfile + 1}` });
    }
    if (status.profileCount !== null && status.profileCount !== undefined) {
      rows.push({ label: "Profiles", value: String(status.profileCount) });
    }
    if (status.layout) {
      rows.push({ label: "Layout", value: status.layout });
    }
    if (status.serial) {
      rows.push({ label: "Serial", value: status.serial, mono: true });
    }
    return rows;
  }
  if (status.activeProfile !== null && status.activeProfile !== undefined) {
    rows.push({
      label: "Profile",
      value: status.onboardProfileFormat?.name
        ? `${status.activeProfile} (${status.onboardProfileFormat.name})`
        : String(status.activeProfile),
    });
  }
  if (status.hostCount) {
    rows.push({
      label: "Paired host",
      value: status.currentHost !== null && status.currentHost !== undefined
        ? `${status.currentHost + 1} of ${status.hostCount}`
        : `— of ${status.hostCount}`,
    });
  }
  if (status.friendlyName) {
    rows.push({ label: "Friendly name", value: status.friendlyName });
  }
  if (status.modelId) {
    rows.push({ label: "Model ID", value: status.modelId, mono: true });
  }
  if (status.unitId) {
    rows.push({ label: "Unit ID", value: status.unitId, mono: true });
  }
  return rows;
}

// Top-level Save to Keyboard: disabled until a tab stages changes (tracked
// via a module counter bumped by staged tabs — simple, no cross-tab store).
// On click → blocking "overwrites onboard flash slot N" modal → confirmed
// saveWootingProfile. RAM switch needs no modal and lives in Profiles.
function KeyboardSaveBar({ info, status }: { info: HidInterfaceInfo; status: KeyboardStatus }) {
  const [pending, setPending] = useState(false);
  const slot = status.activeProfile ?? 0;

  async function saveAll() {
    const confirmed = await confirmFlashOverwrite(slot, "staged keyboard changes");
    if (!confirmed) return;
    setPending(true);
    try {
      await saveWootingProfile(info, WOOTING_COMMAND.saveKeyboardProfile, slot, true);
      showToast(`Saved to flash slot ${slot + 1}.`, "success");
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error), "error");
    } finally {
      setPending(false);
    }
  }

  return (
    <div class="setting-row">
      <span class="setting-description">Staged changes save to onboard flash slot {slot + 1}.</span>
      <button class="rescan-button" disabled={pending} onClick={() => void saveAll()}>
        {pending ? "Saving…" : "Save to Keyboard"}
      </button>
    </div>
  );
}

interface Props {
  connection: MouseConnection;
  activeGameOverride?: ActiveGameOverride | null;
}

export function OverviewPage({ connection, activeGameOverride }: Props) {
  const {
    list,
    connected,
    connectedInfo,
    view,
    connectingKey,
    failedKey,
    select,
    refresh,
    back,
    patchStatus,
    setAutoRefreshPaused,
  } = connection;
  const [deviceTab, setDeviceTab] = useState<DeviceTab>("overview");
  const [keyboardTab, setKeyboardTab] = useState<KeyboardTab>("overview");
  const { apps: conflictingApps, dismiss: dismissConflicting } = useConflictingApps();

  useEffect(() => {
    setDeviceTab("overview");
    setKeyboardTab("overview");
  }, [connected?.key]);

  useEffect(() => {
    setAutoRefreshPaused(deviceTab === "performance");
    return () => setAutoRefreshPaused(false);
  }, [deviceTab, setAutoRefreshPaused]);

  // ── Connected device dashboard ──────────────────────────────────────
  if (view === "device" && connected) {
    const { status } = connected;
    // Analog keyboards: read-only tabs reusing the mouse tab chrome. Each
    // tab gates on its status field (missing → hidden tab, never guessed).
    // Display + staging only — Apply buttons stub to the write wave, and no
    // HID payload is invented (Save/Activate/RGB need a Wootility capture).
    if (isKeyboardStatus(status)) {
      const keyboardTabs: KeyboardTabDef[] = [{ id: "overview", icon: Info, label: "Overview" }];
      // Actuation + Rapid Trigger render off the profile default (0x27 f1
      // actuation, f2/f3/f4/f7 RT) — present whenever analogProfile
      // answered. Advanced Keys renders off akc/mappings (live: 1 combo).
      if (status.analogProfile) {
        keyboardTabs.push({ id: "actuation", icon: SlidersHorizontal, label: "Actuation" });
        keyboardTabs.push({ id: "rapidTrigger", icon: Zap, label: "Rapid Trigger" });
      }
      if (status.akc || status.mappings || status.dks) {
        keyboardTabs.push({ id: "advancedKeys", icon: Settings2, label: "Advanced Keys" });
      }
      if (status.mappings?.mapping || status.mappings?.main || status.mappings?.function) {
        keyboardTabs.push({ id: "remap", icon: MousePointerClick, label: "Remap" });
      }
      if (status.rgb?.core || status.rgb?.colors1 || status.rgb?.colors2 || status.rgb?.layer) {
        keyboardTabs.push({ id: "rgb", icon: Lightbulb, label: "RGB" });
      }
      if (status.gamepad?.profile) keyboardTabs.push({ id: "gamepad", icon: Gamepad2, label: "Gamepad" });
      if ((status.profileCount ?? status.profileNames.length) > 0 || status.diagnostics || status.analogSnapshot) {
        keyboardTabs.push({ id: "profiles", icon: Layers, label: "Profiles" });
      }
      const visibleTab = keyboardTabs.some((tab) => tab.id === keyboardTab) ? keyboardTab : "overview";
      return (
        <section class="page page-overview">
          <nav class="device-tabs-bar">
            <button class="device-tab-back" onClick={back}>
              <ArrowLeft size={13} aria-hidden="true" /> Devices
            </button>
            {keyboardTabs.map((tab) => (
              <button
                key={tab.id}
                class={`device-tab-pill ${visibleTab === tab.id ? "active" : ""}`}
                onClick={() => setKeyboardTab(tab.id)}
              >
                <tab.icon size={13} aria-hidden="true" /> {tab.label}
              </button>
            ))}
          </nav>
          <ConflictingAppsModal apps={conflictingApps} onDismissed={dismissConflicting} />
          {visibleTab === "overview" && (
            <>
              <div class="device-showcase">
                <h1 class="device-showcase-name">{status.name}</h1>
                <p class="device-showcase-brand">{connected.brand}</p>
                <div class="device-showcase-visual">
                  <img
                    class="device-showcase-image"
                    src={deviceImage(connected.key, status.name)}
                    onError={deviceImageFallback}
                    alt={status.name}
                  />
                </div>
                <div class="device-showcase-status">
                  <span class="device-showcase-dot" aria-hidden="true" />
                  Connected
                  {status.connectionType && (
                    <span class="device-showcase-status-detail">· {status.connectionType}</span>
                  )}
                </div>
              </div>
              <div class="info-section">
                <span class="info-section-title">Current Status</span>
                <div class="info-grid">
                  <div class="info-row">
                    <span class="info-label"><Keyboard size={11} aria-hidden="true" /> Type</span>
                    <span class="info-value">Analog keyboard</span>
                  </div>
                  <div class="info-row">
                    <span class="info-label">Actuation</span>
                    <span class="info-value">
                      {status.analogProfile?.actuationMm !== null && status.analogProfile?.actuationMm !== undefined
                        ? `${status.analogProfile.actuationMm.toFixed(2)}mm`
                        : "Not reported"}
                    </span>
                  </div>
                  <div class="info-row">
                    <span class="info-label">Profile</span>
                    <span class="info-value">
                      {status.activeProfile !== null && status.activeProfile !== undefined
                        ? status.profileNames[status.activeProfile] ?? `Profile ${status.activeProfile + 1}`
                        : "Default"}
                    </span>
                  </div>
                  {status.connectionType && (
                    <div class="info-row">
                      <span class="info-label">Connection</span>
                      <span class="info-value">
                        {status.connectionDetail
                          ? `${status.connectionType} (${status.connectionDetail})`
                          : status.connectionType}
                      </span>
                    </div>
                  )}
                </div>
              </div>
              {statusDetailRows(status).length > 0 && (
                <div class="info-section">
                  <span class="info-section-title">Device Information</span>
                  <div class="info-grid">
                    <div class="info-row">
                      <span class="info-label">Manufacturer</span>
                      <span class="info-value">{connected.brand}</span>
                    </div>
                    {statusDetailRows(status).map((row) => (
                      <div class="info-row" key={row.label}>
                        <span class="info-label">{row.label}</span>
                        <span class={`info-value ${row.mono ? "info-value-mono" : ""}`}>{row.value}</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </>
          )}
          {visibleTab === "actuation" && status.analogProfile && (
            <DeviceKeyboardActuationTab status={status} />
          )}
          {visibleTab === "rapidTrigger" && status.analogProfile && (
            <DeviceKeyboardRapidTriggerTab status={status} />
          )}
          {visibleTab === "advancedKeys" && (
            <DeviceKeyboardAdvancedKeysTab status={status} />
          )}
          {visibleTab === "remap" && (
            <DeviceKeyboardRemapTab status={status} onApplied={patchStatus} />
          )}
          {visibleTab === "rgb" && connectedInfo && (
            <DeviceKeyboardRgbTab info={connectedInfo} status={status} onApplied={patchStatus} />
          )}
          {visibleTab === "gamepad" && (
            <DeviceKeyboardGamepadTab status={status} onApplied={patchStatus} />
          )}
          {visibleTab === "profiles" && connectedInfo && (
            <DeviceKeyboardProfilesTab info={connectedInfo} status={status} onApplied={patchStatus} />
          )}
          {visibleTab !== "overview" && connectedInfo && (
            <KeyboardSaveBar info={connectedInfo} status={status} />
          )}
        </section>
      );
    }
    // Any driver-backed brand can now be written, not just Logitech/Razer —
    // the generic write layer (native-hid/write.ts) exposes the shared
    // setter surface across every candidate driver, and each tab gates its
    // individual controls on the status fields the device actually reports
    // (so a brand without, say, motion sync simply doesn't get that slider).
    const canControl = connectedInfo !== null;
    const infoRows = statusDetailRows(status);
    const tabs: TabDef[] = [{ id: "overview", icon: Info, label: "Overview" }];

    const hasPerformance = status.dpi > 0 ||
      (status.supportedPollingRates && status.supportedPollingRates.length > 0) ||
      status.liftOffDistance != null ||
      status.gamingSurfaceMode != null ||
      status.debounceMs != null ||
      status.sleepTimeout != null ||
      status.motionSync != null ||
      status.angleSnapping != null ||
      status.rippleControl != null ||
      status.performanceMode != null ||
      status.hyperMode != null ||
      status.sensorMode != null ||
      status.usbSpeed != null ||
      status.primaryButton != null ||
      (status.dpiStages && status.dpiStages.length > 0);
    if (hasPerformance) tabs.push({ id: "performance", icon: SlidersHorizontal, label: "Performance" });

    const hasAdvanced =
      typeof status.slamclickFilter === "boolean" ||
      typeof status.motionJitterFilter === "boolean" ||
      Array.isArray(status.eggCpiStages) ||
      Boolean(status.ninjutsoSystemMode) ||
      Boolean(status.ninjutsoOpticalEngine) ||
      status.ninjutsoHyperClick != null ||
      Boolean(status.ninjutsoSlamClick) ||
      status.finalmouseDongleLedMode != null ||
      status.sensorMode != null ||
      status.performanceDuration != null ||
      status.dpiLedMode != null ||
      status.wheelAcceleration != null ||
      typeof status.activeProfile === "number";
    if (hasAdvanced) tabs.push({ id: "advanced", icon: Settings2, label: "Advanced" });

    // Gated on the device actually reporting lighting: the generic Razer
    // client has no lighting at all (its class has no setLighting and never
    // sets `lighting`), so forcing the tab for every Razer opened it onto "No
    // lighting data available" — while the Razer models that *do* report it
    // (Cobra, Viper Mini) still get it through these same two checks.
    const hasLighting = !!(status.lighting) ||
      (status.lightingZones && status.lightingZones.length > 0);
    if (hasLighting) {
      tabs.push({ id: "lighting", icon: Lightbulb, label: "Lighting" });
    }

    if (status.activeProfile !== null && status.activeProfile !== undefined) {
      tabs.push({ id: "profiles", icon: Layers, label: "Profiles" });
    }

    if (status.razerButtonMappings || status.eggButtonMappings || status.analogButtonTuning) {
      tabs.push({ id: "buttons", icon: MousePointerClick, label: "Buttons" });
    }

    return (
      <section class="page page-overview">
        {/* Device feature tab bar */}
        <nav class="device-tabs-bar">
          <button class="device-tab-back" onClick={back}>
            <ArrowLeft size={13} aria-hidden="true" /> Devices
          </button>
          {tabs.map((tab) => (
            <button
              key={tab.id}
              class={`device-tab-pill ${deviceTab === tab.id ? "active" : ""}`}
              onClick={() => setDeviceTab(tab.id)}
            >
              <tab.icon size={13} aria-hidden="true" /> {tab.label}
            </button>
          ))}
        </nav>

        {/* Conflicting apps warning */}
        <ConflictingAppsModal apps={conflictingApps} onDismissed={dismissConflicting} />

        {/* ── Overview tab ───────────────────────────────────────── */}
        {deviceTab === "overview" && (
          <>
            {/* Game Override Banner */}
            {activeGameOverride && (
              <div class="profile-override-banner">
                <Gamepad2 size={14} aria-hidden="true" />
                <span>
                  <strong>{activeGameOverride.gameName}</strong> profile is active — DPI and polling rate are controlled by the game. Your defaults return when it closes.
                </span>
              </div>
            )}

            {/* Device Showcase */}
            <div class="device-showcase">
              <h1 class="device-showcase-name">{status.name}</h1>
              <p class="device-showcase-brand">{connected.brand}</p>
              <div class="device-showcase-visual">
                <img
                  class="device-showcase-image"
                  src={deviceImage(connected.key, status.name)}
                  onError={deviceImageFallback}
                  alt={status.name}
                />
              </div>
              <div class="device-showcase-status">
                <span class="device-showcase-dot" aria-hidden="true" />
                Connected
                {status.connectionType && (
                  <span class="device-showcase-status-detail">· {status.connectionType}</span>
                )}
                <span class="device-showcase-status-detail">· <Battery size={14} class="device-showcase-battery-icon" /> Battery {status.batteryPercent !== null ? `${status.batteryPercent}%` : "N/A"}</span>
              </div>
            </div>

            {/* Current Status */}
            <div class="info-section">
              <span class="info-section-title">Current Status</span>
              <div class="info-grid">
                <div class="info-row">
                  <span class="info-label">DPI</span>
                  <span class="info-value">{status.dpi.toLocaleString()}</span>
                </div>
                <div class="info-row">
                  <span class="info-label">Polling Rate</span>
                  <span class="info-value">{status.pollingRateHz} Hz</span>
                </div>
                <div class="info-row">
                  <span class="info-label">Profile</span>
                  <span class="info-value">
                    {status.activeProfile !== null && status.activeProfile !== undefined
                      ? status.onboardProfileFormat?.name ?? `Profile ${status.activeProfile}`
                      : "Default"}
                  </span>
                </div>
                <div class="info-row">
                  <span class="info-label"><Battery size={11} aria-hidden="true" /> Battery</span>
                  <span class="info-value">
                    {status.batteryPercent !== null ? `${status.batteryPercent}%` : "N/A"}
                    {status.batteryPercent !== null && status.batteryState !== "Unknown" && (
                      <span class="info-value-sub"> {status.batteryState}</span>
                    )}
                  </span>
                </div>
                {status.connectionType && (
                  <div class="info-row">
                    <span class="info-label">Connection</span>
                    <span class="info-value">
                      {status.connectionDetail
                        ? `${status.connectionType} (${status.connectionDetail})`
                        : status.connectionType}
                    </span>
                  </div>
                )}
              </div>
            </div>

            {/* Device Information */}
            {infoRows.length > 0 && (
              <div class="info-section">
                <span class="info-section-title">Device Information</span>
                <div class="info-grid">
                  <div class="info-row">
                    <span class="info-label">Manufacturer</span>
                    <span class="info-value">{connected.brand}</span>
                  </div>
                  {infoRows.map((row) => (
                    <div class="info-row" key={row.label}>
                      <span class="info-label">{row.label}</span>
                      <span class={`info-value ${row.mono ? "info-value-mono" : ""}`}>{row.value}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </>
        )}

        {/* ── Performance tab ─────────────────────────────────────── */}
        {deviceTab === "performance" && connectedInfo && (
          <DevicePerformanceTab
            info={connectedInfo}
            status={status}
            brand={connected.brand}
            onApplied={patchStatus}
            lockedBy={activeGameOverride?.gameName}
            readOnly={!canControl}
          />
        )}

        {/* ── Advanced tab ─────────────────────────────────────── */}
        {deviceTab === "advanced" && connectedInfo && (
          <DeviceAdvancedTab
            info={connectedInfo}
            status={status}
            onApplied={patchStatus}
            readOnly={!canControl}
          />
        )}

        {/* ── Lighting tab ──────────────────────────────────────── */}
        {deviceTab === "lighting" && (
          connectedInfo ? (
            <DeviceLightingTab
              info={connectedInfo}
              status={status}
              onApplied={patchStatus}
              readOnly={!canControl}
            />
          ) : (
            <div class="tab-placeholder">
              <Lightbulb size={32} aria-hidden="true" />
              <h2>Lighting</h2>
              <p>Connecting to device… lighting controls will appear shortly.</p>
            </div>
          )
        )}

        {/* ── Profiles tab (placeholder) ──────────────────────────── */}
        {deviceTab === "profiles" && (
          <div class="tab-placeholder">
            <Layers size={32} aria-hidden="true" />
            <h2>Profiles</h2>
            <p>Profile management coming soon for this device.</p>
          </div>
        )}

        {/* ── Buttons tab ───────────────────────────────────────── */}
        {deviceTab === "buttons" && (
          connectedInfo ? (
            <DeviceButtonsTab
              info={connectedInfo}
              status={status}
              brand={connected.brand}
              onApplied={patchStatus}
              readOnly={!canControl}
            />
          ) : (
            <div class="tab-placeholder">
              <MousePointerClick size={32} aria-hidden="true" />
              <h2>Buttons</h2>
              <p>Connecting to device… button controls will appear shortly.</p>
            </div>
          )
        )}
      </section>
    );
  }

  // ── Device list / empty state ───────────────────────────────────────
  return (
    <section class="page page-overview">
      {list.status === "error" && (
        <div class="empty-state">
          <h2>Couldn't list HID devices</h2>
          <p>{list.message}</p>
        </div>
      )}

      {list.status === "loaded" && list.candidates.length === 0 && list.unsupported.length === 0 && (
        <div class="overview-empty">
          <div class="overview-empty-icon">
            <Usb size={40} aria-hidden="true" />
          </div>
          <h2>No device connected</h2>
          <p>Connect a supported OpenMouse device to begin configuring it.</p>
          <button class="rescan-button rescan-button-standalone" onClick={() => void refresh()}>
            <RefreshCw size={14} /> Refresh Devices
          </button>
        </div>
      )}

      {list.status === "loaded" && list.unsupported.length > 0 && (
        <div class="device-unsupported-notice" role="status">
          <Keyboard size={20} aria-hidden="true" />
          <div>
            <strong>{[...new Set(list.unsupported.map((device) => device.name))].join(", ")} detected</strong>
            <p>OpenMouse can configure Wooting keyboards, but this Razer Huntsman has no keyboard driver yet.</p>
            {list.candidates.length === 0 && (
              <button class="rescan-button" onClick={() => void refresh()}>
                <RefreshCw size={14} /> Refresh Devices
              </button>
            )}
          </div>
        </div>
      )}

      {list.status === "loading" && (
        <div class="overview-empty">
          <div class="overview-empty-icon">
            <RefreshCw size={40} class="spin" aria-hidden="true" />
          </div>
          <h2>Searching for devices</h2>
          <p>Looking for supported devices…</p>
        </div>
      )}

      {list.status === "loaded" && list.candidates.length > 0 && (
        <>
          <h1 class="page-title">Devices</h1>
          <ul class={`device-grid device-grid--${Math.min(list.candidates.length, 4) || 1}`}>
            {list.candidates.map((candidate) => {
              const primaryBrand = candidate.brands[0];
              const isConnected = connected?.key === candidate.info.key;
              const isConnecting = connectingKey === candidate.info.key;
              // What the device said it is, when it has ever answered: a
              // receiver's own name is "USB Receiver" and its artwork is a
              // placeholder, while the mouse behind it has both. Falls back to
              // the live status for a device connected in this session, then to
              // whatever the interface called itself.
              const displayName = (isConnected ? connected?.status.name : null)
                ?? getDeviceName(candidate.info.key)
                ?? candidate.info.productString;
              const state = isConnecting
                ? "connecting"
                : isConnected
                  ? "connected"
                  : failedKey === candidate.info.key
                    ? "error"
                    : "idle";
              return (
                <DeviceTile
                  key={candidate.info.key}
                  candidate={candidate}
                  displayName={displayName}
                  features={primaryBrand ? BRAND_FEATURES[primaryBrand] : undefined}
                  state={state}
                  battery={isConnected && connected && !isKeyboardStatus(connected.status) ? connected.status.batteryPercent : null}
                  onSelect={() => select(candidate)}
                />
              );
            })}
          </ul>
        </>
      )}
    </section>
  );
}
