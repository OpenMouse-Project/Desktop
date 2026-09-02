import { useEffect, useState } from "preact/hooks";
import { AlertTriangle } from "lucide-preact";
import type { MouseLighting, MouseStatus } from "@openmouse/protocol/drivers/mouse-types";
import type { HidInterfaceInfo } from "../native-hid/tauri-hid-device";
import { setLighting } from "../native-hid/write";
import { showToast } from "../lib/toast";
import { RadialColorPicker } from "./RadialColorPicker";

interface Props {
  info: HidInterfaceInfo;
  status: MouseStatus;
  onApplied: (patch: Partial<MouseStatus>) => void;
  readOnly?: boolean;
}

export function DeviceLightingTab({ info, status, onApplied, readOnly }: Props) {
  const lighting = status.lighting;
  const [pending, setPending] = useState(false);

  const [stagedMode, setStagedMode] = useState<string | null>(lighting?.mode ?? null);
  const [stagedColor, setStagedColor] = useState(lighting?.color ?? "#ff0000");
  const [stagedColor2, setStagedColor2] = useState(lighting?.color2 ?? "#0000ff");
  const [stagedSpeed, setStagedSpeed] = useState<number>(lighting?.speed ?? 1);

  useEffect(() => { setStagedMode(lighting?.mode ?? null); }, [lighting?.mode]);
  useEffect(() => { setStagedColor(lighting?.color ?? "#ff0000"); }, [lighting?.color]);
  useEffect(() => { setStagedColor2(lighting?.color2 ?? "#0000ff"); }, [lighting?.color2]);
  useEffect(() => { setStagedSpeed(lighting?.speed ?? 1); }, [lighting?.speed]);

  if (!lighting) {
    return (
      <div class="tab-placeholder">
        <p>No lighting data available for this device.</p>
      </div>
    );
  }

  const busy = pending || readOnly;
  const showColor = lighting.colorModes.includes(stagedMode as any);
  const showDualColor = lighting.dualColorModes.includes(stagedMode as any);
  const showSpeed = lighting.reactiveModes.includes(stagedMode as any);

  const dirty =
    stagedMode !== lighting.mode ||
    stagedColor !== (lighting.color ?? "#ff0000") ||
    stagedColor2 !== (lighting.color2 ?? "#0000ff") ||
    stagedSpeed !== (lighting.speed ?? 1);

  function revert() {
    setStagedMode(lighting?.mode ?? null);
    setStagedColor(lighting?.color ?? "#ff0000");
    setStagedColor2(lighting?.color2 ?? "#0000ff");
    setStagedSpeed(lighting?.speed ?? 1);
  }

  async function applyAll() {
    if (!stagedMode) return;
    setPending(true);
    try {
      const patch: MouseLighting = {
        zone: lighting!.zone,
        modes: lighting!.modes,
        mode: stagedMode as MouseLighting["mode"],
        color: showColor ? stagedColor : null,
        color2: showDualColor ? stagedColor2 : null,
        colorModes: lighting!.colorModes,
        dualColorModes: lighting!.dualColorModes,
        reactiveModes: lighting!.reactiveModes,
        speeds: lighting!.speeds,
        speed: showSpeed ? stagedSpeed : null,
        writeOnly: lighting!.writeOnly,
      };
      const applied = await setLighting(info, patch);
      onApplied({ lighting: applied });
      showToast("Lighting applied.", "success");
    } catch (err) {
      showToast(err instanceof Error ? err.message : String(err), "error");
    } finally {
      setPending(false);
    }
  }

  return (
    <div class="performance-tab">
      {readOnly && (
        <p class="performance-locked-note">
          Read-only — lighting controls are not available for this device yet.
        </p>
      )}

      {lighting.writeOnly && !readOnly && (
        <p class="performance-locked-note">
          Lighting is write-only on this device. The current effect is cached from the last change and may not reflect the hardware state.
        </p>
      )}

      {/* ── Single lighting card ─────────────────────────────────── */}
      <div class="dpi-panel">
        <div class="dpi-panel-header">
          <div>
            <span class="setting-eyebrow">Lighting</span>
            <span class="setting-title">Effect</span>
          </div>
        </div>

        {/* Effect grid */}
        <div class="dpi-preset-grid">
          {lighting.modes.map((mode) => (
            <button
              key={mode}
              class={`dpi-preset ${mode === stagedMode ? "active" : ""}`}
              disabled={busy}
              onClick={() => setStagedMode(mode)}
            >
              {mode}
            </button>
          ))}
        </div>

        {/* Color pickers */}
        {showColor && (
          <>
            <div class="lighting-divider" />
            <div class="lighting-color-section">
              <span class="setting-title">Color</span>
              <div class="radial-picker-row">
                <RadialColorPicker
                  value={stagedColor}
                  onChange={setStagedColor}
                  disabled={busy}
                />
                {showDualColor && (
                  <RadialColorPicker
                    value={stagedColor2}
                    onChange={setStagedColor2}
                    disabled={busy}
                  />
                )}
              </div>
            </div>
          </>
        )}

        {/* Speed */}
        {showSpeed && lighting.speeds.length > 0 && (
          <>
            <div class="lighting-divider" />
            <div class="lighting-speed-section">
              <span class="setting-title">Speed</span>
              <div class="dpi-preset-grid lighting-speed-grid">
                {lighting.speeds.map((s) => (
                  <button
                    key={s}
                    class={`dpi-preset ${s === stagedSpeed ? "active" : ""}`}
                    disabled={busy}
                    onClick={() => setStagedSpeed(s)}
                  >
                    {s}
                  </button>
                ))}
              </div>
            </div>
          </>
        )}
      </div>

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
