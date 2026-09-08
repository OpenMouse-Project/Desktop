import { useEffect, useState } from "preact/hooks";
import { invoke } from "@tauri-apps/api/core";
import { BridgeView } from "./layouts/BridgeView";
import { CrossGradePrompt } from "./components/CrossGradePrompt";
import { registerVariant, requestVariant } from "./lib/cross-grade";

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
    registerVariant("bridge");
    if (localStorage.getItem(DISCORD_RPC_PREFERENCE) === "true") {
      void invoke("enable");
    }
  }, []);

  async function switchMode(next: AppMode) {
    if (next === "full-desktop") {
      // This build never bundles FullDesktopView, so there's nothing to
      // switch to locally. requestVariant either launches an
      // already-installed Desktop build and exits this process, or
      // downloads+launches its installer and exits — either way control
      // doesn't return here on success. Don't touch the persisted mode:
      // if it fails (offline, no matching release asset), this build
      // should still come back up as Bridge next launch, not get stuck
      // showing CrossGradePrompt with no installed Desktop to switch to.
      await requestVariant("full-desktop");
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
