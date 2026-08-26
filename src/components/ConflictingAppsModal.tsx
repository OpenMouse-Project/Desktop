import { useState } from "preact/hooks";
import { AlertTriangle } from "lucide-preact";
import type { ConflictingApp } from "../hooks/use-conflicting-apps";

interface Props {
  apps: ConflictingApp[];
  onDismissed: () => void;
}

export function ConflictingAppsModal({ apps, onDismissed }: Props) {
  const [retried, setRetried] = useState(false);

  if (apps.length === 0) return null;

  const names = [...new Set(apps.map((a) => a.label))];
  const list =
    names.length === 1
      ? names[0]
      : names.length === 2
        ? `${names[0]} and ${names[1]}`
        : `${names.slice(0, -1).join(", ")}, and ${names[names.length - 1]}`;

  function handleOk() {
    setRetried(true);
    onDismissed();
  }

  return (
    <div class="conflict-modal-backdrop">
      <div class="conflict-modal">
        <div class="conflict-modal-icon">
          <AlertTriangle size={28} />
        </div>
        <h2 class="conflict-modal-title">
          {retried ? "Still Running!" : "Conflicting Software Detected"}
        </h2>
        <p class="conflict-modal-body">
          {retried ? (
            <>
              <strong>{list}</strong> is <em>still</em> running. Come on bruh, just close the
              process — OpenMouse can't do its thing while it's open.
            </>
          ) : (
            <>
              <strong>{list}</strong> {names.length === 1 ? "is" : "are"} running and{" "}
              {names.length === 1 ? "is" : "are"} blocking OpenMouse from accessing your device.
            </>
          )}
        </p>
        <p class="conflict-modal-instructions">
          To fix this:
        </p>
        <ol class="conflict-modal-steps">
          <li>Quit <strong>{list}</strong> completely</li>
          <li>Open Task Manager and end any remaining background services</li>
          <li>Click the button below once done</li>
        </ol>
        <div class="conflict-modal-detected">
          <span class="conflict-modal-detected-label">Detected processes:</span>
          {names.map((n) => (
            <span key={n} class="conflict-modal-process-badge">{n}</span>
          ))}
        </div>
        <button class="conflict-modal-ok" onClick={handleOk}>
          {retried ? "I closed it — check again" : "I closed it — Continue"}
        </button>
      </div>
    </div>
  );
}
