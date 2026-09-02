import { useState } from "preact/hooks";
import { LayoutDashboard, Gamepad2, BookOpen, Settings, RefreshCw } from "lucide-preact";
import logo from "../assets/logo.png";
import { runUpdateCheck } from "../lib/update-check";
import { showToast } from "../lib/toast";

type Page = "overview" | "games" | "supported" | "settings";

interface Props {
  page: Page;
  onNavigate: (page: Page) => void;
}

const NAV_ITEMS: { id: Page; icon: typeof LayoutDashboard; label: string }[] = [
  { id: "overview", icon: LayoutDashboard, label: "Overview" },
  { id: "games", icon: Gamepad2, label: "Games" },
  { id: "supported", icon: BookOpen, label: "Supported mice" },
];

export function AppSidebar({ page, onNavigate }: Props) {
  const [checking, setChecking] = useState(false);

  async function handleCheckUpdate() {
    setChecking(true);
    try {
      const update = await runUpdateCheck();
      if (!update) {
        showToast("You're up to date.", "info");
      }
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error), "error");
    } finally {
      setChecking(false);
    }
  }

  return (
    <aside class="app-sidebar">
      <div class="app-sidebar-top">
        <div class="app-sidebar-brand">
          <img class="brand-mark" src={logo} alt="" width={14} height={20} />
        </div>
        <nav class="app-sidebar-nav">
          {NAV_ITEMS.map((item) => (
            <button
              key={item.id}
              class={`app-sidebar-nav-item ${page === item.id ? "active" : ""}`}
              title={item.label}
              onClick={() => onNavigate(item.id)}
            >
              <item.icon size={18} aria-hidden="true" />
            </button>
          ))}
        </nav>
      </div>

      <div class="app-sidebar-bottom">
        <button
          class={`app-sidebar-nav-item ${page === "settings" ? "active" : ""}`}
          title="Settings"
          onClick={() => onNavigate("settings")}
        >
          <Settings size={18} aria-hidden="true" />
        </button>
        <button
          class="app-sidebar-nav-item"
          title="Check for updates"
          onClick={() => void handleCheckUpdate()}
          disabled={checking}
        >
          <RefreshCw size={18} class={checking ? "spin" : ""} aria-hidden="true" />
        </button>
      </div>
    </aside>
  );
}
