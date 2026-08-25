import { useEffect, useState } from "preact/hooks";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getVersion } from "@tauri-apps/api/app";
import { Minus, Square, X } from "lucide-preact";
import { subscribeUpdateCheck } from "../lib/update-check";

const win = getCurrentWindow();

interface Props {
  /** Bridge Mode's popover is too small for maximize to make sense. */
  showMaximize?: boolean;
}

export function TitleBar({ showMaximize = true }: Props) {
  const [version, setVersion] = useState("");
  // lib/update-check.ts's background poll (every 30 min, for the life of
  // the app — see its own docs on why it lives at module scope rather than
  // a hook) is what actually drives this; TitleBar just reflects it.
  const [updateAvailable, setUpdateAvailable] = useState(false);

  useEffect(() => {
    void getVersion().then(setVersion);
  }, []);

  useEffect(() => subscribeUpdateCheck((state) => setUpdateAvailable(state.available !== null)), []);

  return (
    <div class="titlebar">
      <div class="titlebar-left" data-tauri-drag-region>
        <span class="titlebar-title">OpenMouse Desktop</span>
        {version && (
          <span
            class={`titlebar-badge ${updateAvailable ? "titlebar-badge-update" : ""}`}
            title={updateAvailable ? "An update is available — see Settings" : undefined}
          >
            v{version} · Beta
          </span>
        )}
      </div>
      <div class="titlebar-drag" data-tauri-drag-region />
      <div class="titlebar-controls">
        <button onClick={() => win.minimize()} title="Minimize">
          <Minus size={13} />
        </button>
        {showMaximize && (
          <button onClick={() => win.toggleMaximize()} title="Maximize">
            <Square size={10} />
          </button>
        )}
        <button class="titlebar-close" onClick={() => win.close()} title="Close">
          <X size={13} />
        </button>
      </div>
    </div>
  );
}
