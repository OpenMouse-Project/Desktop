import { useEffect, useState } from "preact/hooks";
import { invoke } from "@tauri-apps/api/core";
import { BridgeView } from "./layouts/BridgeView";
import { FullDesktopView } from "./layouts/FullDesktopView";
import "./App.css";

type AppMode = "bridge" | "full-desktop";

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

  return mode === "bridge" ? (
    <BridgeView mode={mode} onModeChange={switchMode} />
  ) : (
    <FullDesktopView mode={mode} onModeChange={switchMode} />
  );
}

export default App;
