import { useEffect, useState } from "preact/hooks";
import { AlertTriangle } from "lucide-preact";
import type { MouseStatus } from "@openmouse/protocol/drivers/mouse-types";
import type { HidInterfaceInfo } from "../native-hid/tauri-hid-device";
import {
  setDpi as logitechSetDpi,
  setGamingSurfaceMode as logitechSetGamingSurfaceMode,
  setLiftOffDistance as logitechSetLiftOffDistance,
  setPollingRate as logitechSetPollingRate,
} from "../native-hid/logitech-actions";
import {
  setActiveDpiStage,
  setAngleSnapping,
  setDebounceTime,
  setDpi,
  setGamingSurfaceMode,
  setLiftOffDistance,
  setLowPowerThreshold,
  setMotionSync,
  setPerformanceMode,
  setPollingRate,
  setPrimaryButton,
  setRippleControl,
  setSleepTimeout,
  setUsbSpeed,
  type GamingSurfaceMode,
  type LiftOffDistance,
} from "../native-hid/write";
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

// Advanced boolean toggles the generic write layer exposes. Each binds a
// status field to its write action; a control renders only when the device
// reports a value for that field (which in practice means the brand's driver
// reads and writes it). These apply immediately on toggle, not via the apply
// bar — they behave like the switch rows in the webapp's processing card.
const ADVANCED_TOGGLES: readonly {
  key: "motionSync" | "angleSnapping" | "rippleControl" | "performanceMode";
  label: string;
  description: string;
}[] = [
  { key: "motionSync", label: "Motion sync", description: "Synchronizes X/Y sensor sampling for smoother diagonals." },
  { key: "angleSnapping", label: "Angle snapping", description: "Rounds movement to straight lines and diagonals for a steadier cursor." },
  { key: "rippleControl", label: "Ripple control", description: "Filters sensor ripple at high polling rates for more stable tracking." },
  { key: "performanceMode", label: "Performance / hyper mode", description: "Runs the sensor at its full performance setting." },
];

export function DevicePerformanceTab({ info, status, brand, onApplied, lockedBy, readOnly }: Props) {
  const locked = lockedBy !== undefined;
  const [pending, setPending] = useState(false);

  const isLogitech = brand === "Logitech";
  const isRazer = brand === "Razer";

  const [stagedDpi, setStagedDpi] = useState(status.dpi);
  const [stagedPollingRate, setStagedPollingRate] = useState(status.pollingRateHz);
  const [stagedLod, setStagedLod] = useState(status.liftOffDistance);
  const [stagedSurface, setStagedSurface] = useState(status.gamingSurfaceMode);
  const [stagedSleep, setStagedSleep] = useState(status.sleepTimeout ?? null);

  useEffect(() => { setStagedDpi(status.dpi); }, [status.dpi]);
  useEffect(() => { setStagedPollingRate(status.pollingRateHz); }, [status.pollingRateHz]);
  useEffect(() => { setStagedLod(status.liftOffDistance); }, [status.liftOffDistance]);
  useEffect(() => { setStagedSurface(status.gamingSurfaceMode); }, [status.gamingSurfaceMode]);
  useEffect(() => { setStagedSleep(status.sleepTimeout ?? null); }, [status.sleepTimeout]);

  // DPI bounds: a per-brand floor. Razer caps at 8500; Logitech tops out at
  // 32000; other brands borrow Logitech's generous bounds (the device is the
  // source of truth — setDpi() returns the value actually applied).
  const dpiMin = isRazer ? 100 : LOGITECH_DPI_MIN;
  const dpiMax = isRazer ? 8500 : LOGITECH_DPI_MAX;
  const dpiStep = isRazer ? 100 : LOGITECH_DPI_STEP;
  const dpiPresets = isRazer ? [400, 800, 1600, 3200, 6400, 8000] : LOGITECH_DPI_PRESETS;

  const hasSleep = typeof status.sleepTimeout === "number";

  const dirty =
    stagedDpi !== status.dpi ||
    stagedPollingRate !== status.pollingRateHz ||
    stagedLod !== status.liftOffDistance ||
    stagedSurface !== status.gamingSurfaceMode ||
    (hasSleep && stagedSleep !== status.sleepTimeout);

  const busy = pending || locked || readOnly;

  function revert() {
    setStagedDpi(status.dpi);
    setStagedPollingRate(status.pollingRateHz);
    setStagedLod(status.liftOffDistance);
    setStagedSurface(status.gamingSurfaceMode);
    setStagedSleep(status.sleepTimeout ?? null);
  }

  // Route a single write to the right transport: Logitech through its own
  // index-resolving module (logitech-actions.ts), everything else through
  // the generic write layer (native-hid/write.ts).
  async function applyAll() {
    setPending(true);
    try {
      const patch: Partial<MouseStatus> = {};

      if (stagedDpi !== status.dpi) {
        const applied = await (isLogitech
          ? logitechSetDpi(info, Math.round(stagedDpi))
          : setDpi(info, Math.round(stagedDpi)));
        patch.dpi = applied;
      }
      if (stagedPollingRate !== status.pollingRateHz) {
        const applied = await (isLogitech
          ? logitechSetPollingRate(info, stagedPollingRate)
          : setPollingRate(info, stagedPollingRate));
        patch.pollingRateHz = applied;
      }
      if (stagedLod !== status.liftOffDistance && stagedLod != null) {
        const applied = await (isLogitech
          ? logitechSetLiftOffDistance(info, stagedLod)
          : setLiftOffDistance(info, stagedLod as LiftOffDistance));
        patch.liftOffDistance = applied;
      }
      if (stagedSurface !== status.gamingSurfaceMode && stagedSurface != null) {
        const applied = await (isLogitech
          ? logitechSetGamingSurfaceMode(info, stagedSurface)
          : setGamingSurfaceMode(info, stagedSurface as GamingSurfaceMode));
        patch.gamingSurfaceMode = applied;
      }
      if (hasSleep && stagedSleep !== status.sleepTimeout && stagedSleep != null) {
        const applied = await setSleepTimeout(info, stagedSleep);
        patch.sleepTimeout = applied;
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

  async function toggleAdvanced(key: (typeof ADVANCED_TOGGLES)[number]["key"], next: boolean) {
    try {
      const applied = await (key === "motionSync"
        ? setMotionSync(info, next)
        : key === "angleSnapping"
          ? setAngleSnapping(info, next)
          : key === "rippleControl"
            ? setRippleControl(info, next)
            : setPerformanceMode(info, next));
      onApplied({ [key]: Boolean(applied) } as Partial<MouseStatus>);
      showToast(`${key === "motionSync" ? "Motion sync" : key === "angleSnapping" ? "Angle snapping" : key === "rippleControl" ? "Ripple control" : "Performance mode"} ${next ? "enabled" : "disabled"}.`, "success");
    } catch (err) {
      showToast(err instanceof Error ? err.message : String(err), "error");
    }
  }

  // Immediate (non-apply-bar) writes for the single-value controls — DPI
  // stage, debounce, low-power threshold, USB speed, primary button. These
  // apply on change like the advanced toggles, not via the apply bar.
  async function runImmediate(
    label: string,
    promise: Promise<unknown>,
    patch: (value: unknown) => Partial<MouseStatus> | null,
  ) {
    try {
      const applied = await promise;
      const next = patch(applied);
      if (next) onApplied(next);
      showToast(`${label} applied.`, "success");
    } catch (err) {
      showToast(err instanceof Error ? err.message : String(err), "error");
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
          Connected — live controls are available for this device.
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

      {/* ── Sleep ────────────────────────────────────────────────── */}
      {hasSleep && (
        <div class="setting-row">
          <div class="setting-label">
            <span class="setting-title">Sleep timeout</span>
            <span class="setting-description">How long the mouse stays awake when idle before sleeping.</span>
          </div>
          <div class="performance-chip-group">
            {[600, 3600, 7200, 14400, 28800].map((seconds) => (
              <button
                key={seconds}
                class={`performance-chip ${seconds === stagedSleep ? "active" : ""}`}
                disabled={busy}
                onClick={() => setStagedSleep(seconds)}
              >
                {seconds >= 3600 ? `${Math.round(seconds / 3600)} hr` : `${Math.round(seconds / 60)} min`}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* ── Advanced toggles ─────────────────────────────────────── */}
      {ADVANCED_TOGGLES.map(({ key, label, description }) =>
        typeof status[key] === "boolean" ? (
          <div class="setting-row" key={key}>
            <div class="setting-label">
              <span class="setting-title">{label}</span>
              <span class="setting-description">{description}</span>
            </div>
            <label class="switch">
              <input
                type="checkbox"
                checked={Boolean(status[key])}
                disabled={busy}
                onChange={() => void toggleAdvanced(key, !Boolean(status[key]))}
              />
              <span class="switch-track" />
            </label>
          </div>
        ) : null,
      )}

      {/* ── DPI stages (Teevolution, Ninjutso, VGN, …) ───────────── */}
      {status.dpiStages && status.dpiStages.length > 1 && (
        <div class="setting-row">
          <div class="setting-label">
            <span class="setting-title">DPI stages</span>
            <span class="setting-description">On-device sensitivity stages; pick the active one.</span>
          </div>
          <div class="performance-chip-group">
            {status.dpiStages.map((_, index) => (
              <button
                key={index}
                class={`performance-chip ${index === status.activeDpiStage ? "active" : ""}`}
                disabled={busy}
                onClick={() => void runImmediate("Active DPI stage", setActiveDpiStage(info, index), (v) =>
                  typeof v === "number" ? { activeDpiStage: v } : null)}
              >
                Stage {index + 1}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* ── Debounce (ATK, Pulsar, WLMouse, Wallhack, …) ─────────── */}
      {typeof status.debounceMs === "number" && (
        <div class="setting-row">
          <div class="setting-label">
            <span class="setting-title">Debounce</span>
            <span class="setting-description">Filters out physical switch bounce (0–20 ms). Higher is steadier, slightly more click lag.</span>
          </div>
          <select
            class="performance-select"
            value={String(status.debounceMs)}
            disabled={busy}
            onChange={(e) => {
              const ms = Number((e.target as HTMLSelectElement).value);
              void runImmediate("Debounce", setDebounceTime(info, ms), (v) =>
                typeof v === "number" ? { debounceMs: v } : null);
            }}
          >
            {Array.from({ length: 21 }, (_, ms) => <option key={ms} value={ms}>{ms} ms</option>)}
          </select>
        </div>
      )}

      {/* ── Low-power threshold (Razer) ──────────────────────────── */}
      {typeof status.lowBatteryWarning === "number" && (
        <div class="setting-row">
          <div class="setting-label">
            <span class="setting-title">Low power mode</span>
            <span class="setting-description">Slows the mouse down to save battery below this level.</span>
          </div>
          <select
            class="performance-select"
            value={String(status.lowBatteryWarning)}
            disabled={busy}
            onChange={(e) => {
              const pct = Number((e.target as HTMLSelectElement).value);
              void runImmediate("Low power threshold", setLowPowerThreshold(info, pct), (v) =>
                typeof v === "number" ? { lowBatteryWarning: v } : null);
            }}
          >
            {[5, 10, 15, 20, 25, 30].map((pct) => <option key={pct} value={pct}>{pct}%</option>)}
          </select>
        </div>
      )}

      {/* ── USB speed + primary button (Zaunkoenig) ──────────────── */}
      {status.usbSpeed != null && (
        <div class="setting-row">
          <div class="setting-label">
            <span class="setting-title">USB speed</span>
            <span class="setting-description">Full pins polling to 1000 Hz; High enables faster rates.</span>
          </div>
          <div class="performance-chip-group">
            {(["Full", "High"] as const).map((speed) => (
              <button
                key={speed}
                class={`performance-chip ${speed === status.usbSpeed ? "active" : ""}`}
                disabled={busy}
                onClick={() => void runImmediate("USB speed", setUsbSpeed(info, speed), (v) =>
                  v === "Full" || v === "High" ? { usbSpeed: v } : null)}
              >
                {speed}
              </button>
            ))}
          </div>
        </div>
      )}

      {status.primaryButton != null && (
        <div class="setting-row">
          <div class="setting-label">
            <span class="setting-title">Primary button</span>
            <span class="setting-description">Which button acts as the primary click.</span>
          </div>
          <div class="performance-chip-group">
            {(["Left", "Right"] as const).map((button) => (
              <button
                key={button}
                class={`performance-chip ${button === status.primaryButton ? "active" : ""}`}
                disabled={busy}
                onClick={() => void runImmediate("Primary button", setPrimaryButton(info, button), (v) =>
                  v === "Left" || v === "Right" ? { primaryButton: v } : null)}
              >
                {button}
              </button>
            ))}
          </div>
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