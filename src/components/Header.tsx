import { Battery, Cpu, Usb } from "lucide-preact";
import { StatusChip } from "./StatusChip";

type Page = "overview" | "settings";

interface Props {
  page: Page;
  onNavigate: (page: Page) => void;
}

export function Header({ page, onNavigate }: Props) {
  return (
    <header class="app-header">
      <div class="app-header-status">
        <StatusChip icon={Battery} label="Battery" value="—" />
        <StatusChip icon={Cpu} label="Firmware" value="—" />
        <StatusChip icon={Usb} label="Connection" value="No device" />
      </div>

      <nav class="app-header-tabs">
        <button
          class={page === "overview" ? "active" : ""}
          onClick={() => onNavigate("overview")}
        >
          Overview
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
