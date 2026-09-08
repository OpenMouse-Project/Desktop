import { useEffect, useState } from "preact/hooks";
import { invoke } from "@tauri-apps/api/core";
import { BridgeView } from "./layouts/BridgeView";
import { CrossGradePrompt } from "./components/CrossGradePrompt";

type AppMode = "bridge" | "full-desktop";
const DISCORD_RPC_PREFERENCE = "openmouse.discord-rpc.enabled";

/**
 * Entry point for the bridge-only build (see vite.config.ts's `~app-entry`
 * alias and package.json's `build:bridge`). Deliberately does NOT import
 * FullDesktopView or anything under it — that's the whole point of this
 * file existing separately from App.tsx: it keeps the device-config tabs,
 * device images, and game-profile code out of this build's module graph
 * entirely, so the Bridge installer doesn't ship megabytes of UI a
 * bridge-only user never opens.
 *
 * `mode` still comes from the same Rust-side ModeState as the full-desktop
 * build (it's the same persisted preference), so if this install's mode
 * was ever flipped to "full-desktop" — most likely by a Desktop install
 * sharing the same app-data preference, or a leftover from before the
 * user switched to the bridge-only download — we can't render
 * FullDesktopView locally. CrossGradePrompt offers to fetch the real
 * Desktop build instead of silently doing nothing.
 */
function App() {
  const [mode, setModeState] = useState<AppMode | null>(null);

  useEffect(() => {
    invoke<AppMode>("get_mode").then(setModeState);
    if (localStorage.getItem(DISCORD_RPC_PREFERENCE) === "true") {
      void invoke("enable");
    }
  }, []);

  async function switchMode(next: AppMode) {
    if (next === "full-desktop") {
      // Don't tell the Rust side we're in full-desktop mode when this
      // build can't render it — leave the persisted mode alone so a
      // relaunch (e.g. after actually installing Desktop) isn't left in a
      // state this build can't show.
      return;
    }
    const confirmed = await invoke<AppMode>("set_mode", { mode: next });
    setModeState(confirmed);
  }

  if (mode === null) {
    return null;
  }

  return mode === "full-desktop" ? (
    <CrossGradePrompt onStayInBridge={() => switchMode("bridge")} />
  ) : (
    <BridgeView mode={mode} onModeChange={switchMode} />
  );
}

export default App;
