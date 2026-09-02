import type { MouseStatus } from "@openmouse/protocol/drivers/mouse-types";
import {
  RAZER_BUTTON_CONTROLS,
  RAZER_BUTTON_MAPPINGS,
  RAZER_LOCKED_BUTTON_CONTROL,
  RAZER_TOGGLE_CONTROLS,
  RAZER_TOGGLE_CONTROL_INFO,
  type RazerButtonControl,
} from "@openmouse/protocol/razer";
import { EGG_BUTTON_NAMES, EGG_BUTTON_MAPPINGS } from "@openmouse/protocol/drivers/endgame/egg-op1-hid";
import type { HidInterfaceInfo } from "../native-hid/tauri-hid-device";
import { setButtonMapping, setMulticlickFilter } from "../native-hid/write";
import { showToast } from "../lib/toast";

interface Props {
  info: HidInterfaceInfo;
  status: MouseStatus;
  brand: string;
  onApplied: (patch: Partial<MouseStatus>) => void;
  readOnly?: boolean;
}

async function writeToast(
  label: string,
  promise: Promise<unknown>,
  onApplied: (patch: Partial<MouseStatus>) => void,
  patch: (value: unknown) => Partial<MouseStatus>,
) {
  try {
    const applied = await promise;
    onApplied(patch(applied));
    showToast(`${label} applied.`, "success");
  } catch (err) {
    showToast(err instanceof Error ? err.message : String(err), "error");
  }
}

const RAZER_BUTTON_LABEL: Record<RazerButtonControl, string> = {
  leftClick: "Left Click",
  rightClick: "Right Click",
  mouse4: "Mouse Button 4",
  mouse5: "Mouse Button 5",
};

export function DeviceButtonsTab({ info, status, brand, onApplied, readOnly }: Props) {
  const disabled = readOnly ?? false;
  const isRazer = brand === "Razer" && Boolean(status.razerButtonMappings);
  const isEgg = brand === "Endgame Gear" && Boolean(status.eggButtonMappings);

  if (!isRazer && !isEgg) {
    return (
      <div class="tab-placeholder">
        <p>No reconfigurable buttons are exposed by this device's driver.</p>
      </div>
    );
  }

  return (
    <div class="performance-tab">
      {/* ── Razer ──────────────────────────────────────────────── */}
      {isRazer && status.razerButtonMappings && (
        <div class="setting-row-block">
          <span class="setting-eyebrow">Razer — Buttons</span>
          {RAZER_BUTTON_CONTROLS.filter((control) => control !== RAZER_LOCKED_BUTTON_CONTROL).map((control) => {
            const current = status.razerButtonMappings?.[control] ?? "";
            return (
              <div class="setting-row" key={control}>
                <div class="setting-label">
                  <span class="setting-title">{RAZER_BUTTON_LABEL[control]}</span>
                  <span class="setting-description">Choose what this button sends.</span>
                </div>
                <select
                  class="performance-select"
                  value={current}
                  disabled={disabled}
                  onChange={(e) => {
                    const mapping = (e.target as HTMLSelectElement).value as (typeof RAZER_BUTTON_MAPPINGS)[number];
                    void writeToast(
                      mapping === "Disabled" ? "Disabled button" : `Rebound ${RAZER_BUTTON_LABEL[control]}`,
                      setButtonMapping(info, control, mapping),
                      onApplied,
                      () => ({ razerButtonMappings: { ...status.razerButtonMappings, [control]: mapping } }),
                    );
                  }}
                >
                  {RAZER_BUTTON_MAPPINGS.map((mapping) => (
                    <option key={mapping} value={mapping}>{mapping}</option>
                  ))}
                </select>
              </div>
            );
          })}
        </div>
      )}

      {isRazer && status.razerButtonMappings && (
        <div class="setting-row-block">
          <span class="setting-eyebrow">Razer — Scroll &amp; toggles</span>
          {RAZER_TOGGLE_CONTROLS.map((control) => {
            const infoRow = RAZER_TOGGLE_CONTROL_INFO[control];
            const current = status.razerButtonMappings?.[control] ?? "";
            return (
              <div class="setting-row" key={control}>
                <div class="setting-label">
                  <span class="setting-title">{infoRow.label}</span>
                  <span class="setting-description">{infoRow.enabledLabel}</span>
                </div>
                <input
                  class="performance-select"
                  type="text"
                  value={current}
                  placeholder="Disabled"
                  disabled={disabled}
                  onBlur={(e) => {
                    const v = (e.target as HTMLInputElement).value.trim();
                    void writeToast(
                      infoRow.label,
                      setButtonMapping(info, control, v === "" ? "Disabled" : v),
                      onApplied,
                      () => ({ razerButtonMappings: { ...status.razerButtonMappings, [control]: v === "" ? "Disabled" : v } }),
                    );
                  }}
                />
              </div>
            );
          })}
        </div>
      )}

      {/* ── Endgame Gear ───────────────────────────────────────── */}
      {isEgg && status.eggButtonMappings && (
        <div class="setting-row-block">
          <span class="setting-eyebrow">Endgame Gear — Buttons</span>
          {EGG_BUTTON_NAMES.map((name, index) => {
            const current = status.eggButtonMappings?.[index] ?? "";
            return (
              <div class="setting-row" key={name}>
                <div class="setting-label">
                  <span class="setting-title">{name}</span>
                </div>
                <select
                  class="performance-select"
                  value={current}
                  disabled={disabled}
                  onChange={(e) => {
                    const mapping = (e.target as HTMLSelectElement).value as (typeof EGG_BUTTON_MAPPINGS)[number];
                    void writeToast(
                      `Rebound ${name}`,
                      setButtonMapping(info, index, mapping),
                      onApplied,
                      () => {
                        const next = [...(status.eggButtonMappings ?? [])];
                        next[index] = mapping;
                        return { eggButtonMappings: next } as Partial<MouseStatus>;
                      },
                    );
                  }}
                >
                  {EGG_BUTTON_MAPPINGS.map((mapping) => (
                    <option key={mapping} value={mapping}>{mapping}</option>
                  ))}
                </select>
              </div>
            );
          })}
        </div>
      )}

      {isEgg && Array.isArray(status.eggMulticlickFilters) && status.eggMulticlickFilters.length > 0 && (
        <div class="setting-row-block">
          <span class="setting-eyebrow">Endgame Gear — Debounce</span>
          {EGG_BUTTON_NAMES.slice(0, status.eggMulticlickFilters.length).map((name, index) => {
            const current = status.eggMulticlickFilters?.[index] ?? 16;
            return (
              <div class="setting-row" key={`debounce-${name}`}>
                <div class="setting-label">
                  <span class="setting-title">{(current as number) > 0 ? `${name} (${current} ms)` : `${name} (off)`}</span>
                </div>
                <select
                  class="performance-select"
                  value={String(current)}
                  disabled={disabled}
                  onChange={(e) => {
                    const v = Number((e.target as HTMLSelectElement).value);
                    void writeToast(
                      `${name} debounce`,
                      setMulticlickFilter(info, index, v),
                      onApplied,
                      () => {
                        const next = [...(status.eggMulticlickFilters ?? [])];
                        next[index] = v;
                        return { eggMulticlickFilters: next } as Partial<MouseStatus>;
                      },
                    );
                  }}
                >
                  {[0, 1, 2, 4, 8, 12, 16].map((v) => (
                    <option key={v} value={v}>{v === 0 ? "Off" : `${v} ms`}</option>
                  ))}
                </select>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}