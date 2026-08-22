import { useEffect, useState } from "preact/hooks";
import { ArrowLeft, Battery, Cpu, Mouse, RefreshCw, Usb } from "lucide-preact";
import {
  connectToInterface,
  listCandidateInterfaces,
  type CandidateInterface,
  type ConnectedDevice,
} from "../native-hid/scan";

type ListState =
  | { status: "loading" }
  | { status: "loaded"; candidates: CandidateInterface[] }
  | { status: "error"; message: string };

export function OverviewPage() {
  const [list, setList] = useState<ListState>({ status: "loading" });
  const [connected, setConnected] = useState<ConnectedDevice | null>(null);
  const [connectingKey, setConnectingKey] = useState<string | null>(null);
  const [connectError, setConnectError] = useState<string | null>(null);

  async function refresh() {
    setList({ status: "loading" });
    try {
      const candidates = await listCandidateInterfaces();
      setList({ status: "loaded", candidates });
    } catch (error) {
      setList({ status: "error", message: error instanceof Error ? error.message : String(error) });
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  async function connect(candidate: CandidateInterface) {
    setConnectingKey(candidate.info.key);
    setConnectError(null);
    try {
      const device = await connectToInterface(candidate.info);
      setConnected(device);
    } catch (error) {
      setConnectError(error instanceof Error ? error.message : String(error));
    } finally {
      setConnectingKey(null);
    }
  }

  // The device was already read once and closed immediately in
  // connectToInterface() — nothing here holds a live connection open, so
  // "Back" is just clearing the snapshot, not disconnecting anything.
  function backToList() {
    setConnected(null);
    void refresh();
  }

  if (connected) {
    const { status } = connected;
    return (
      <section class="page">
        <div class="device-card">
          <div class="device-card-header">
            <Mouse size={22} aria-hidden="true" />
            <div>
              <h2>{status.name}</h2>
              <p class="device-card-brand">{connected.brand}</p>
            </div>
            <button class="rescan-button" onClick={backToList} title="Back to device list">
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
            </div>
            <div class="device-stat">
              <span class="device-stat-label">
                <Cpu size={12} aria-hidden="true" /> Firmware
              </span>
              <span class="device-stat-value">{status.firmware[0] ?? "—"}</span>
            </div>
          </div>
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
              <div class="device-list-row-info">
                <span class="device-list-row-name">
                  {candidate.info.productString || "Unknown device"}
                </span>
                <span class="device-list-row-meta">
                  {candidate.brands.join(" / ")} · {candidate.info.vendorId.toString(16).padStart(4, "0")}:
                  {candidate.info.productId.toString(16).padStart(4, "0")}
                </span>
              </div>
              <button
                class="connect-button"
                disabled={connectingKey === candidate.info.key}
                onClick={() => void connect(candidate)}
              >
                {connectingKey === candidate.info.key ? "Connecting…" : "Connect"}
              </button>
            </li>
          ))}
        </ul>
      )}

      {connectError && <p class="connect-error">{connectError}</p>}
    </section>
  );
}
