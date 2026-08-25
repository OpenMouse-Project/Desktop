import { useEffect, useState } from "preact/hooks";
import { ArrowLeft, Battery, Gamepad2, Info, RefreshCw, SlidersHorizontal, Lightbulb, Layers, MousePointerClick, Usb } from "lucide-preact";
import type { MouseStatus } from "@openmouse/protocol/drivers/mouse-types";
import type { MouseConnection } from "../hooks/use-mouse-connection";
import type { ActiveGameOverride } from "../hooks/use-game-watcher";
import { deviceImage, UNKNOWN_DEVICE_IMAGE } from "../native-hid/device-images";
import { DevicePerformanceTab } from "../components/DevicePerformanceTab";

function fallbackToUnknownDevice(event: Event) {
  const img = event.currentTarget as HTMLImageElement;
  if (img.src.endsWith(UNKNOWN_DEVICE_IMAGE)) return;
  img.src = UNKNOWN_DEVICE_IMAGE;
}

type DeviceTab = "overview" | "performance" | "lighting" | "profiles" | "buttons";

interface TabDef {
  id: DeviceTab;
  icon: typeof SlidersHorizontal;
  label: string;
}

interface DetailRow {
  label: string;
  value: string;
  mono?: boolean;
}

function statusDetailRows(status: MouseStatus): DetailRow[] {
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
    select,
    refresh,
    back,
    patchStatus,
    setAutoRefreshPaused,
  } = connection;
  const [deviceTab, setDeviceTab] = useState<DeviceTab>("overview");

  useEffect(() => {
    setDeviceTab("overview");
  }, [connected?.key]);

  useEffect(() => {
    setAutoRefreshPaused(deviceTab === "performance");
    return () => setAutoRefreshPaused(false);
  }, [deviceTab, setAutoRefreshPaused]);

  // ── Connected device dashboard ──────────────────────────────────────
  if (view === "device" && connected) {
    const { status } = connected;
    const canControl = connected.brand === "Logitech" && connectedInfo !== null;
    const infoRows = statusDetailRows(status);

    // Capability-driven tab list
    const tabs: TabDef[] = [{ id: "overview", icon: Info, label: "Overview" }];

    const hasPerformance = status.dpi > 0 ||
      (status.supportedPollingRates && status.supportedPollingRates.length > 0) ||
      status.liftOffDistance != null ||
      status.gamingSurfaceMode != null;
    if (hasPerformance) tabs.push({ id: "performance", icon: SlidersHorizontal, label: "Performance" });

    if (status.lighting || (status.lightingZones && status.lightingZones.length > 0)) {
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
                  onError={fallbackToUnknownDevice}
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
            onApplied={patchStatus}
            lockedBy={activeGameOverride?.gameName}
            readOnly={!canControl}
          />
        )}

        {/* ── Lighting tab (placeholder) ──────────────────────────── */}
        {deviceTab === "lighting" && (
          <div class="tab-placeholder">
            <Lightbulb size={32} aria-hidden="true" />
            <h2>Lighting</h2>
            <p>Lighting controls coming soon for this device.</p>
          </div>
        )}

        {/* ── Profiles tab (placeholder) ──────────────────────────── */}
        {deviceTab === "profiles" && (
          <div class="tab-placeholder">
            <Layers size={32} aria-hidden="true" />
            <h2>Profiles</h2>
            <p>Profile management coming soon for this device.</p>
          </div>
        )}

        {/* ── Buttons tab (placeholder) ───────────────────────────── */}
        {deviceTab === "buttons" && (
          <div class="tab-placeholder">
            <MousePointerClick size={32} aria-hidden="true" />
            <h2>Buttons</h2>
            <p>Button configuration coming soon for this device.</p>
          </div>
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

      {list.status === "loaded" && list.candidates.length === 0 && (
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

      {list.status === "loading" && (
        <div class="overview-empty">
          <div class="overview-empty-icon">
            <RefreshCw size={40} class="spin" aria-hidden="true" />
          </div>
          <h2>Searching for devices</h2>
          <p>Looking for supported mice…</p>
        </div>
      )}

      {list.status === "loaded" && list.candidates.length > 0 && (
        <>
          <h1 class="page-title">Devices</h1>
          <ul class="device-list">
            {list.candidates.map((candidate) => (
              <li class="device-list-row" key={candidate.info.key}>
                <div class="device-list-row-main">
                  <img
                    class="device-list-row-image"
                    src={deviceImage(candidate.info.key, candidate.info.productString)}
                    onError={fallbackToUnknownDevice}
                    alt=""
                  />
                  <div class="device-list-row-info">
                    <span class="device-list-row-name">
                      {candidate.info.productString || "Unknown device"}
                    </span>
                    <span class="device-list-row-meta">
                      {candidate.brands.join(" / ")} · {candidate.info.vendorId.toString(16).padStart(4, "0")}:
                      {candidate.info.productId.toString(16).padStart(4, "0")}
                    </span>
                  </div>
                </div>
                <button
                  class="connect-button"
                  disabled={connectingKey === candidate.info.key}
                  onClick={() => select(candidate)}
                >
                  {connectingKey === candidate.info.key
                    ? "Connecting…"
                    : connected?.key === candidate.info.key
                      ? "View"
                      : "Connect"}
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
    </section>
  );
}
