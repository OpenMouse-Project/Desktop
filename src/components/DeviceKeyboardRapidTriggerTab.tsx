// Wooting Rapid Trigger tab (Wootility clone): 60% map + 3 cards.
//
// Defaults come from the 0x27 keyboard profile (f2 enable, f3/f4
// sensitivities, f7 continuous) — the standalone 0x36 profile answers
// empty on ARM and is never read. Sensitivity slider is HIGH (small mm)
// left → LOW (large mm) right, i.e. inverted versus the stored value.
// Split sensitivity is staged-only. Every Apply is staged-only:
// TODO(woot-per-key-write) — no per-key RT SET encoder exists, so Apply
// toasts "needs USB capture" and sends no HID.

import { useEffect, useState } from "preact/hooks";
import type { KeyboardStatus } from "@openmouse/keyboard-protocol/drivers/keyboard-types";
import { showToast } from "../lib/toast";
import { DeviceKeyboardMap, KEYBOARD_60_COUNT, type KeySelection } from "./DeviceKeyboardMap";

/** RT sensitivity range staged in mm; HIGH = small value (left). */
const SENS_MIN_MM = 0.05;
const SENS_MAX_MM = 2.0;
const SENS_STEP_MM = 0.05;

interface Props {
  status: KeyboardStatus;
}

export function DeviceKeyboardRapidTriggerTab({ status }: Props) {
  const profileFields = status.analogProfile?.profileFields ?? [];
  const field = (n: number) => profileFields.find((entry) => entry.field === n)?.value ?? null;
  const liveEnabled = field(2) !== null ? field(2) !== 0 : null;
  const liveContinuous = field(7) !== null ? field(7) !== 0 : null;

  const [selection, setSelection] = useState<KeySelection>({ selected: new Set(), lastSelected: null });
  const [stagedEnabled, setStagedEnabled] = useState(liveEnabled ?? false);
  const [stagedSens, setStagedSens] = useState(0.1);
  const [split, setSplit] = useState(false);
  const [stagedContinuous, setStagedContinuous] = useState(liveContinuous ?? false);
  const [pending, setPending] = useState(false);

  useEffect(() => {
    if (liveEnabled !== null) setStagedEnabled(liveEnabled);
  }, [liveEnabled]);
  useEffect(() => {
    if (liveContinuous !== null) setStagedContinuous(liveContinuous);
  }, [liveContinuous]);

  const count = selection.selected.size;

  function stageSens(value: number) {
    const clamped = Math.min(SENS_MAX_MM, Math.max(SENS_MIN_MM, value));
    setStagedSens(Math.round(clamped / SENS_STEP_MM) * SENS_STEP_MM);
  }

  function applyStaged(what: string) {
    setPending(true);
    try {
      // TODO(woot-per-key-write): no per-key RT SET encoder exists.
      showToast(`Per-key write needs USB capture — ${what} staged for ${count} key${count === 1 ? "" : "s"} only.`, "info");
    } finally {
      setPending(false);
    }
  }

  return (
    <div>
      <DeviceKeyboardMap
        badgeMm={0.1}
        selection={selection}
        onChange={setSelection}
        emptyHint="TO ADJUST RAPID TRIGGER, PLEASE SELECT ONE OR MORE KEYS FIRST"
        selectedHint={(n) => `${n} KEYS SELECTED`}
      />
      <div class="kb-cards">
        <div class="kb-card">
          <span class="kb-card-title">
            Enable Rapid Trigger
            <label class="switch">
              <input
                type="checkbox"
                checked={stagedEnabled}
                disabled={pending || count === 0}
                onChange={(event) => setStagedEnabled(event.currentTarget.checked)}
              />
              <span class="switch-track" />
            </label>
          </span>
          <p class="game-profile-hint">
            Rapid Trigger dynamically actuates and resets your key based on your intention to press or release the key.
            Rapid Trigger starts and ends after the actuation point.
          </p>
          <p class="kb-card-count">
            ENABLED ON <strong>{KEYBOARD_60_COUNT} KEYS</strong>
          </p>
          <div class="setting-row">
            <button class="rescan-button" disabled={pending || count === 0} onClick={() => applyStaged("Rapid Trigger enable")}>
              Apply enable
            </button>
          </div>
        </div>
        <div class="kb-card">
          <span class="kb-card-title">Rapid Trigger Sensitivity</span>
          <div class="setting-row">
            <span class="setting-description">Split sensitivity</span>
            <label class="switch">
              <input
                type="checkbox"
                checked={split}
                disabled={pending || count === 0}
                onChange={(event) => setSplit(event.currentTarget.checked)}
              />
              <span class="switch-track" />
            </label>
          </div>
          <span class="setting-description">SENSITIVITY</span>
          <div class="setting-row">
            <input
              type="range"
              min={SENS_MIN_MM}
              max={SENS_MAX_MM}
              step={SENS_STEP_MM}
              value={stagedSens}
              disabled={pending || count === 0}
              onInput={(event) => stageSens(Number((event.target as HTMLInputElement).value))}
              aria-label="Rapid Trigger sensitivity in millimetres (high = small)"
            />
            <span class="info-value info-value-mono">{stagedSens.toFixed(2)} mm</span>
          </div>
          <div class="setting-row">
            <span class="setting-description">HIGH</span>
            <span class="setting-description">LOW</span>
          </div>
          {split && <p class="game-profile-hint">Split press/release sensitivities are staged separately — same capture TODO on Apply.</p>}
          <div class="setting-row">
            <button class="rescan-button" disabled={pending || count === 0} onClick={() => applyStaged("sensitivity")}>
              Apply sensitivity
            </button>
          </div>
        </div>
        <div class="kb-card">
          <span class="kb-card-title">
            Continuous Rapid Trigger
            <label class="switch">
              <input
                type="checkbox"
                checked={stagedContinuous}
                disabled={pending || count === 0}
                onChange={(event) => setStagedContinuous(event.currentTarget.checked)}
              />
              <span class="switch-track" />
            </label>
          </span>
          <p class="game-profile-hint">
            When enabled, Rapid Trigger ends when the entire key is released. When disabled, Rapid Trigger ends at the
            actuation point.
          </p>
          <div class="setting-row">
            <button class="rescan-button" disabled={pending || count === 0} onClick={() => applyStaged("Continuous Rapid Trigger")}>
              Apply continuous
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
