import { useEffect, useState } from "preact/hooks";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type AppMode = "bridge" | "full-desktop";

function BridgePanel() {
  return (
    <section class="panel">
      <h2>Bridge Mode</h2>
      <p>
        Minimal background companion. Game detection, low-battery alerts, and
        native HID access will live here — closing this window hides it to
        the tray instead of quitting.
      </p>
      <p class="placeholder">(placeholder — device list not wired up yet)</p>
    </section>
  );
}

function FullDesktopPanel() {
  return (
    <section class="panel">
      <h2>Full Desktop Mode</h2>
      <p>
        The full device configuration UI — DPI, polling rate, RGB, and
        firmware controls — will live here, reusing openmouse's components.
      </p>
      <p class="placeholder">(placeholder — driver bridge not wired up yet)</p>
    </section>
  );
}

function App() {
  const [mode, setModeState] = useState<AppMode | null>(null);

  useEffect(() => {
    invoke<AppMode>("get_mode").then(setModeState);
  }, []);

  async function switchMode(next: AppMode) {
    const confirmed = await invoke<AppMode>("set_mode", { mode: next });
    setModeState(confirmed);
  }

  if (mode === null) {
    return null;
  }

  return (
    <main class="container">
      <header class="mode-toggle">
        <button
          class={mode === "bridge" ? "active" : ""}
          onClick={() => switchMode("bridge")}
        >
          Bridge Mode
        </button>
        <button
          class={mode === "full-desktop" ? "active" : ""}
          onClick={() => switchMode("full-desktop")}
        >
          Full Desktop Mode
        </button>
      </header>

      {mode === "bridge" ? <BridgePanel /> : <FullDesktopPanel />}
    </main>
  );
}

export default App;
