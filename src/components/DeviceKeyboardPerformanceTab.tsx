// Wooting Performance tab: kept for its keyboard-profile flash save.
//
// The actuation/RT clone tabs own the sliders now; this module keeps the
// confirm-gated 0x2A keyboard-profile save so the Save button has a host.
// Not rendered as its own tab — see DeviceKeyboardActuationTab.
import { useState } from "preact/hooks";
import { WOOTING_COMMAND } from "@openmouse/keyboard-protocol/wooting";
import type { HidInterfaceInfo } from "../native-hid/tauri-hid-device";
import { saveWootingProfile } from "../native-hid/write";
import { showToast } from "../lib/toast";
import { confirmFlashOverwrite, type KeyboardTabProps } from "./DeviceKeyboardShared";

interface Props extends KeyboardTabProps {
  info: HidInterfaceInfo;
}

export function DeviceKeyboardPerformanceTab({ info, status }: Props) {
  const [pending, setPending] = useState(false);

  async function saveKeyboard() {
    const slot = status.activeProfile ?? 0;
    const confirmed = await confirmFlashOverwrite(slot, "Keyboard profile (actuation + rapid trigger)");
    if (!confirmed) return;
    setPending(true);
    try {
      await saveWootingProfile(info, WOOTING_COMMAND.saveKeyboardProfile, slot, true);
      showToast(`Keyboard profile saved to flash slot ${slot + 1}.`, "success");
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error), "error");
    } finally {
      setPending(false);
    }
  }

  return (
    <div class="info-section">
      <span class="info-section-title">Keyboard profile</span>
      <div class="setting-row">
        <button class="rescan-button" disabled={pending} onClick={() => void saveKeyboard()}>Save keyboard profile to flash</button>
        <span class="setting-description">Overwrites onboard flash — blocking confirm first.</span>
      </div>
    </div>
  );
}
