import { useEffect, useState } from "preact/hooks";
import { invoke } from "@tauri-apps/api/core";
import { BridgeView } from "./layouts/BridgeView";
import { CrossGradePrompt } from "./components/CrossGradePrompt";
import { WelcomeChooser } from "./components/WelcomeChooser";
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
 * `mode` is persisted per-install now (lib.rs's mode.json lives in this
 * build's own, identifier-scoped app-data dir — separate from Desktop's),
 * so this build's own switchMode is the only thing that could ever set it
 * to "full-desktop" here, and it deliberately never does (see below) —
 * CrossGradePrompt below is a defensive fallback for that state, not a
 * path this code normally takes.
 */
function App() {
  // undefined = get_mode hasn't answered yet. null = it answered with "no
  // mode chosen yet" (first launch) — show WelcomeChooser.
  const [mode, setModeState] = useState<AppMode | null | undefined>(undefined);

  useEffect(() => {
    invoke<AppMode | null>("get_mode").then(setModeState);
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
      // should still come back up as Bridge (or WelcomeChooser, on a
      // fresh install) next launch, not get stuck showing CrossGradePrompt
      // with no installed Desktop to switch to.
      await requestVariant("full-desktop");
      return;
    }
    const confirmed = await invoke<AppMode>("set_mode", { mode: next });
    setModeState(confirmed);
  }

  if (mode === undefined) {
    return null;
  }

  if (mode === null) {
    return <WelcomeChooser onChooseBridge={() => switchMode("bridge")} onChooseFullDesktop={() => switchMode("full-desktop")} />;
  }

  return mode === "full-desktop" ? (
    <CrossGradePrompt onStayInBridge={() => switchMode("bridge")} />
  ) : (
    <BridgeView mode={mode} onModeChange={switchMode} />
  );
}

export default App;
