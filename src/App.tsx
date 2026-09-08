import { useEffect, useState } from "preact/hooks";
import { invoke } from "@tauri-apps/api/core";
import { BridgeView } from "./layouts/BridgeView";
import { FullDesktopView } from "./layouts/FullDesktopView";
import { registerVariant } from "./lib/cross-grade";

type AppMode = "bridge" | "full-desktop";
const DISCORD_RPC_PREFERENCE = "openmouse.discord-rpc.enabled";

function App() {
  const [mode, setModeState] = useState<AppMode | null>(null);

  useEffect(() => {
    invoke<AppMode>("get_mode").then(setModeState);
    // This is the desktop build's entry point — see App.bridge.tsx for the
    // bridge build's — so it always registers "full-desktop" regardless of
    // which UI mode is currently showing (that's a separate, purely
    // cosmetic runtime toggle; see AppMode). lib/cross-grade.ts's shared
    // manifest is what lets the *other* build find this install later.
    registerVariant("full-desktop");
    if (localStorage.getItem(DISCORD_RPC_PREFERENCE) === "true") {
      void invoke("enable");
    }
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
