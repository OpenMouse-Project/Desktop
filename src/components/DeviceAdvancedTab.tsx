import type { MouseStatus } from "@openmouse/protocol/drivers/mouse-types";
import type { HidInterfaceInfo } from "../native-hid/tauri-hid-device";
import {
  setCpiLevels,
  setCustomPollingDivider,
  setDongleLedMode,
  setDpiLighting,
  setMotionJitterFilter,
  setNinjutsoHyperClick,
  setNinjutsoOpticalEngine,
  setNinjutsoSlamClick,
  setNinjutsoSystemMode,
  setPerformanceDuration,
  setProfile,
  setSlamclickFilter,
  setSpdtMode,
  setTournamentScrollMode,
  setTournamentScrollTimeout,
  setWheelAcceleration,
} from "../native-hid/write";
import { showToast } from "../lib/toast";

interface Props {
  info: HidInterfaceInfo;
  status: MouseStatus;
  onApplied: (patch: Partial<MouseStatus>) => void;
  readOnly?: boolean;
}

async function writeToast(
  label: string,
  promise: Promise<unknown>,
  onApplied?: (patch: Partial<MouseStatus>) => void,
  patch?: (value: unknown) => Partial<MouseStatus>,
) {
  try {
    const applied = await promise;
    if (onApplied && patch) onApplied(patch(applied));
    showToast(`${label} applied.`, "success");
  } catch (err) {
    showToast(err instanceof Error ? err.message : String(err), "error");
  }
}

function Segmented<T extends string>({
  value,
  options,
  disabled,
  onChange,
  label,
}: {
  value: T | null | undefined;
  options: readonly T[];
  disabled: boolean;
  onChange: (next: T) => void;
  label: string;
}) {
  return (
    <div class="segmented-group" role="radiogroup" aria-label={label}>
      {options.map((option) => (
        <button
          key={option}
          class={option === value ? "active" : ""}
          disabled={disabled}
          onClick={() => onChange(option)}
        >
          {option}
        </button>
      ))}
    </div>
  );
}

function SwitchRow({
  enabled,
  disabled,
  onChange,
  label,
}: {
  enabled: boolean;
  disabled: boolean;
  onChange: (next: boolean) => void;
  label: string;
}) {
  return (
    <div class="setting-row">
      <div class="setting-label">
        <span class="setting-title">{label}</span>
      </div>
      <label class="switch">
        <input type="checkbox" checked={enabled} disabled={disabled} onChange={() => onChange(!enabled)} />
        <span class="switch-track" />
      </label>
    </div>
  );
}

/**
 * Brand-gated feature surface — the parts of the webapp's advanced cards that
 * only exist on a specific brand's driver. Every section renders only when the
 * connected device's `MouseStatus` reports the field it controls (mirrors the
 * webapp's `cardAvailability`): an Endgame Gear mouse shows Filters/GX
 * switch/CPI, a Ninjutso shows Sensor performance/Click behavior, and so on.
 */
export function DeviceAdvancedTab({ info, status, onApplied, readOnly }: Props) {
  const disabled = readOnly ?? false;

  // ── Endgame Gear ──────────────────────────────────────────────────────
  const isEgg = typeof status.slamclickFilter === "boolean" || typeof status.motionJitterFilter === "boolean";
  const eggSections = isEgg || (Array.isArray(status.eggCpiStages) && status.eggCpiStages.length > 0);

  // ── Ninjutso ──────────────────────────────────────────────────────────
  const ninjutsoSensor = Boolean(status.ninjutsoSystemMode) || Boolean(status.ninjutsoOpticalEngine);
  const ninjutsoClick = status.ninjutsoHyperClick != null || Boolean(status.ninjutsoSlamClick);

  // ── Finalmouse ────────────────────────────────────────────────────────
  const finalmouse = status.finalmouseDongleLedMode != null;

  // ── Teevolution ───────────────────────────────────────────────────────
  const teevolution = status.sensorMode != null || status.performanceDuration != null || status.dpiLedMode != null;

  // ── Pulsar Pro ────────────────────────────────────────────────────────
  const pulsarPro = status.wheelAcceleration != null || typeof status.activeProfile === "number";

  if (!eggSections && !ninjutsoSensor && !ninjutsoClick && !finalmouse && !teevolution && !pulsarPro) {
    return (
      <div class="tab-placeholder">
        <p>No brand-specific settings are exposed by this device's driver.</p>
      </div>
    );
  }

  return (
    <div class="performance-tab">
      {readOnly && (
        <p class="performance-locked-note">Connected — live controls are available.</p>
      )}

      {/* ── Endgame Gear ──────────────────────────────────────────── */}
      {isEgg && (
        <div class="setting-row-block">
          <span class="setting-eyebrow">Endgame Gear</span>

          {typeof status.slamclickFilter === "boolean" && (
            <SwitchRow
              label="Slamclick filter"
              enabled={status.slamclickFilter}
              disabled={disabled}
              onChange={(next) => void writeToast("Slamclick filter", setSlamclickFilter(info, next), onApplied, () => ({
                slamclickFilter: next,
              }))}
            />
          )}
          {typeof status.motionJitterFilter === "boolean" && (
            <SwitchRow
              label="Motion-jitter filter"
              enabled={status.motionJitterFilter}
              disabled={disabled}
              onChange={(next) => void writeToast("Motion-jitter filter", setMotionJitterFilter(info, next), onApplied, () => ({
                motionJitterFilter: next,
              }))}
            />
          )}

          {(status.leftSpdtMode != null || status.rightSpdtMode != null) && (
            <div class="setting-row">
              <div class="setting-label">
                <span class="setting-title">GX switch mode</span>
              </div>
              <div class="setting-row-block">
                {(["Off", "GX Safe", "GX Speed"] as const).length > 0 && (
                  <Segmented
                    label="Left button"
                    value={status.leftSpdtMode ?? "Off"}
                    options={["Off", "GX Safe", "GX Speed"] as const}
                    disabled={disabled}
                    onChange={(mode) =>
                      void writeToast("GX switch (left)", setSpdtMode(info, "left", mode), onApplied, () => ({
                        leftSpdtMode: mode,
                      }))}
                  />
                )}
                {status.rightSpdtMode != null && (
                  <Segmented
                    label="Right button"
                    value={status.rightSpdtMode}
                    options={["Off", "GX Safe", "GX Speed"] as const}
                    disabled={disabled}
                    onChange={(mode) =>
                      void writeToast("GX switch (right)", setSpdtMode(info, "right", mode), onApplied, () => ({
                        rightSpdtMode: mode,
                      }))}
                  />
                )}
              </div>
            </div>
          )}

          {Array.isArray(status.eggCpiStages) && status.eggCpiStages.length > 0 && (
            <div class="setting-row">
              <div class="setting-label">
                <span class="setting-title">CPI levels ({status.eggCpiLevels ?? 1})</span>
              </div>
              <div class="segmented-group">
                {[1, 2, 3, 4].map((level) => (
                  <button
                    key={level}
                    class={level === status.eggCpiLevels ? "active" : ""}
                    disabled={disabled}
                    onClick={() =>
                      void writeToast("CPI levels", setCpiLevels(info, level), onApplied, () => ({ eggCpiLevels: level }))}
                  >
                    {level}
                  </button>
                ))}
              </div>
            </div>
          )}

          {status.eggPollingDivider != null && (
            <div class="setting-row">
              <div class="setting-label">
                <span class="setting-title">8K polling divider</span>
                <span class="setting-description">
                  Result: {Number.isInteger(status.eggPollingDivider) && status.eggPollingDivider > 0
                    ? `${(8000 / status.eggPollingDivider).toLocaleString(undefined, { maximumFractionDigits: 2 })} Hz`
                    : "—"}
                </span>
              </div>
              <input
                class="performance-select"
                type="number"
                min={1}
                max={255}
                value={String(status.eggPollingDivider)}
                disabled={disabled}
                onBlur={(e) => {
                  const v = Number((e.target as HTMLInputElement).value);
                  if (Number.isInteger(v) && v > 0 && v <= 255) {
                    void writeToast("Polling divider", setCustomPollingDivider(info, v), onApplied, () => ({ eggPollingDivider: v }));
                  }
                }}
              />
            </div>
          )}
        </div>
      )}

      {/* ── Ninjutso ──────────────────────────────────────────────── */}
      {(ninjutsoSensor || ninjutsoClick) && (
        <div class="setting-row-block">
          <span class="setting-eyebrow">Ninjutso</span>

          {status.ninjutsoSystemMode && (
            <div class="setting-row">
              <div class="setting-label">
                <span class="setting-title">System mode</span>
              </div>
              <Segmented
                label="System mode"
                value={status.ninjutsoSystemMode}
                options={(status.ninjutsoSystemModes ?? ["High Speed", "Competitive", "Ultra"]) as typeof status.ninjutsoSystemMode[]}
                disabled={disabled}
                onChange={(mode) =>
                  void writeToast("System mode", setNinjutsoSystemMode(info, mode), onApplied, () => ({ ninjutsoSystemMode: mode }))}
              />
            </div>
          )}
          {status.ninjutsoOpticalEngine && (
            <div class="setting-row">
              <div class="setting-label">
                <span class="setting-title">Optical Engine</span>
              </div>
              <Segmented
                label="Optical Engine"
                value={status.ninjutsoOpticalEngine}
                options={["Standard", "Burst"] as const}
                disabled={disabled}
                onChange={(mode) =>
                  void writeToast("Optical Engine", setNinjutsoOpticalEngine(info, mode), onApplied, () => ({ ninjutsoOpticalEngine: mode }))}
              />
            </div>
          )}
          {status.ninjutsoHyperClick != null && (
            <SwitchRow
              label="HyperClick"
              enabled={Boolean(status.ninjutsoHyperClick)}
              disabled={disabled}
              onChange={(next) => void writeToast("HyperClick", setNinjutsoHyperClick(info, next), onApplied, () => ({ ninjutsoHyperClick: next }))}
            />
          )}
          {status.ninjutsoSlamClick && (
            <div class="setting-row">
              <div class="setting-label">
                <span class="setting-title">Slam-Click</span>
              </div>
              <Segmented
                label="Slam-Click"
                value={status.ninjutsoSlamClick}
                options={["Low", "Medium", "High"] as const}
                disabled={disabled}
                onChange={(level) =>
                  void writeToast("Slam-Click", setNinjutsoSlamClick(info, level), onApplied, () => ({ ninjutsoSlamClick: level }))}
              />
            </div>
          )}
        </div>
      )}

      {/* ── Finalmouse ────────────────────────────────────────────── */}
      {finalmouse && (
        <div class="setting-row-block">
          <span class="setting-eyebrow">Finalmouse</span>
          <div class="setting-row">
            <div class="setting-label">
              <span class="setting-title">Dongle LED</span>
            </div>
            <select
              class="performance-select"
              value={String(status.finalmouseDongleLedMode ?? 0)}
              disabled={disabled}
              onChange={(e) => {
                const v = Number((e.target as HTMLSelectElement).value);
                void writeToast("Dongle LED", setDongleLedMode(info, v), onApplied, () => ({ finalmouseDongleLedMode: v }));
              }}
            >
              <option value={0}>Off</option>
              <option value={1}>Battery indicator</option>
              <option value={2}>Solid white</option>
            </select>
          </div>
          <div class="setting-row">
            <div class="setting-label">
              <span class="setting-title">Tournament scroll</span>
            </div>
            <select
              class="performance-select"
              value={String(status.finalmouseTournamentScrollMode ?? 0)}
              disabled={disabled}
              onChange={(e) => {
                const v = Number((e.target as HTMLSelectElement).value);
                void writeToast("Tournament scroll", setTournamentScrollMode(info, v), onApplied, () => ({ finalmouseTournamentScrollMode: v }));
              }}
            >
              <option value={0}>Off</option>
              <option value={1}>Scroll up</option>
              <option value={2}>Scroll down</option>
              <option value={3}>Both directions</option>
            </select>
          </div>
          <div class="setting-row">
            <div class="setting-label">
              <span class="setting-title">Passthrough window</span>
            </div>
            <select
              class="performance-select"
              value={String(status.finalmouseTournamentScrollTimeoutMs ?? 100)}
              disabled={disabled}
              onChange={(e) => {
                const v = Number((e.target as HTMLSelectElement).value);
                void writeToast("Passthrough window", setTournamentScrollTimeout(info, v), onApplied, () => ({ finalmouseTournamentScrollTimeoutMs: v }));
              }}
            >
              <option value={100}>100 ms</option>
              <option value={500}>500 ms</option>
              <option value={1000}>1 second</option>
              <option value={1500}>1.5 seconds</option>
            </select>
          </div>
        </div>
      )}

      {/* ── Teevolution / DPI lighting ────────────────────────────── */}
      {teevolution && (
        <div class="setting-row-block">
          <span class="setting-eyebrow">Teevolution</span>
          {status.dpiLedMode != null && (
            <div class="setting-row">
              <div class="setting-label">
                <span class="setting-title">DPI indicator effect</span>
              </div>
              <select
                class="performance-select"
                value={String(status.dpiLedMode)}
                disabled={disabled}
                onChange={(e) => {
                  const mode = Number((e.target as HTMLSelectElement).value);
                  void writeToast(
                    "DPI indicator",
                    setDpiLighting(info, mode, status.dpiLedBrightness ?? 0, status.dpiLedSpeed ?? 0),
                    onApplied,
                    () => ({ dpiLedMode: mode }),
                  );
                }}
              >
                <option value={0}>Off</option>
                <option value={1}>Steady</option>
                <option value={2}>Breathing</option>
              </select>
            </div>
          )}
          {status.performanceDuration != null && (
            <div class="setting-row">
              <div class="setting-label">
                <span class="setting-title">Performance duration</span>
              </div>
              <select
                class="performance-select"
                value={String(status.performanceDuration)}
                disabled={disabled}
                onChange={(e) => {
                  const v = Number((e.target as HTMLSelectElement).value);
                  void writeToast("Performance duration",
                    setPerformanceDuration(info, v),
                    onApplied,
                    () => ({ performanceDuration: v }));
                }}
              >
                {[10, 30, 60, 180, 300, 600].map((v) => (
                  <option key={v} value={v}>{v > 60 ? `${Math.round(v / 60)} min` : `${v} min`}</option>
                ))}
              </select>
            </div>
          )}
        </div>
      )}

      {/* ── Pulsar Pro ────────────────────────────────────────────── */}
      {pulsarPro && (
        <div class="setting-row-block">
          <span class="setting-eyebrow">Pulsar Pro</span>
          {status.wheelAcceleration != null && (
            <SwitchRow
              label="Wheel acceleration"
              enabled={Boolean(status.wheelAcceleration)}
              disabled={disabled}
              onChange={(next) => void writeToast("Wheel acceleration", setWheelAcceleration(info, next), onApplied, () => ({ wheelAcceleration: next }))}
            />
          )}
          {typeof status.activeProfile === "number" && status.activeProfile > 0 && (
            <div class="setting-row">
              <div class="setting-label">
                <span class="setting-title">Active profile</span>
              </div>
              <select
                class="performance-select"
                value={String(status.activeProfile)}
                disabled={disabled}
                onChange={(e) => {
                  const v = Number((e.target as HTMLSelectElement).value);
                  void writeToast("Profile", setProfile(info, v), onApplied, () => ({ activeProfile: v }));
                }}
              >
                {[1, 2, 3, 4, 5].map((v) => <option key={v} value={v}>{v}</option>)}
              </select>
            </div>
          )}
        </div>
      )}
    </div>
  );
}