// Wooting Profiles tab: slot switcher + flash health + analog snapshot.
//
// The switcher stages the target slot; Activate is stubbed — the slot
// sequence needs a Wootility capture, and persisting overwrites flash.
// Diagnostics render only what the board answered: 0x66 counts stay hidden
// (keyCount/rgbProfileCount/analogProfileCount are null on ARM), flash
// health shows when present, and the snapshot shows a "keys at rest"
// indicator instead of a fake live view.

import { useEffect, useState } from "preact/hooks";
import { WOOTING_COMMAND } from "@openmouse/keyboard-protocol/wooting";
import type { HidInterfaceInfo } from "../native-hid/tauri-hid-device";
import { saveWootingProfile, switchWootingProfile } from "../native-hid/write";
import { showToast } from "../lib/toast";
import { confirmFlashOverwrite, type KeyboardTabProps } from "./DeviceKeyboardShared";

interface Props extends KeyboardTabProps {
  info: HidInterfaceInfo;
}

export function DeviceKeyboardProfilesTab({ info, status, onApplied }: Props) {
  const count = status.profileCount ?? status.profileNames.length;
  const active = status.activeProfile ?? 0;
  const [staged, setStaged] = useState(active);
  const [pending, setPending] = useState(false);

  useEffect(() => {
    setStaged(status.activeProfile ?? 0);
  }, [status.activeProfile]);

  const dirty = staged !== active;
  const diagnostics = status.diagnostics ?? null;
  const snapshot = status.analogSnapshot ?? null;

  // RAM-only switch: plain button, no confirm. Verifies 0x0b and patches
  // the live slot; throws surface as error toasts, never a guessed switch.
  async function switchStaged() {
    setPending(true);
    try {
      const applied = await switchWootingProfile(info, staged);
      onApplied({ activeProfile: applied });
      showToast(`Switched to profile ${applied + 1} (RAM — flash default unchanged).`, "success");
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error), "error");
    } finally {
      setPending(false);
    }
  }

  // FLASH save: blocking "overwrites onboard flash slot N" modal first,
  // then saveProfile with the modal's answer as confirmed.
  async function saveStaged(commandId: number, what: string) {
    const confirmed = await confirmFlashOverwrite(staged, what);
    if (!confirmed) return;
    setPending(true);
    try {
      await saveWootingProfile(info, commandId, staged, true);
      showToast(`${what} saved to flash slot ${staged + 1}.`, "success");
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error), "error");
    } finally {
      setPending(false);
    }
  }

  return (
    <div class="info-section">
      <span class="info-section-title">Profiles</span>
      {count === 0 || count === null ? (
        <p class="game-profile-hint">No onboard profiles reported — the switcher appears when the board names a slot.</p>
      ) : (
        <>
          <div class="performance-chip-group">
            {Array.from({ length: count }, (_, slot) => {
              const name = status.profileNames[slot];
              return (
                <button
                  key={slot}
                  class={`performance-chip ${slot === staged ? "active" : ""}`}
                  disabled={pending}
                  onClick={() => setStaged(slot)}
                >
                  {name ? `${slot + 1}: ${name}` : `Profile ${slot + 1}`}
                  {slot === active ? " ●" : ""}
                </button>
              );
            })}
          </div>
          <div class="setting-row">
            <button class="rescan-button" disabled={pending || !dirty} onClick={() => void switchStaged()}>
              {pending ? "Switching…" : "Switch profile (RAM)"}
            </button>
            {dirty && <span class="setting-description">RAM-only — the flash default stays where it is.</span>}
          </div>
          <div class="setting-row">
            <button class="rescan-button" disabled={pending} onClick={() => void saveStaged(WOOTING_COMMAND.saveKeyboardProfile, "Keyboard profile")}>
              Save profile to flash
            </button>
            <span class="setting-description">Overwrites onboard flash — blocking confirm first.</span>
          </div>
        </>
      )}

      <span class="info-section-title">Flash health</span>
      {diagnostics && (diagnostics.flashConnected !== null || diagnostics.flashStats) ? (
        <div class="info-grid">
          <div class="info-row">
            <span class="info-label">Flash chip</span>
            <span class="info-value">{diagnostics.flashConnected === null ? "—" : diagnostics.flashConnected ? "Connected" : "Not detected"}</span>
          </div>
          {diagnostics.flashStats && (
            <div class="info-row">
              <span class="info-label">Pages used</span>
              <span class="info-value info-value-mono">
                {diagnostics.flashStats.usedPages ?? "—"} / {diagnostics.flashStats.totalPages ?? "—"}
              </span>
            </div>
          )}
          {diagnostics.keyCount !== null && (
            <div class="info-row">
              <span class="info-label">Keys</span>
              <span class="info-value info-value-mono">{diagnostics.keyCount}</span>
            </div>
          )}
        </div>
      ) : (
        <p class="game-profile-hint">Flash health silent — counts that answer 0x66 on ARM stay hidden, never guessed.</p>
      )}

      <span class="info-section-title">Analog snapshot</span>
      {snapshot?.available ? (
        <div class="info-grid">
          <div class="info-row">
            <span class="info-label">State</span>
            <span class="info-value">{snapshot.keys.length === 0 ? "Keys at rest" : `${snapshot.keys.length} keys pressed`}</span>
          </div>
        </div>
      ) : (
        <p class="game-profile-hint">One-shot snapshot (0x14) unavailable — the streaming 0xFF53 interface covers live keys instead.</p>
      )}
    </div>
  );
}
