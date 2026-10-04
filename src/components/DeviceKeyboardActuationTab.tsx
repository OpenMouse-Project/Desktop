// Wooting Actuation tab (Wootility clone): 60% map + Set Actuation Point
// slider + Visual Feedback press-tester.
//
// Every badge shows the profile default (0x27 f1 → actuationMm) — per-key
// overrides are NOT readable on ARM (0x31 empty-OK), so variance is never
// invented. Slider clamps 0.1–4.0mm step 0.05. Apply is staged-only:
// TODO(woot-per-key-write) — per-key SET encoders do not exist, so Apply
// toasts "needs USB capture" and sends no HID. Visual Feedback wires to
// the analog interface when the transport exposes one, else a static hint.

import { useEffect, useState } from "preact/hooks";
import type { KeyboardStatus } from "@openmouse/keyboard-protocol/drivers/keyboard-types";
import { showToast } from "../lib/toast";
import { DeviceKeyboardMap, type KeySelection } from "./DeviceKeyboardMap";

/** Wootility actuation range for the 60HE+: millimetres. */
const ACTUATION_MIN_MM = 0.1;
const ACTUATION_MAX_MM = 4.0;
const ACTUATION_STEP_MM = 0.05;

interface Props {
  status: KeyboardStatus;
}

export function DeviceKeyboardActuationTab({ status }: Props) {
  const liveMm = status.analogProfile?.actuationMm ?? null;
  const [selection, setSelection] = useState<KeySelection>({ selected: new Set(), lastSelected: null });
  const [stagedMm, setStagedMm] = useState(liveMm ?? 0.2);
  const [pending, setPending] = useState(false);
  // Visual feedback: deepest pressed depth in mm (0–4), null = at rest.
  const [pressedMm, setPressedMm] = useState<number | null>(null);

  useEffect(() => {
    if (liveMm !== null) setStagedMm(liveMm);
  }, [liveMm]);

  const count = selection.selected.size;
  const dirty = liveMm !== null && Math.abs(stagedMm - liveMm) > 1e-9;

  function stageFromSlider(value: number) {
    const clamped = Math.min(ACTUATION_MAX_MM, Math.max(ACTUATION_MIN_MM, value));
    setStagedMm(Math.round(clamped / ACTUATION_STEP_MM) * ACTUATION_STEP_MM);
  }

  function applyStaged() {
    setPending(true);
    try {
      // TODO(woot-per-key-write): per-key actuation SET encoders do not
      // exist — staged only, no HID send beyond switchProfile elsewhere.
      showToast(
        `Per-key write needs USB capture — actuation ${stagedMm.toFixed(2)}mm staged for ${count} key${count === 1 ? "" : "s"} only.`,
        "info",
      );
    } finally {
      setPending(false);
    }
  }

  const travelPct = pressedMm === null ? 0 : Math.min(100, (pressedMm / 4.0) * 100);
  const actuated = pressedMm !== null && pressedMm >= stagedMm;

  return (
    <div>
      <DeviceKeyboardMap
        badgeMm={liveMm}
        selection={selection}
        onChange={setSelection}
        emptyHint="TO ADJUST ACTUATION POINT, PLEASE SELECT ONE OR MORE KEYS FIRST"
        selectedHint={(n) => `${n} KEYS SELECTED`}
      />
      <div class="kb-feedback">
        <div class="kb-card">
          <span class="kb-card-title">Set Actuation Point</span>
          <p class="game-profile-hint">
            Customize the actuation point by setting the exact distance a key must be pressed before it registers a keypress.
          </p>
          <p class="kb-card-count">
            CHANGING ACTUATION POINT FOR <strong>{count} key{count === 1 ? "" : "s"}</strong>
          </p>
          <div class="setting-row">
            <input
              type="range"
              min={ACTUATION_MIN_MM}
              max={ACTUATION_MAX_MM}
              step={ACTUATION_STEP_MM}
              value={stagedMm}
              disabled={pending || count === 0}
              onInput={(event) => stageFromSlider(Number((event.target as HTMLInputElement).value))}
              aria-label="Staged actuation point in millimetres"
            />
            <span class="info-value info-value-mono">{stagedMm.toFixed(2)} mm</span>
          </div>
          <div class="setting-row">
            <button class="rescan-button" disabled={pending || count === 0} onClick={applyStaged}>
              {pending ? "Staging…" : `Apply to ${count} key${count === 1 ? "" : "s"}`}
            </button>
            {dirty && <span class="setting-description">Live {liveMm?.toFixed(2)}mm kept until applied.</span>}
          </div>
          {liveMm === null && (
            <p class="game-profile-hint">Board didn't report a profile actuation — badges show — instead of a guess.</p>
          )}
        </div>
        <div class="kb-card">
          <span class="kb-card-title">Visual Feedback</span>
          <p class="game-profile-hint">
            Press a key to test the actuation point. The key is activated when the slider turns yellow.
          </p>
          <span class="setting-description">PRESSED KEY</span>
          <div class="kb-travel">
            <div class="kb-key">
              <span class="kb-key-label">{selection.lastSelected ?? "Key"}</span>
            </div>
            <div class="kb-travel-track" aria-hidden="true">
              <div
                class={`kb-travel-fill${actuated ? " kb-travel-fill-live" : ""}`}
                style={{ height: `${travelPct}%` }}
              />
              <div
                class="kb-travel-marker"
                style={{ bottom: `${(stagedMm / 4.0) * 100}%` }}
                title={`Actuation ${stagedMm.toFixed(2)}mm`}
              />
            </div>
            <div class="setting-row">
              <button
                class="rescan-button"
                onClick={() => setPressedMm((prev) => (prev === null ? stagedMm + 0.3 : null))}
              >
                {pressedMm === null ? "Simulate press" : "Release"}
              </button>
            </div>
          </div>
          <p class="game-profile-hint">
            Live analog stream wires here when the transport exposes the 0xFF53 interface — static simulation until then.
          </p>
        </div>
      </div>
    </div>
  );
}
