import { useEffect, useRef, useState } from "preact/hooks";
import type { MouseStatus } from "@openmouse/protocol/drivers/mouse-types";
import type { HidInterfaceInfo } from "../native-hid/tauri-hid-device";
import {
  setDpi,
  setGamingSurfaceMode,
  setLiftOffDistance,
  setPollingRate,
  type GamingSurfaceMode,
  type LiftOffDistance,
} from "../native-hid/logitech-actions";
import { showToast } from "../lib/toast";
import { DPI_MAX, DPI_MIN, DPI_PRESETS, DPI_STEP, GAMING_SURFACE_MODES } from "../lib/logitech-controls";

interface Props {
  info: HidInterfaceInfo;
  status: MouseStatus;
  /** Patches the cached status with the value the device actually applied. */
  onApplied: (patch: Partial<MouseStatus>) => void;
  /** Name of the game whose profile currently owns the mouse's live settings, if any — see use-game-watcher.ts. */
  lockedBy?: string;
}

/**
 * Writable controls for the settings a Logitech HID++ mouse can change live
 * (no flash write, no "confirm with the user first" warning in the driver's
 * own docs — see native-hid/logitech-actions.ts). Each control opens its own
 * short-lived connection per edit, same as a status read; only one write is
 * ever in flight at a time (see the `pending` guard below) since two of this
 * tab's own controls sharing the same open device handle at once risks the
 * same cross-talk `hid.rs`'s dedup fix exists for on the read side.
 *
 * These controls ARE the "default" a game profile (lib/game-profiles.ts)
 * restores to when the game closes — editing them while a profile has
 * already taken over would just get silently overwritten again next launch
 * and, worse, redefine what "default" even means mid-override. `lockedBy`
 * disables every control here for exactly as long as that's true, rather
 * than let a value someone changes here quietly stop meaning anything.
 */
export function DevicePerformanceTab({ info, status, onApplied, lockedBy }: Props) {
  const locked = lockedBy !== undefined;
  const [pending, setPending] = useState<"dpi" | "rate" | "lod" | "surface" | null>(null);
  const [dpiXInput, setDpiXInput] = useState(String(status.dpi));
  const [dpiYInput, setDpiYInput] = useState(String(status.dpiY ?? status.dpi));
  // The connection's background auto-refresh (use-mouse-connection.ts, every
  // 5s, paused while this tab is open — see OverviewPage) could otherwise
  // land while the user has typed a new value but not hit Apply yet and
  // silently wipe it back to the device's current value. Belt-and-suspenders
  // even with that pause in place.
  const dpiXFocused = useRef(false);
  const dpiYFocused = useRef(false);

  // Keep the editable fields in sync with the cached status — including
  // after this same control's own apply corrects it to whatever the device
  // actually accepted — but only while the user isn't actively typing.
  useEffect(() => {
    if (!dpiXFocused.current) setDpiXInput(String(status.dpi));
  }, [status.dpi]);
  useEffect(() => {
    if (!dpiYFocused.current) setDpiYInput(String(status.dpiY ?? status.dpi));
  }, [status.dpiY, status.dpi]);

  async function applyDpiValues(dpi: number, dpiY?: number) {
    if (!Number.isFinite(dpi) || dpi <= 0) return;
    if (dpi === status.dpi && (dpiY === undefined || dpiY === (status.dpiY ?? status.dpi))) return;
    setPending("dpi");
    try {
      const applied = await setDpi(info, Math.round(dpi), dpiY !== undefined ? Math.round(dpiY) : undefined);
      onApplied({ dpi: applied });
      showToast(`DPI set to ${applied}.`, "success");
    } catch (err) {
      showToast(err instanceof Error ? err.message : String(err), "error");
    } finally {
      setPending(null);
    }
  }

  async function applyPollingRate(hz: number) {
    if (hz === status.pollingRateHz) return;
    setPending("rate");
    try {
      const applied = await setPollingRate(info, hz);
      onApplied({ pollingRateHz: applied });
      showToast(`Polling rate set to ${applied} Hz.`, "success");
    } catch (err) {
      showToast(err instanceof Error ? err.message : String(err), "error");
    } finally {
      setPending(null);
    }
  }

  async function applyLiftOffDistance(value: LiftOffDistance) {
    if (value === status.liftOffDistance) return;
    setPending("lod");
    try {
      const applied = await setLiftOffDistance(info, value);
      onApplied({ liftOffDistance: applied });
      showToast(`Lift-off distance set to ${applied}.`, "success");
    } catch (err) {
      showToast(err instanceof Error ? err.message : String(err), "error");
    } finally {
      setPending(null);
    }
  }

  async function applyGamingSurfaceMode(mode: GamingSurfaceMode) {
    if (mode === status.gamingSurfaceMode) return;
    setPending("surface");
    try {
      const applied = await setGamingSurfaceMode(info, mode);
      onApplied({ gamingSurfaceMode: applied });
      showToast(`Gaming surface set to ${applied}.`, "success");
    } catch (err) {
      showToast(err instanceof Error ? err.message : String(err), "error");
    } finally {
      setPending(null);
    }
  }

  // Folding `locked` into `busy` disables every control below (they all
  // already gate on `busy`) without needing to touch each one individually.
  const busy = pending !== null || locked;
  const isCustomDpi = !DPI_PRESETS.includes(status.dpi);
  const showSeparateAxes = status.supportsSeparateDpiAxes === true;

  return (
    <div class="performance-tab">
      {locked && (
        <p class="performance-locked-note">
          Locked while <strong>{lockedBy}</strong> is running — this is your default, and it'll be editable again once the game closes.
        </p>
      )}

      <div class="dpi-panel">
        <div class="dpi-panel-header">
          <div class="setting-label">
            <span class="setting-eyebrow">DPI</span>
            <span class="setting-title">Sensitivity</span>
          </div>
          <div class="dpi-panel-value-group">
            <span class="dpi-panel-value">{status.dpi.toLocaleString()} DPI</span>
            {isCustomDpi && <span class="dpi-panel-badge">Custom</span>}
          </div>
        </div>

        <div class="dpi-preset-grid">
          {DPI_PRESETS.map((preset) => (
            <button
              key={preset}
              class={`dpi-preset ${preset === status.dpi ? "active" : ""}`}
              disabled={busy}
              onClick={() => void applyDpiValues(preset, showSeparateAxes ? preset : undefined)}
            >
              {preset.toLocaleString()}
            </button>
          ))}
        </div>

        <div class="dpi-axis-row">
          <label class="dpi-axis-field">
            <span>{showSeparateAxes ? "X axis" : "DPI"}</span>
            <input
              type="number"
              min={DPI_MIN}
              max={DPI_MAX}
              step={DPI_STEP}
              value={dpiXInput}
              disabled={busy}
              onFocus={() => { dpiXFocused.current = true; }}
              onBlur={() => { dpiXFocused.current = false; }}
              onInput={(event) => setDpiXInput((event.target as HTMLInputElement).value)}
              onKeyDown={(event) => { if (event.key === "Enter") void applyDpiValues(Number(dpiXInput), showSeparateAxes ? Number(dpiYInput) : undefined); }}
            />
          </label>
          {showSeparateAxes && (
            <label class="dpi-axis-field">
              <span>Y axis</span>
              <input
                type="number"
                min={DPI_MIN}
                max={DPI_MAX}
                step={DPI_STEP}
                value={dpiYInput}
                disabled={busy}
                onFocus={() => { dpiYFocused.current = true; }}
                onBlur={() => { dpiYFocused.current = false; }}
                onInput={(event) => setDpiYInput((event.target as HTMLInputElement).value)}
                onKeyDown={(event) => { if (event.key === "Enter") void applyDpiValues(Number(dpiXInput), Number(dpiYInput)); }}
              />
            </label>
          )}
          <button
            class="connect-button dpi-apply-button"
            disabled={busy || !dpiXInput || (showSeparateAxes && !dpiYInput)}
            onClick={() => void applyDpiValues(Number(dpiXInput), showSeparateAxes ? Number(dpiYInput) : undefined)}
          >
            {pending === "dpi" ? "Applying…" : "Apply"}
          </button>
        </div>

        <p class="dpi-current-caption">
          {showSeparateAxes
            ? `Current X ${status.dpi} · Y ${status.dpiY ?? status.dpi} DPI`
            : `Current ${status.dpi} DPI`}
        </p>
      </div>

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
                class={`performance-chip ${hz === status.pollingRateHz ? "active" : ""}`}
                disabled={busy}
                onClick={() => void applyPollingRate(hz)}
              >
                {hz} Hz
              </button>
            ))}
          </div>
        </div>
      )}

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
                    class={mode === status.gamingSurfaceMode ? "active" : ""}
                    disabled={busy}
                    onClick={() => void applyGamingSurfaceMode(mode)}
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
                    class={value === status.liftOffDistance ? "active" : ""}
                    disabled={busy}
                    onClick={() => void applyLiftOffDistance(value)}
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
    </div>
  );
}
