import type { Update } from "@tauri-apps/plugin-updater";
import { Download, X } from "lucide-preact";

interface Props {
  update: Update;
  installing: boolean;
  onInstall: () => void;
  onDismiss: () => void;
}

/**
 * Confirmation step between "found an update" and actually downloading and
 * installing it. Settings' "Check for Updates" used to go straight from
 * check() into downloadAndInstall() + relaunch() the moment it found
 * anything — CONFIRMED as a real surprise, not a hypothetical one: clicking
 * a button that says "check" shouldn't be able to close the app out from
 * under you a few seconds later with zero chance to back out. This is the
 * chance to back out.
 */
export function UpdateAvailableModal({ update, installing, onInstall, onDismiss }: Props) {
  return (
    <div class="modal-overlay" onClick={() => !installing && onDismiss()}>
      <div class="modal-panel" onClick={(event) => event.stopPropagation()}>
        <div class="modal-panel-header">
          <h2>Update available</h2>
          <button class="icon-button" onClick={onDismiss} title="Close" disabled={installing}>
            <X size={16} />
          </button>
        </div>

        <p class="update-modal-version">
          OpenMouse Desktop <strong>v{update.version}</strong>
        </p>

        {update.body && <p class="update-modal-notes">{update.body}</p>}

        <p class="update-modal-hint">
          {installing
            ? "Downloading and installing — the app will restart automatically when it's done."
            : "Installing will download the update and restart the app."}
        </p>

        <div class="modal-panel-footer">
          <button class="rescan-button" onClick={onDismiss} disabled={installing}>
            Later
          </button>
          <button class="connect-button" onClick={onInstall} disabled={installing}>
            <Download size={14} /> {installing ? "Installing…" : `Install v${update.version}`}
          </button>
        </div>
      </div>
    </div>
  );
}
