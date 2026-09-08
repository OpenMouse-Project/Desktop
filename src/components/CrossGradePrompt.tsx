import { useState } from "preact/hooks";
import { TitleBar } from "./TitleBar";
import { requestVariant } from "../lib/cross-grade";

interface Props {
  /** Flips the persisted mode back to "bridge" so this build has something to render. */
  onStayInBridge: () => void;
}

/**
 * Shown by App.bridge.tsx when the persisted app mode is "full-desktop"
 * but this install is the bridge-only build, which never bundles
 * FullDesktopView. requestVariant (lib/cross-grade.ts) either launches an
 * already-installed Desktop build or downloads+launches its installer —
 * either way this process exits on success, so `opening` only matters for
 * the failure case (offline, no matching release asset), where it falls
 * back to just opening the releases page and control returns here.
 */
export function CrossGradePrompt({ onStayInBridge }: Props) {
  const [opening, setOpening] = useState(false);

  async function handleGetDesktop() {
    setOpening(true);
    try {
      await requestVariant("full-desktop");
    } finally {
      setOpening(false);
    }
  }

  return (
    <div class="bridge-view">
      <TitleBar showMaximize={false} />
      <div class="cross-grade-prompt">
        <p class="cross-grade-title">Full Desktop isn't included in this build</p>
        <p class="cross-grade-body">
          This is the Bridge-only install, so the full device-configuration UI isn't downloaded here. Get OpenMouse
          Desktop to use it, or switch back to Bridge Mode.
        </p>
        <div class="cross-grade-actions">
          <button class="rescan-button" onClick={handleGetDesktop} disabled={opening}>
            {opening ? "Opening…" : "Get OpenMouse Desktop"}
          </button>
          <button class="rescan-button" onClick={onStayInBridge}>
            Stay in Bridge Mode
          </button>
        </div>
      </div>
    </div>
  );
}
