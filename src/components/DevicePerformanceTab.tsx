import { useEffect, useState } from "preact/hooks";
import { AlertTriangle } from "lucide-preact";
import type { MouseStatus } from "@openmouse/protocol/drivers/mouse-types";
import type { HidInterfaceInfo } from "../native-hid/tauri-hid-device";
import {
  setDpi as logitechSetDpi,
  setGamingSurfaceMode,
  setLiftOffDistance,
  setPollingRate as logitechSetPollingRate,
} from "../native-hid/logitech-actions";
import {
  setDpi as razerSetDpi,
  setPollingRate as razerSetPollingRate,
} from "../native-hid/razer-actions";
import { showToast } from "../lib/toast";
import { DPI_MAX as LOGITECH_DPI_MAX, DPI_MIN as LOGITECH_DPI_MIN, DPI_PRESETS as LOGITECH_DPI_PRESETS, DPI_STEP as LOGITECH_DPI_STEP, GAMING_SURFACE_MODES } from "../lib/logitech-controls";

interface Props {
  info: HidInterfaceInfo;
  status: MouseStatus;
  brand: string;
  onApplied: (patch: Partial<MouseStatus>) => void;
  lockedBy?: string;
  readOnly?: boolean;
}

export function DevicePerformanceTab({ info, status, brand, onApplied, lockedBy, readOnly }: Props) {
  const locked = lockedBy !== undefined;
  const [pending, setPending] = useState(false);

  // Staged values — what the user has selected but NOT yet applied to the device.
  const [stagedDpi, setStagedDpi] = useState(status.dpi);
  const [stagedPollingRate, setStagedPollingRate] = useState(status.pollingRateHz);
  const [stagedLod, setStagedLod] = useState(status.liftOffDistance);
  const [stagedSurface, setStagedSurface] = useState(status.gamingSurfaceMode);

  // Keep staging in sync with device status (after a successful apply or background refresh).
  useEffect(() => { setStagedDpi(status.dpi); }, [status.dpi]);
  useEffect(() => { setStagedPollingRate(status.pollingRateHz); }, [status.pollingRateHz]);
  useEffect(() => { setStagedLod(status.liftOffDistance); }, [status.liftOffDistance]);
  useEffect(() => { setStagedSurface(status.gamingSurfaceMode); }, [status.gamingSurfaceMode]);

  const isRazer = brand === "Razer";
  const dpiPresets = isRazer ? [400, 800, 1600, 3200, 6400, 8000] : LOGITECH_DPI_PRESETS;
  const dpiMin = isRazer ? 100 : LOGITECH_DPI_MIN;
  const dpiMax = isRazer ? 8500 : LOGITECH_DPI_MAX;
  const dpiStep = isRazer ? 100 : LOGITECH_DPI_STEP;

  const dirty =
    stagedDpi !== status.dpi ||
    stagedPollingRate !== status.pollingRateHz ||
    stagedLod !== status.liftOffDistance ||
    stagedSurface !== status.gamingSurfaceMode;

  const busy = pending || locked || readOnly;

  function revert() {
    setStagedDpi(status.dpi);
    setStagedPollingRate(status.pollingRateHz);
    setStagedLod(status.liftOffDistance);
    setStagedSurface(status.gamingSurfaceMode);
  }

  async function applyAll() {
    setPending(true);
    try {
      const setDpi = brand === "Razer" ? razerSetDpi : logitechSetDpi;
      const setPollingRate = brand === "Razer" ? razerSetPollingRate : logitechSetPollingRate;
      const patch: Partial<MouseStatus> = {};

      if (stagedDpi !== status.dpi) {
        const applied = await setDpi(info, Math.round(stagedDpi));
        patch.dpi = applied;
      }
      if (stagedPollingRate !== status.pollingRateHz) {
        const applied = await setPollingRate(info, stagedPollingRate);
        patch.pollingRateHz = applied;
      }
      if (stagedLod !== status.liftOffDistance && stagedLod != null) {
        const applied = await setLiftOffDistance(info, stagedLod);
        patch.liftOffDistance = applied;
      }
      if (stagedSurface !== status.gamingSurfaceMode && stagedSurface != null) {
        const applied = await setGamingSurfaceMode(info, stagedSurface);
        patch.gamingSurfaceMode = applied;
      }

      if (Object.keys(patch).length > 0) {
        onApplied(patch);
        showToast("Settings applied.", "success");
      }
    } catch (err) {
      showToast(err instanceof Error ? err.message : String(err), "error");
    } finally {
      setPending(false);
    }
  }

  return (
    <div class="performance-tab">
      {locked && (
        <p class="performance-locked-note">
          Locked while <strong>{lockedBy}</strong> is running — this is your default, and it'll be editable again once the game closes.
        </p>
      )}

      {readOnly && !locked && (
        <p class="performance-locked-note">
          Read-only — live controls are not available for this device yet. Showing current values.
        </p>
      )}

      {/* ── DPI ──────────────────────────────────────────────────── */}
      <div class="dpi-panel">
        <div class="dpi-panel-header">
          <div class="setting-label">
            <span class="setting-eyebrow">DPI</span>
            <span class="setting-title">Sensitivity</span>
          </div>
          <div class="dpi-panel-value-group">
            <span class={`dpi-panel-value ${stagedDpi !== status.dpi ? "dpi-panel-value--dirty" : ""}`}>
              {stagedDpi.toLocaleString()} DPI
            </span>
            {!dpiPresets.includes(stagedDpi) && <span class="dpi-panel-badge">Custom</span>}
          </div>
        </div>

        <div class="dpi-preset-grid">
          {dpiPresets.map((preset) => (
            <button
              key={preset}
              class={`dpi-preset ${preset === stagedDpi ? "active" : ""}`}
              disabled={busy}
              onClick={() => setStagedDpi(preset)}
            >
              {preset.toLocaleString()}
            </button>
          ))}
        </div>

        <div class="dpi-axis-row">
          <label class="dpi-axis-field">
            <span>Custom DPI ({dpiMin}–{dpiMax.toLocaleString()})</span>
            <input
              type="number"
              min={dpiMin}
              max={dpiMax}
              step={dpiStep}
              placeholder={`e.g. ${dpiPresets[2]}`}
              value={String(stagedDpi)}
              disabled={busy}
              onInput={(e) => {
                const v = Number((e.target as HTMLInputElement).value);
                if (Number.isFinite(v) && v > 0) setStagedDpi(v);
              }}
              onKeyDown={(e) => { if (e.key === "Enter") void applyAll(); }}
            />
          </label>
        </div>

        <p class="dpi-current-caption">Current {status.dpi} DPI</p>
      </div>

      {/* ── Polling rate ─────────────────────────────────────────── */}
      {status.supportedPollingRates && status.supportedPollingRates.length > 0 && (
        <div class="setting-row">
          <div class="setting-label">
            <span class="setting-title">Polling rate</span>
            <span class="setting-description">How often the mouse reports its position.</span>
          </div>
          <div class="performance-chip-group">
            {status.supportedPollingRates.map((hz) => (
              <button
                key={hz}
                class={`performance-chip ${hz === stagedPollingRate ? "active" : ""}`}
                disabled={busy}
                onClick={() => setStagedPollingRate(hz)}
              >
                {hz} Hz
              </button>
            ))}
          </div>
        </div>
      )}

      {/* ── Sensor ───────────────────────────────────────────────── */}
      {(status.gamingSurfaceMode != null || (status.supportedLiftOffDistances && status.supportedLiftOffDistances.length > 0)) && (
        <div class="sensor-panel">
          <span class="setting-eyebrow">Sensor</span>

          {status.gamingSurfaceMode != null && (
            <>
              <span class="sensor-panel-title">Gaming surface</span>
              <div class="segmented-group">
                {GAMING_SURFACE_MODES.map((mode) => (
                  <button
                    key={mode}
                    class={mode === stagedSurface ? "active" : ""}
                    disabled={busy}
                    onClick={() => setStagedSurface(mode)}
                  >
                    {mode}
                  </button>
                ))}
              </div>
              <p class="sensor-panel-description">
                Tunes the sensor for gaming mouse pads. Auto lets the mouse decide; turn it off if tracking misbehaves on a non-gaming surface.
              </p>
            </>
          )}

          {status.supportedLiftOffDistances && status.supportedLiftOffDistances.length > 0 && (
            <>
              <span class="sensor-panel-title">Lift-off distance</span>
              <div class="segmented-group">
                {status.supportedLiftOffDistances.map((value) => (
                  <button
                    key={value}
                    class={value === stagedLod ? "active" : ""}
                    disabled={busy}
                    onClick={() => setStagedLod(value)}
                  >
                    {value}
                  </button>
                ))}
              </div>
              <p class="sensor-panel-description">
                Controls how far you can lift the mouse before tracking stops. Higher values keep tracking a little longer.
              </p>
            </>
          )}
        </div>
      )}

      {/* ── Apply bar ────────────────────────────────────────────── */}
      {dirty && !readOnly && (
        <div class="apply-bar">
          <span class="apply-bar-label"><AlertTriangle size={14} class="apply-bar-icon" /> You have unsaved changes</span>
          <div class="apply-bar-actions">
            <button class="apply-bar-revert" disabled={pending} onClick={revert}>Revert</button>
            <button class="apply-bar-apply" disabled={pending} onClick={() => void applyAll()}>
              {pending ? "Applying…" : "Apply Changes"}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
