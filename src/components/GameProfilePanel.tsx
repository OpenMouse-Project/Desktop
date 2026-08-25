import { useState } from "preact/hooks";
import { Save, Trash2, X, Zap } from "lucide-preact";
import type { Game } from "../hooks/use-game-watcher";
import type { MouseConnection } from "../hooks/use-mouse-connection";
import {
  applyGameProfile,
  clearGameProfile,
  describeProfile,
  getGameProfile,
  isProfileMeaningful,
  saveGameProfile,
  type GameProfile,
} from "../lib/game-profiles";
import { DPI_MAX, DPI_MIN, DPI_PRESETS, DPI_STEP } from "../lib/logitech-controls";
import { showToast } from "../lib/toast";

interface Props {
  game: Game;
  connection: MouseConnection;
  onClose: () => void;
}

/**
 * Editor for one game's saved DPI/polling-rate profile (lib/game-profiles.ts)
 * — just those two, not every writable setting DevicePerformanceTab exposes,
 * since DPI and polling rate are close to the only ones near-universally
 * supported across gaming mice. Modeled after DevicePerformanceTab's own
 * controls (same presets, same chip group) so the two don't feel like
 * different apps. Either field left at "Not set" means this profile doesn't
 * touch that setting when it's applied.
 *
 * The polling-rate row only shows up once a controllable mouse is connected
 * — its valid values are whatever that specific device reports supporting,
 * not a fixed list this panel could guess at. DPI has no such dependency (a
 * plain number), so it's always editable.
 */
export function GameProfilePanel({ game, connection, onClose }: Props) {
  const existing = getGameProfile(game.id);
  const status = connection.connected?.status;
  const canControl = connection.connected?.brand === "Logitech" && connection.connectedInfo !== null;
  const showSeparateAxes = status?.supportsSeparateDpiAxes === true;

  const [dpiInput, setDpiInput] = useState(existing?.dpi !== undefined ? String(existing.dpi) : "");
  const [dpiYInput, setDpiYInput] = useState(existing?.dpiY !== undefined ? String(existing.dpiY) : "");
  const [pollingRateHz, setPollingRateHz] = useState<number | undefined>(existing?.pollingRateHz);
  const [autoApply, setAutoApply] = useState(existing?.autoApply ?? false);
  const [hasSavedProfile, setHasSavedProfile] = useState(existing !== undefined);
  const [pending, setPending] = useState<"save" | "apply" | null>(null);

  function buildProfile(): GameProfile {
    const dpi = dpiInput.trim() === "" ? undefined : Math.round(Number(dpiInput));
    const dpiY = showSeparateAxes && dpiYInput.trim() !== "" ? Math.round(Number(dpiYInput)) : undefined;
    return {
      dpi: dpi !== undefined && Number.isFinite(dpi) && dpi > 0 ? dpi : undefined,
      dpiY: dpiY !== undefined && Number.isFinite(dpiY) && dpiY > 0 ? dpiY : undefined,
      pollingRateHz,
      autoApply,
    };
  }

  function handleSave() {
    saveGameProfile(game.id, buildProfile());
    setHasSavedProfile(true);
    showToast(`Saved profile for ${game.name}.`, "success");
  }

  async function handleApplyNow() {
    const profile = buildProfile();
    if (!isProfileMeaningful(profile) || !connection.connectedInfo) return;
    setPending("apply");
    try {
      await applyGameProfile(connection.connectedInfo, profile, connection.patchStatus);
      showToast(`Applied "${game.name}" profile — ${describeProfile(profile)}.`, "success");
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error), "error");
    } finally {
      setPending(null);
    }
  }

  function handleClear() {
    clearGameProfile(game.id);
    setDpiInput("");
    setDpiYInput("");
    setPollingRateHz(undefined);
    setAutoApply(false);
    setHasSavedProfile(false);
    showToast(`Cleared profile for ${game.name}.`, "info");
  }

  const busy = pending !== null;
  const draft = buildProfile();
  const canApplyNow = canControl && isProfileMeaningful(draft) && !busy;

  return (
    <div class="game-profile-overlay" onClick={onClose}>
      <div class="game-profile-panel" onClick={(event) => event.stopPropagation()}>
        <div class="game-profile-panel-header">
          <div>
            <span class="setting-eyebrow">Profile</span>
            <h2>{game.name}</h2>
          </div>
          <button class="icon-button" onClick={onClose} title="Close">
            <X size={16} />
          </button>
        </div>

        <div class="dpi-panel">
          <div class="dpi-panel-header">
            <div class="setting-label">
              <span class="setting-eyebrow">DPI</span>
              <span class="setting-title">Sensitivity</span>
            </div>
            {dpiInput !== "" && (
              <button class="chip-clear" onClick={() => { setDpiInput(""); setDpiYInput(""); }}>
                Not set
              </button>
            )}
          </div>

          <div class="dpi-preset-grid">
            {DPI_PRESETS.map((preset) => (
              <button
                key={preset}
                class={`dpi-preset ${String(preset) === dpiInput ? "active" : ""}`}
                onClick={() => {
                  setDpiInput(String(preset));
                  if (showSeparateAxes) setDpiYInput(String(preset));
                }}
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
                placeholder="Not set"
                value={dpiInput}
                onInput={(event) => setDpiInput((event.target as HTMLInputElement).value)}
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
                  placeholder="Not set"
                  value={dpiYInput}
                  onInput={(event) => setDpiYInput((event.target as HTMLInputElement).value)}
                />
              </label>
            )}
          </div>
        </div>

        {status?.supportedPollingRates && status.supportedPollingRates.length > 0 && (
          <div class="setting-row setting-row-block">
            <div class="setting-label">
              <span class="setting-title">Polling rate</span>
            </div>
            <div class="performance-chip-group">
              <button
                class={`performance-chip ${pollingRateHz === undefined ? "active" : ""}`}
                onClick={() => setPollingRateHz(undefined)}
              >
                Not set
              </button>
              {status.supportedPollingRates.map((hz) => (
                <button
                  key={hz}
                  class={`performance-chip ${hz === pollingRateHz ? "active" : ""}`}
                  onClick={() => setPollingRateHz(hz)}
                >
                  {hz} Hz
                </button>
              ))}
            </div>
          </div>
        )}

        {!canControl && (
          <p class="game-profile-hint">
            Connect a controllable mouse to also set a polling rate for this game.
          </p>
        )}

        <div class="setting-row">
          <div class="setting-label">
            <span class="setting-title">Apply automatically</span>
            <span class="setting-description">
              Push this profile to your mouse the moment {game.name} is detected running.
            </span>
          </div>
          <label class="switch">
            <input
              type="checkbox"
              checked={autoApply}
              onChange={(event) => setAutoApply(event.currentTarget.checked)}
            />
            <span class="switch-track" />
          </label>
        </div>

        <div class="game-profile-panel-footer">
          {hasSavedProfile && (
            <button class="rescan-button" onClick={handleClear} disabled={busy}>
              <Trash2 size={14} /> Clear
            </button>
          )}
          <button class="rescan-button" onClick={() => void handleApplyNow()} disabled={!canApplyNow}>
            <Zap size={14} /> {pending === "apply" ? "Applying…" : "Apply now"}
          </button>
          <button class="connect-button" onClick={handleSave} disabled={busy}>
            <Save size={14} /> Save profile
          </button>
        </div>
      </div>
    </div>
  );
}
