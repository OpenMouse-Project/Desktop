import { X } from "lucide-preact";
import { CHANGELOG } from "../lib/changelog";

interface Props {
  onClose: () => void;
}

/** Read-only list of what's changed per version — lib/changelog.ts is the data, this just renders it. */
export function ChangelogModal({ onClose }: Props) {
  return (
    <div class="modal-overlay" onClick={onClose}>
      <div class="modal-panel" onClick={(event) => event.stopPropagation()}>
        <div class="modal-panel-header">
          <h2>Changelog</h2>
          <button class="icon-button" onClick={onClose} title="Close">
            <X size={16} />
          </button>
        </div>

        <div class="changelog-list">
          {CHANGELOG.map((entry) => (
            <div class="changelog-entry" key={entry.version}>
              <div class="changelog-entry-header">
                <span class="changelog-version">v{entry.version}</span>
                <span class="changelog-date">{entry.date}</span>
              </div>
              <ul>
                {entry.changes.map((change) => (
                  <li key={change}>{change}</li>
                ))}
              </ul>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
