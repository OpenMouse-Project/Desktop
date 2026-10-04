// Wooting Gamepad tab: mode label + group/bind counts.
//
// Read-only: binds (0x28) answer empty on ARM — binds live in the 0x29
// profile instead — so the tab renders the 0x29 mode selector (best-effort)
// plus group counts, never invented binds.

import type { KeyboardTabProps } from "./DeviceKeyboardShared";
import { rowHex } from "./DeviceKeyboardShared";

export function DeviceKeyboardGamepadTab({ status }: KeyboardTabProps) {
  const gamepad = status.gamepad ?? null;

  if (!gamepad || !gamepad.profile) {
    return (
      <div class="info-section">
        <span class="info-section-title">Gamepad</span>
        <p class="game-profile-hint">
          The gamepad profile (0x29) is silent on this slot — and 0x28 binds answer empty on every ARM board, so
          nothing is guessed. Groups appear when the board answers.
        </p>
      </div>
    );
  }

  const profile = gamepad.profile;
  return (
    <div class="info-section">
      <span class="info-section-title">Gamepad (slot {gamepad.slot + 1})</span>
      <div class="info-grid">
        <div class="info-row">
          <span class="info-label">Mode</span>
          <span class="info-value info-value-mono">{profile.mode ?? "—"}</span>
        </div>
        <div class="info-row">
          <span class="info-label">Groups</span>
          <span class="info-value">{profile.groups.length} bind/axis group{profile.groups.length === 1 ? "" : "s"}</span>
        </div>
        <div class="info-row">
          <span class="info-label">0x28 binds</span>
          <span class="info-value">Empty on ARM (live in 0x29)</span>
        </div>
      </div>
      {profile.groups.slice(0, 4).map((group, index) => (
        <div class="info-grid" key={index}>
          <div class="info-row">
            <span class="info-label">Group {index + 1}</span>
            <span class="info-value info-value-mono">{rowHex(group).slice(0, 48)}{group.length > 16 ? " …" : ""}</span>
          </div>
        </div>
      ))}
    </div>
  );
}
