import { useEffect, useState } from "preact/hooks";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getVersion } from "@tauri-apps/api/app";
import { Minus, Square, X } from "lucide-preact";

const win = getCurrentWindow();

interface Props {
  /** Bridge Mode's popover is too small for maximize to make sense. */
  showMaximize?: boolean;
}

export function TitleBar({ showMaximize = true }: Props) {
  const [version, setVersion] = useState("");

  useEffect(() => {
    void getVersion().then(setVersion);
  }, []);

  return (
    <div class="titlebar">
      <div class="titlebar-left" data-tauri-drag-region>
        <span class="titlebar-title">OpenMouse Desktop</span>
        {version && <span class="titlebar-badge">v{version} · Beta</span>}
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
