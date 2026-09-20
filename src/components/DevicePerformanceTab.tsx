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
  deviceCapabilities,
  dpiBounds,
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
  /** The capability lists only the driver class knows come from the registry now — see write.ts. */
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

  // Valid values for the controls whose options are set by the device's own
  // protocol, asked of the driver class that answered it (see write.ts) —
  // hardcoding them here is what made the sleep and debounce controls reject
  // almost every value on every brand that didn't match the guess.
  const capabilities = deviceCapabilities(info);

  const isLogitech = brand === "Logitech";
  const isRazer = brand === "Razer";
  const isAttackShark = brand === "Attack Shark";

  const [stagedDpi, setStagedDpi] = useState(status.dpi);
  const [stagedDpiY, setStagedDpiY] = useState(status.dpiY ?? status.dpi);
  /**
   * Mice with separate X/Y axes (the Razer family reports
   * `supportsSeparateDpiAxes`) get one DPI control that moves both axes
   * together, with an explicit unlock for anyone who wants them apart.
   *
   * Linked is the default because the alternative is silent: this panel shows
   * a single number, so writing only the staged X while the mouse keeps its
   * own Y puts the two axes on different sensitivities with nothing on screen
   * saying so — the mouse then tracks X and Y (and therefore diagonals) at
   * different speeds. Omitting Y entirely is what "linked" means for every
   * driver: their own `setDpi(dpi, dpiY = dpi)` defaults it to X.
   */
  const [unlinkedAxes, setUnlinkedAxes] = useState(false);
  const hasSeparateAxes = status.supportsSeparateDpiAxes === true;
  const [stagedPollingRate, setStagedPollingRate] = useState(status.pollingRateHz);
  const [stagedLod, setStagedLod] = useState(status.liftOffDistance);
  const [stagedSurface, setStagedSurface] = useState(status.gamingSurfaceMode);
  const [stagedSleep, setStagedSleep] = useState(status.sleepTimeout ?? null);

  useEffect(() => { setStagedDpi(status.dpi); }, [status.dpi]);
  useEffect(() => { setStagedDpiY(status.dpiY ?? status.dpi); }, [status.dpiY, status.dpi]);
  useEffect(() => { setStagedPollingRate(status.pollingRateHz); }, [status.pollingRateHz]);
  useEffect(() => { setStagedLod(status.liftOffDistance); }, [status.liftOffDistance]);
  useEffect(() => { setStagedSurface(status.gamingSurfaceMode); }, [status.gamingSurfaceMode]);
  useEffect(() => { setStagedSleep(status.sleepTimeout ?? null); }, [status.sleepTimeout]);

  // Bounds for the DPI field: the device's own protocol-provided limits
  // (MouseUiHints.dpiStageEditor, then its driver's ceiling) ahead of these
  // branded fallbacks — see dpiBounds().
  const { min: dpiMin, max: dpiMax, step: dpiStep } = dpiBounds(status, capabilities, {
    min: isAttackShark ? 50 : isRazer ? 100 : LOGITECH_DPI_MIN,
    max: isAttackShark ? 22000 : isRazer ? 8500 : LOGITECH_DPI_MAX,
    step: isAttackShark ? 50 : isRazer ? 100 : LOGITECH_DPI_STEP,
  });
  const dpiPresets = (isAttackShark
    ? [400, 800, 1600, 3200, 6400, 12800, 22000]
    : isRazer
      ? [400, 800, 1600, 3200, 6400, 8000]
      : LOGITECH_DPI_PRESETS).filter((preset) => preset >= dpiMin && preset <= dpiMax);

  // The device's own UI policy, straight from the protocol (MouseUiHints).
  // It exists so a driver can say which controls are meaningful for its
  // protocol generation, and ignoring it is why the app offered switches and
  // cards that do nothing on some devices.
  const ui = status.ui;

  // `hideLodLow` — no low setting in this firmware; `lodRequiresSurface` —
  // lift-off only applies while a gaming surface mode is selected.
  const loftOffValues = (status.supportedLiftOffDistances ?? [])
    .filter((value) => !(ui?.hideLodLow && value === "Low"));
  const loftOffDisabled = ui?.lodRequiresSurface === true && status.gamingSurfaceMode === "Off";

  const visibleToggles = ADVANCED_TOGGLES.filter(({ key }) => {
    if (ui?.hideProcessingCard) return false;
    if (key === "motionSync") return !ui?.hideMotionSync;
    if (key === "angleSnapping") return !ui?.hideAngleSnapping;
    if (key === "rippleControl") return !ui?.hideRippleControl;
    return true;
  });

  // Debounce: the driver's own list when it publishes one, otherwise every
  // value up to the ceiling it reports. Null when this protocol generation has
  // no such control at all (Ninjutso reports a ceiling of 0), so the row is
  // hidden rather than offering values the mouse rejects.
  const debounceValues = typeof status.debounceMs === "number"
    ? capabilities.debounceOptions
      ?? (capabilities.debounceMaxMs !== null
        ? Array.from({ length: capabilities.debounceMaxMs + 1 }, (_, ms) => ms)
        : null)
    : null;

  const hasSleep = typeof status.sleepTimeout === "number"
    && !ui?.hideSleepCard
    && capabilities.sleepTimeouts !== null;

  const dirty =
    stagedDpi !== status.dpi ||
    (hasSeparateAxes && unlinkedAxes && stagedDpiY !== status.dpiY) ||
    stagedPollingRate !== status.pollingRateHz ||
    stagedLod !== status.liftOffDistance ||
    stagedSurface !== status.gamingSurfaceMode ||
    (hasSleep && stagedSleep !== status.sleepTimeout);

  const busy = pending || locked || readOnly;

  function revert() {
    setStagedDpi(status.dpi);
    setStagedDpiY(status.dpiY ?? status.dpi);
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

      if (stagedDpi !== status.dpi || (hasSeparateAxes && unlinkedAxes && stagedDpiY !== status.dpiY)) {
        // Linked (the default): Y is omitted so every driver's own
        // `setDpi(dpi, dpiY = dpi)` default keeps both axes on this value —
        // never the mouse's own Y, which would leave the axes unequal.
        const dpiY = hasSeparateAxes && unlinkedAxes ? Math.round(stagedDpiY) : undefined;
        const applied = await (isLogitech
          ? logitechSetDpi(info, Math.round(stagedDpi), dpiY)
          : setDpi(info, Math.round(stagedDpi), dpiY));
        patch.dpi = applied;
        // The drivers verify both axes (and throw if the mouse kept
        // something else), so an unlinked Y is confirmed too.
        if (hasSeparateAxes) patch.dpiY = dpiY ?? applied;
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
    // `pending` gates every control in this tab (`busy`), so a toggle has to
    // set it too: without it a second click lands while the first write is
    // still on the wire and the generic write layer rejects it as "already
    // busy with another request" — an error toast for a click the UI
    // presented as available.
    setPending(true);
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
    } finally {
      setPending(false);
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
    // See toggleAdvanced: every one of these controls starts its write
    // eagerly from an onClick/onChange, so without `pending` the second click
    // of a rapid pair races the first and is rejected as "busy".
    setPending(true);
    try {
      const applied = await promise;
      const next = patch(applied);
      if (next) onApplied(next);
      showToast(`${label} applied.`, "success");
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
          Connected — live controls are available for this device.
        </p>
      )}

      {/* ── DPI ──────────────────────────────────────────────────── */}
      {/* A device whose driver cannot report a DPI value (e.g. the Attack
          Shark X11's native channel, whose DPI command is write-only for
          now) must not render a control whose Apply would fail with
          "setDpi not supported". Hide the panel until it reports a real
          DPI. */}
      {status.dpi > 0 && (
      <div class="dpi-panel">
        <div class="dpi-panel-header">
          <div class="setting-label">
            <span class="setting-eyebrow">DPI</span>
            <span class="setting-title">Sensitivity</span>
          </div>
          <div class="dpi-panel-value-group">
            <span class={`dpi-panel-value ${stagedDpi !== status.dpi || (hasSeparateAxes && unlinkedAxes && stagedDpiY !== status.dpiY) ? "dpi-panel-value--dirty" : ""}`}>
              {hasSeparateAxes && unlinkedAxes
                ? `${stagedDpi.toLocaleString()} × ${stagedDpiY.toLocaleString()} DPI`
                : `${stagedDpi.toLocaleString()} DPI`}
            </span>
            {!dpiPresets.includes(stagedDpi) && <span class="dpi-panel-badge">Custom</span>}
            {hasSeparateAxes && (
              <label class="dpi-panel-axis-link" title="Link X and Y sensitivity">
                <span>Link X/Y</span>
                <input
                  type="checkbox"
                  checked={!unlinkedAxes}
                  disabled={busy}
                  onChange={() => {
                    const linked = unlinkedAxes;
                    setUnlinkedAxes(!linked);
                    // Re-linking snaps Y back onto X, so what Apply writes is
                    // what the panel shows.
                    if (linked) setStagedDpiY(stagedDpi);
                  }}
                />
              </label>
            )}
          </div>
        </div>

        <div class="dpi-preset-grid">
          {dpiPresets.map((preset) => (
            <button
              key={preset}
              class={`dpi-preset ${preset === stagedDpi ? "active" : ""}`}
              disabled={busy}
              onClick={() => {
                setStagedDpi(preset);
                if (!unlinkedAxes) setStagedDpiY(preset);
              }}
            >
              {preset.toLocaleString()}
            </button>
          ))}
        </div>

        <div class="dpi-axis-row">
          <label class="dpi-axis-field">
            <span>{hasSeparateAxes && unlinkedAxes ? "X axis" : "Custom DPI"} ({dpiMin}–{dpiMax.toLocaleString()})</span>
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
                if (Number.isFinite(v) && v > 0) {
                  setStagedDpi(v);
                  if (!unlinkedAxes) setStagedDpiY(v);
                }
              }}
              onKeyDown={(e) => { if (e.key === "Enter") void applyAll(); }}
            />
          </label>
          {hasSeparateAxes && unlinkedAxes && (
            <label class="dpi-axis-field">
              <span>Y axis ({dpiMin}–{dpiMax.toLocaleString()})</span>
              <input
                type="number"
                min={dpiMin}
                max={dpiMax}
                step={dpiStep}
                placeholder={`e.g. ${dpiPresets[2]}`}
                value={String(stagedDpiY)}
                disabled={busy}
                onInput={(e) => {
                  const v = Number((e.target as HTMLInputElement).value);
                  if (Number.isFinite(v) && v > 0) setStagedDpiY(v);
                }}
                onKeyDown={(e) => { if (e.key === "Enter") void applyAll(); }}
              />
            </label>
          )}
        </div>

        <p class="dpi-current-caption">
          Current {status.dpi} DPI
          {hasSeparateAxes && status.dpiY != null && status.dpiY !== status.dpi ? ` (Y ${status.dpiY})` : ""}
        </p>
      </div>
      )}

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
                // `pollingReadOnly` is the protocol saying the rate can be read
                // but not staged (a limited firmware generation) — the chip
                // still shows the device's rate, it just can't be picked.
                disabled={busy || ui?.pollingReadOnly}
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

          {loftOffValues.length > 0 && (
            <>
              <span class="sensor-panel-title">Lift-off distance</span>
              <div class="segmented-group">
                {loftOffValues.map((value) => (
                  <button
                    key={value}
                    class={value === stagedLod ? "active" : ""}
                    disabled={busy || loftOffDisabled}
                    onClick={() => setStagedLod(value)}
                  >
                    {value}
                  </button>
                ))}
              </div>
              <p class="sensor-panel-description">
                {loftOffDisabled
                  ? "Available with a gaming surface mode selected — lift-off distance is ignored while surface tracking is off."
                  : "Controls how far you can lift the mouse before tracking stops. Higher values keep tracking a little longer."}
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
          <select
            class="performance-select"
            value={String(stagedSleep)}
            disabled={busy}
            onChange={(e) => setStagedSleep(Number((e.target as HTMLSelectElement).value))}
          >
            {(() => {
              // Straight from the driver. Razer's list is fifteen one-minute
              // steps, which as chips wrapped into five rows of buttons; a
              // select is the same convention this panel already uses for the
              // low-power list. The staged value is appended when the list
              // doesn't contain it, so a mouse sitting on something else still
              // shows the truth rather than nothing selected.
              const steps = capabilities.sleepTimeouts ?? [];
              const current = stagedSleep;
              const options = current != null && !steps.includes(current)
                ? [...steps, current].sort((a, b) => a - b)
                : steps;
              return options.map((seconds) => (
                <option key={seconds} value={seconds}>
                  {/* 0 is the "never sleep" value GearHub/MCHOSE use; the rest
                      are seconds (see deviceCapabilities). */}
                  {seconds === 0 ? "Never" : seconds >= 3600 ? `${Math.round(seconds / 3600)} hr` : `${Math.round(seconds / 60)} min`}
                </option>
              ));
            })()}
          </select>
        </div>
      )}

      {/* ── Advanced toggles ─────────────────────────────────────── */}
      {visibleToggles.map(({ key, label, description }) =>
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
      {debounceValues !== null && (
        <div class="setting-row">
          <div class="setting-label">
            <span class="setting-title">Debounce</span>
            <span class="setting-description">Filters out physical switch bounce (0–{debounceValues[debounceValues.length - 1]} ms). Higher is steadier, slightly more click lag.</span>
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
            {debounceValues?.map((ms) => <option key={ms} value={ms}>{ms} ms</option>)}
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
            {(() => {
              // Straight from the driver when it says (Razer's own slider
              // steps); the device's reported value is appended either way so
              // a threshold outside the list — the Viper V4 Pro reports any
              // integer 0-100 — still shows the truth instead of rendering
              // the select with nothing selected.
              const current = status.lowBatteryWarning;
              const steps = capabilities.lowPowerThresholds ?? [5, 10, 15, 20, 25, 30];
              const options = current != null && !steps.includes(current)
                ? [...steps, current].sort((a, b) => a - b)
                : steps;
              return options.map((pct) => <option key={pct} value={pct}>{pct}%</option>);
            })()}
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
            <button class="apply-bar-revert" disabled={busy} onClick={revert}>Revert</button>
            <button class="apply-bar-apply" disabled={busy} onClick={() => void applyAll()}>
              {pending ? "Applying…" : "Apply Changes"}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}