import { useEffect, useState } from "preact/hooks";
import { RefreshCw, TriangleAlert, Usb } from "lucide-preact";
import { listCandidateInterfaces, type CandidateInterface } from "../native-hid/scan";

// Connecting is disabled for now — every attempt so far (across several
// different fixes: which HID collection we open, how long we hold it
// open, one shared HidApi vs. a fresh one per call) reproduced a real
// input freeze on real hardware (a Logitech receiver, a Wooting keyboard,
// an Endgame Gear OP1we) that only cleared once the whole app process was
// killed. None of those fixes actually addressed the cause, so opening a
// device is not safe to ship yet. Listing stays enabled — pure HID
// enumeration, never implicated in any of the freezes.
const CONNECT_DISABLED_REASON =
  "Connecting is temporarily disabled — it reproducibly froze mouse/keyboard input during testing and the real cause isn't understood yet.";

type ListState =
  | { status: "loading" }
  | { status: "loaded"; candidates: CandidateInterface[] }
  | { status: "error"; message: string };

export function OverviewPage() {
  const [list, setList] = useState<ListState>({ status: "loading" });

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

  return (
    <section class="page">
      <div class="device-list-header">
        <h1 class="page-title">Devices</h1>
        <button class="rescan-button" onClick={() => void refresh()} disabled={list.status === "loading"}>
          <RefreshCw size={14} class={list.status === "loading" ? "spin" : ""} /> Refresh
        </button>
      </div>

      <p class="connect-disabled-notice">
        <TriangleAlert size={14} aria-hidden="true" />
        {CONNECT_DISABLED_REASON}
      </p>

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
              <button class="connect-button" disabled title={CONNECT_DISABLED_REASON}>
                Connect
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
