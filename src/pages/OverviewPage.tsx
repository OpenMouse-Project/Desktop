import { useEffect, useState } from "preact/hooks";
import { Battery, Cpu, Mouse, RefreshCw, Usb } from "lucide-preact";
import { scanForDevice, type ConnectedDevice } from "../native-hid/scan";

type ScanState =
  | { status: "scanning" }
  | { status: "found"; device: ConnectedDevice }
  | { status: "not-found" }
  | { status: "error"; message: string };

export function OverviewPage() {
  const [state, setState] = useState<ScanState>({ status: "scanning" });

  async function scan() {
    setState({ status: "scanning" });
    try {
      const device = await scanForDevice();
      setState(device ? { status: "found", device } : { status: "not-found" });
    } catch (error) {
      setState({ status: "error", message: error instanceof Error ? error.message : String(error) });
    }
  }

  useEffect(() => {
    void scan();
    // Only the open connection (if any) needs cleanup on unmount — a
    // scan still in flight has nothing live to close yet.
    return () => {
      if (state.status === "found") void state.device.client.close().catch(() => undefined);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (state.status === "found") {
    const { status } = state.device;
    return (
      <section class="page">
        <div class="device-card">
          <div class="device-card-header">
            <Mouse size={22} aria-hidden="true" />
            <div>
              <h2>{status.name}</h2>
              <p class="device-card-brand">{state.device.brand}</p>
            </div>
            <button class="rescan-button" onClick={() => void scan()} title="Rescan">
              <RefreshCw size={14} />
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
      <div class="empty-state">
        {state.status === "scanning" ? (
          <>
            <Usb class="empty-state-icon spin" size={40} aria-hidden="true" />
            <h2>Scanning for devices…</h2>
            <p>Checking connected HID interfaces for a supported mouse.</p>
          </>
        ) : (
          <>
            <Mouse class="empty-state-icon" size={40} aria-hidden="true" />
            <h2>{state.status === "error" ? "Scan failed" : "No device connected"}</h2>
            <p>
              {state.status === "error"
                ? state.message
                : "Connect a supported mouse over USB or a receiver to see its DPI, polling rate, battery, and firmware here."}
            </p>
            <button class="rescan-button rescan-button-standalone" onClick={() => void scan()}>
              <RefreshCw size={14} /> Scan again
            </button>
          </>
        )}
      </div>
    </section>
  );
}
