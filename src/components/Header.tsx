import { Battery, Usb } from "lucide-preact";
import { StatusChip } from "./StatusChip";
import type { ConnectedDevice } from "../native-hid/scan";

type Page = "overview" | "games" | "settings";

interface Props {
  page: Page;
  onNavigate: (page: Page) => void;
  connected: ConnectedDevice | null;
}

const CHARGING_STATES = new Set(["Charging", "Charging slowly", "Almost full", "Full"]);

export function Header({ page, onNavigate, connected }: Props) {
  const status = connected?.status;
  const battery = status && status.batteryPercent !== null ? `${status.batteryPercent}%` : "—";
  const charging = status ? CHARGING_STATES.has(status.batteryState) : false;
  const connection = connected ? (status?.connectionType ?? connected.brand) : "No device";

  return (
    <header class="app-header">
      <div class="app-header-status">
        <StatusChip icon={Battery} label="Battery" value={battery} charging={charging} />
        <StatusChip icon={Usb} label="Connection" value={connection} />
      </div>

      <nav class="app-header-tabs">
        <button
          class={page === "overview" ? "active" : ""}
          onClick={() => onNavigate("overview")}
        >
          Overview
        </button>
        <button
          class={page === "games" ? "active" : ""}
          onClick={() => onNavigate("games")}
        >
          Games
        </button>
        <button
          class={page === "settings" ? "active" : ""}
          onClick={() => onNavigate("settings")}
        >
          Settings
        </button>
      </nav>
    </header>
  );
}
