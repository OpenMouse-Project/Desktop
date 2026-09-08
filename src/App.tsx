import { useEffect } from "preact/hooks";
import { invoke } from "@tauri-apps/api/core";
import { FullDesktopView } from "./layouts/FullDesktopView";

const DISCORD_RPC_PREFERENCE = "openmouse.discord-rpc.enabled";

function App() {
  useEffect(() => {
    if (localStorage.getItem(DISCORD_RPC_PREFERENCE) === "true") {
      void invoke("enable");
    }
  }, []);

  return <FullDesktopView />;
}

export default App;
