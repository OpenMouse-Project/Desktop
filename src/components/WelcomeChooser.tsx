import { useState } from "preact/hooks";
import { Zap, LayoutDashboard, ArrowRight } from "lucide-preact";
import { TitleBar } from "./TitleBar";

interface Props {
  /** Sets this build's persisted mode to Bridge and shows BridgeView. Always synchronous — every build bundles BridgeView. */
  onChooseBridge: () => void;
  /**
   * Sets this build's persisted mode to Full Desktop. On the Desktop
   * build this is just as instant as onChooseBridge (FullDesktopView is
   * already bundled); on the Bridge build it's actually
   * requestVariant("full-desktop") — launches an already-installed
   * Desktop build, or downloads+launches its installer — so it can take a
   * moment and, on this build specifically, doesn't return on success
   * (the process exits).
   */
  onChooseFullDesktop: () => void | Promise<void>;
}

/**
 * Shown once, on first launch of either build, before a mode has ever
 * been chosen (get_mode returns null — see lib.rs's ModeState/mode.json).
 * Lays out what each mode actually is so the choice isn't a guess, and
 * makes explicit that it isn't permanent — Settings can always switch
 * later, cross-grading to a download if needed.
 */
export function WelcomeChooser({ onChooseBridge, onChooseFullDesktop }: Props) {
  const [startingDesktop, setStartingDesktop] = useState(false);

  async function handleFullDesktop() {
    setStartingDesktop(true);
    try {
      await onChooseFullDesktop();
    } finally {
      setStartingDesktop(false);
    }
  }

  return (
    <div class="welcome-chooser-shell">
      <TitleBar showMaximize={false} />
      <div class="welcome-chooser">
        <div class="welcome-chooser-body">
          <p class="welcome-chooser-title">Choose how to use OpenMouse</p>

          <div class="welcome-chooser-options">
            <button class="welcome-chooser-option" onClick={onChooseBridge} disabled={startingDesktop}>
              <span class="welcome-chooser-option-icon">
                <Zap size={16} />
              </span>
              <span class="welcome-chooser-option-copy">
                <span class="welcome-chooser-option-name">Bridge Mode</span>
                <span class="welcome-chooser-option-body">Lightweight tray companion — game detection, alerts.</span>
              </span>
              <ArrowRight class="welcome-chooser-option-arrow" size={14} />
            </button>

            <button class="welcome-chooser-option" onClick={handleFullDesktop} disabled={startingDesktop}>
              <span class="welcome-chooser-option-icon">
                <LayoutDashboard size={16} />
              </span>
              <span class="welcome-chooser-option-copy">
                <span class="welcome-chooser-option-name">Full Desktop Mode</span>
                <span class="welcome-chooser-option-body">
                  {startingDesktop ? "Starting…" : "Full device configuration — lighting, buttons, profiles."}
                </span>
              </span>
              <ArrowRight class="welcome-chooser-option-arrow" size={14} />
            </button>
          </div>
        </div>

        <p class="welcome-chooser-disclaimer">Switch anytime in Settings.</p>
      </div>
    </div>
  );
}
