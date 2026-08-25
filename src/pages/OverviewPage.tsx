import { useEffect, useState } from "preact/hooks";
import { ArrowLeft, Battery, Info, RefreshCw, SlidersHorizontal, Usb } from "lucide-preact";
import type { MouseStatus } from "@openmouse/protocol/drivers/mouse-types";
import type { MouseConnection } from "../hooks/use-mouse-connection";
import { deviceImage, UNKNOWN_DEVICE_IMAGE } from "../native-hid/device-images";
import { DevicePerformanceTab } from "../components/DevicePerformanceTab";

/** Swaps a broken/missing product photo for the generic fallback, once. */
function fallbackToUnknownDevice(event: Event) {
  const img = event.currentTarget as HTMLImageElement;
  if (img.src.endsWith(UNKNOWN_DEVICE_IMAGE)) return;
  img.src = UNKNOWN_DEVICE_IMAGE;
}

interface DetailRow {
  label: string;
  value: string;
  mono?: boolean;
}

/**
 * Everything `readStatus()` returned beyond the three headline stats
 * (DPI/rate/battery, shown in the grid above the tabs), filtered down to
 * whatever this particular brand/model actually populated — most of
 * `MouseStatus`'s fields are brand-specific and undefined everywhere else.
 * Lives under the Information tab, out of the way of the always-visible
 * stats — firmware included, since it's rarely what someone connects to
 * check first.
 */
function detailRows(status: MouseStatus): DetailRow[] {
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
  if (status.liftOffDistance) {
    rows.push({
      label: "Lift-off distance",
      value: status.supportedLiftOffDistances?.length
        ? `${status.liftOffDistance} (of ${status.supportedLiftOffDistances.join("/")})`
        : status.liftOffDistance,
    });
  }
  if (status.deviceMode) {
    rows.push({ label: "Device mode", value: status.deviceMode });
  }
  if (status.activeProfile !== null && status.activeProfile !== undefined) {
    rows.push({
      label: "Active profile",
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
  if (status.supportedPollingRates?.length) {
    rows.push({ label: "Supported rates", value: status.supportedPollingRates.map((hz) => `${hz} Hz`).join(", ") });
  }
  if (status.supportsSeparateDpiAxes && status.dpiY !== undefined && status.dpiY !== status.dpi) {
    rows.push({ label: "DPI (X × Y)", value: `${status.dpi} × ${status.dpiY}` });
  }
  if (status.primaryButton) {
    rows.push({ label: "Primary button", value: status.primaryButton });
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
}

type DeviceTab = "information" | "performance";

export function OverviewPage({ connection }: Props) {
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
  const [deviceTab, setDeviceTab] = useState<DeviceTab>("performance");

  // A tab choice belongs to whichever device is showing — land back on
  // Performance rather than leaving it selected (and possibly hidden, for a
  // brand without write support) after switching devices.
  useEffect(() => {
    setDeviceTab("performance");
  }, [connected?.key]);

  // The background auto-refresh (use-mouse-connection.ts, every 5s) is a
  // full reconnect — CONFIRMED disruptive when it landed mid-edit on the
  // Performance tab (a visible "looking up the mouse" while clicking
  // through DPI/rate/surface controls). Pause it for as long as this tab is
  // open; Information is read-only, nothing there minds a background sync.
  useEffect(() => {
    setAutoRefreshPaused(deviceTab === "performance");
    return () => setAutoRefreshPaused(false);
  }, [deviceTab, setAutoRefreshPaused]);

  if (view === "device" && connected) {
    const { status } = connected;
    // Write actions only exist for Logitech so far (native-hid/logitech-actions.ts)
    // — other brands get an Information-only card until their own tab exists.
    const canControl = connected.brand === "Logitech" && connectedInfo !== null;
    const rows = detailRows(status);
    return (
      <section class="page">
        <div class="device-card">
          <div class="device-card-header">
            <img
              class="device-card-image"
              src={deviceImage(connected.key, status.name)}
              onError={fallbackToUnknownDevice}
              alt=""
            />
            <div>
              <h2>{status.name}</h2>
              <p class="device-card-brand">{connected.brand}</p>
            </div>
            <button class="rescan-button" onClick={back} title="Back to device list">
              <ArrowLeft size={14} /> Back
            </button>
          </div>

          <div class="device-stat-grid">
            <div class="device-stat">
              <span class="device-stat-label">DPI</span>
              <span class="device-stat-value">{status.dpi}</span>
            </div>
            <div class="device-stat">
              <span class="device-stat-label">Polling rate</span>
              <span class="device-stat-value">{status.pollingRateHz} Hz</span>
            </div>
            <div class="device-stat">
              <span class="device-stat-label">
                <Battery size={12} aria-hidden="true" /> Battery
              </span>
              <span class="device-stat-value">
                {status.batteryPercent !== null ? `${status.batteryPercent}%` : "—"}
              </span>
              {status.batteryPercent !== null && status.batteryState !== "Unknown" && (
                <span class="device-stat-sub">{status.batteryState}</span>
              )}
            </div>
          </div>

          {canControl && (
            <nav class="device-tabs">
              <button
                class={deviceTab === "performance" ? "active" : ""}
                onClick={() => setDeviceTab("performance")}
              >
                <SlidersHorizontal size={13} aria-hidden="true" /> Performance
              </button>
              <button
                class={deviceTab === "information" ? "active" : ""}
                onClick={() => setDeviceTab("information")}
              >
                <Info size={13} aria-hidden="true" /> Information
              </button>
            </nav>
          )}

          {(!canControl || deviceTab === "information") && rows.length > 0 && (
            <div class="device-detail-list">
              {rows.map((row) => (
                <div class="device-detail-row" key={row.label}>
                  <span class="device-detail-label">{row.label}</span>
                  <span class={`device-detail-value ${row.mono ? "device-detail-value-mono" : ""}`}>
                    {row.value}
                  </span>
                </div>
              ))}
            </div>
          )}

          {canControl && deviceTab === "performance" && connectedInfo && (
            <DevicePerformanceTab info={connectedInfo} status={status} onApplied={patchStatus} />
          )}
        </div>
      </section>
    );
  }

  return (
    <section class="page">
      <div class="device-list-header">
        <h1 class="page-title">Devices</h1>
        <button class="rescan-button" onClick={() => void refresh()} disabled={list.status === "loading"}>
          <RefreshCw size={14} class={list.status === "loading" ? "spin" : ""} /> Refresh
        </button>
      </div>

      {list.status === "error" && (
        <div class="empty-state">
          <h2>Couldn't list HID devices</h2>
          <p>{list.message}</p>
        </div>
      )}

      {list.status === "loaded" && list.candidates.length === 0 && (
        <div class="empty-state">
          <Usb class="empty-state-icon" size={40} aria-hidden="true" />
          <h2>No supported devices found</h2>
          <p>Connect a supported mouse over USB or a receiver, then refresh.</p>
        </div>
      )}

      {list.status === "loaded" && list.candidates.length > 0 && (
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
      )}
    </section>
  );
}
