import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-preact";

const win = getCurrentWindow();

interface Props {
  /** Bridge Mode's popover is too small for maximize to make sense. */
  showMaximize?: boolean;
}

export function TitleBar({ showMaximize = true }: Props) {
  return (
    <div class="titlebar">
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
