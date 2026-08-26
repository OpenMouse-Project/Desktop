import { useEffect, useRef, useState } from "preact/hooks";
import { ArrowLeft, Trash2, Zap } from "lucide-preact";
import type { Game } from "../hooks/use-game-watcher";
import type { MouseConnection } from "../hooks/use-mouse-connection";
import type { CandidateInterface } from "../native-hid/scan";
import { deviceImage, UNKNOWN_DEVICE_IMAGE } from "../native-hid/device-images";
import {
  applyGameProfile,
  clearGameProfile,
  describeProfile,
  getGameProfile,
  isProfileMeaningful,
  saveGameProfile,
  type GameProfile,
} from "../lib/game-profiles";
import { DPI_MAX, DPI_MIN, DPI_PRESETS, DPI_STEP, GAMING_SURFACE_MODES } from "../lib/logitech-controls";
import { showToast } from "../lib/toast";

function fallbackToUnknownDevice(event: Event) {
  const img = event.currentTarget as HTMLImageElement;
  if (img.src.endsWith(UNKNOWN_DEVICE_IMAGE)) return;
  img.src = UNKNOWN_DEVICE_IMAGE;
}

interface Props {
  game: Game;
  connection: MouseConnection;
  onClose: () => void;
}

/**
 * Editor for one game's saved DPI/polling-rate profile (lib/game-profiles.ts)
 * — full page rather than a modal, styled after OverviewPage's own device
 * dashboard (hero art up top, device-tabs-back button, the same
 * dpi-panel/performance-chip-group DevicePerformanceTab uses) so this
 * doesn't feel like a different app bolted onto Games. Only DPI and polling
 * rate are editable here, not every writable setting DevicePerformanceTab
 * exposes, since those two are close to the only ones near-universally
 * supported across gaming mice. Either field left at "Not set" means this
 * profile doesn't touch that setting when it's applied.
 *
 * A profile isn't pinned to whatever mouse happens to be plugged in when you
 * open this page — the device picker below lets you choose which one it
 * targets, same `select()` OverviewPage's own device list uses (switches to
 * a cached device instantly, connects fresh otherwise), so picking a
 * different mouse here is the same "which device is active" state the rest
 * of the app already shares, not a second parallel notion of "connected."
 *
 * Every field persists to lib/game-profiles.ts as it's edited (see the
 * effect below) rather than needing an explicit "Save" click — CONFIRMED
 * the source of a real bug when this was a modal: closing it via the × or
 * by clicking outside never saved anything, so setting DPI/rate and
 * flipping "Apply automatically" then just closing (the natural thing to
 * do) silently discarded all of it. use-game-watcher.ts's launch check
 * found no saved profile and quietly did nothing — no error anywhere,
 * exactly matching "the game launching doesn't seem to change the mouse at
 * all." Every other toggle in this app (Discord RPC, Mode) already applies
 * on change, not on a separate save step; this matches.
 */
export function GameProfilePanel({ game, connection, onClose }: Props) {
  const existing = getGameProfile(game.id);
  const { connected, connectedInfo, list, connectingKey, select } = connection;
  const status = connected?.status;
  const canControl = connected?.brand === "Logitech" && connectedInfo !== null;
  const showSeparateAxes = status?.supportsSeparateDpiAxes === true;

  const [dpiInput, setDpiInput] = useState(existing?.dpi !== undefined ? String(existing.dpi) : "");
  const [dpiYInput, setDpiYInput] = useState(existing?.dpiY !== undefined ? String(existing.dpiY) : "");
  const [pollingRateHz, setPollingRateHz] = useState<number | undefined>(existing?.pollingRateHz);
  const [liftOffDistance, setLiftOffDistance] = useState<GameProfile["liftOffDistance"]>(existing?.liftOffDistance);
  const [gamingSurfaceMode, setGamingSurfaceMode] = useState<GameProfile["gamingSurfaceMode"]>(existing?.gamingSurfaceMode);
  const [autoApply, setAutoApply] = useState(existing?.autoApply ?? false);
  const [pending, setPending] = useState<"apply" | null>(null);

  function buildProfile(): GameProfile {
    const dpi = dpiInput.trim() === "" ? undefined : Math.round(Number(dpiInput));
    const dpiY = showSeparateAxes && dpiYInput.trim() !== "" ? Math.round(Number(dpiYInput)) : undefined;
    return {
      dpi: dpi !== undefined && Number.isFinite(dpi) && dpi > 0 ? dpi : undefined,
      dpiY: dpiY !== undefined && Number.isFinite(dpiY) && dpiY > 0 ? dpiY : undefined,
      pollingRateHz,
      liftOffDistance,
      gamingSurfaceMode,
      autoApply,
    };
  }

  // Skip the very first run — there's nothing to persist yet that
  // getGameProfile() didn't already have (this just mirrors `existing`
  // back at itself), and doing it anyway would create an empty saved entry
  // for every game the moment its page is opened, whether or not anything
  // was actually configured.
  const firstRun = useRef(true);
  useEffect(() => {
    if (firstRun.current) {
      firstRun.current = false;
      return;
    }
    saveGameProfile(game.id, buildProfile());
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [dpiInput, dpiYInput, pollingRateHz, liftOffDistance, gamingSurfaceMode, autoApply]);

  async function handleApplyNow() {
    const profile = buildProfile();
    if (!isProfileMeaningful(profile) || !connectedInfo) return;
    setPending("apply");
    try {
      await applyGameProfile(connectedInfo, profile, connection.patchStatus);
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
    showToast(`Cleared profile for ${game.name}.`, "info");
  }

  function handleSelectDevice(candidate: CandidateInterface) {
    select(candidate);
  }

  const busy = pending !== null;
  const draft = buildProfile();
  const canApplyNow = canControl && isProfileMeaningful(draft) && !busy;
  const hasSavedProfile = isProfileMeaningful(draft) || autoApply;
  const candidates = list.status === "loaded" ? list.candidates : [];

  return (
    <section class="page page-game-profile">
      <button class="device-tab-back profile-back-button" onClick={onClose}>
        <ArrowLeft size={13} aria-hidden="true" /> Games
      </button>

      <div class="profile-layout">
        <div class="profile-art-col">
        <div class="profile-art-sticky">
          {game.artworkSecondary ? (
            <div class="game-card-art-split profile-hero-art">
              <img src={game.artwork} alt="" />
              <img src={game.artworkSecondary} alt="" />
            </div>
          ) : game.steamAppId || game.artwork ? (
            <img
              class="profile-hero-art"
              src={
                game.artwork ??
                `https://cdn.cloudflare.steamstatic.com/steam/apps/${game.steamAppId}/library_600x900_2x.jpg`
              }
              alt=""
              onError={(event) => {
                if (game.artworkFallback && event.currentTarget.src !== game.artworkFallback) {
                  event.currentTarget.src = game.artworkFallback;
                } else {
                  event.currentTarget.style.display = "none";
                }
              }}
            />
          ) : (
            <div class="profile-hero-art profile-hero-art-fallback" aria-hidden="true">
              {game.name.slice(0, 1)}
            </div>
          )}
          <div class="profile-hero-info info-section">
            <span class="setting-eyebrow">Profile</span>
            <h1 class="device-showcase-name">{game.name}</h1>
            <div class="profile-hero-actions">
              {hasSavedProfile && (
                <button class="rescan-button" onClick={handleClear} disabled={busy}>
                  <Trash2 size={14} /> Clear
                </button>
              )}
              <button class="rescan-button" onClick={() => void handleApplyNow()} disabled={!canApplyNow}>
                <Zap size={14} /> {pending === "apply" ? "Applying…" : "Apply now"}
              </button>
            </div>
          </div>
        </div>
        </div>

        <div class="profile-content-col">
          <div class="info-section">
            <span class="info-section-title">Target Device</span>
            {candidates.length === 0 ? (
              <p class="game-profile-hint">
                No devices found. Connect a supported OpenMouse device to apply this profile.
              </p>
            ) : (
              <ul class="device-list profile-device-list">
                {candidates.map((candidate) => (
                  <li class="device-list-row" key={candidate.info.key}>
                    <div class="device-list-row-main">
                      <img
                        class="device-list-row-image"
                        src={deviceImage(candidate.info.key, candidate.info.productString)}
                        onError={fallbackToUnknownDevice}
                        alt=""
                      />
                      <div class="device-list-row-info">
                        <span class="device-list-row-name">
                          {candidate.info.productString || "Unknown device"}
                        </span>
                        <span class="device-list-row-meta">{candidate.brands.join(" / ")}</span>
                      </div>
                    </div>
                    <button
                      class="connect-button"
                      disabled={connectingKey === candidate.info.key}
                      onClick={() => handleSelectDevice(candidate)}
                    >
                      {connectingKey === candidate.info.key
                        ? "Connecting…"
                        : connected?.key === candidate.info.key
                          ? "Selected"
                          : "Select"}
                    </button>
                  </li>
                ))}
              </ul>
            )}
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

          {(status?.gamingSurfaceMode != null || (status?.supportedLiftOffDistances && status.supportedLiftOffDistances.length > 0)) && (
            <div class="sensor-panel">
              <span class="setting-eyebrow">Sensor</span>

              {status?.gamingSurfaceMode != null && (
                <>
                  <span class="sensor-panel-title">Gaming surface</span>
                  <div class="performance-chip-group">
                    <button
                      class={`performance-chip ${gamingSurfaceMode === undefined ? "active" : ""}`}
                      onClick={() => setGamingSurfaceMode(undefined)}
                    >
                      Not set
                    </button>
                    {GAMING_SURFACE_MODES.map((mode) => (
                      <button
                        key={mode}
                        class={`performance-chip ${mode === gamingSurfaceMode ? "active" : ""}`}
                        onClick={() => setGamingSurfaceMode(mode)}
                      >
                        {mode}
                      </button>
                    ))}
                  </div>
                </>
              )}

              {status?.supportedLiftOffDistances && status.supportedLiftOffDistances.length > 0 && (
                <>
                  <span class="sensor-panel-title">Lift-off distance</span>
                  <div class="performance-chip-group">
                    <button
                      class={`performance-chip ${liftOffDistance === undefined ? "active" : ""}`}
                      onClick={() => setLiftOffDistance(undefined)}
                    >
                      Not set
                    </button>
                    {status.supportedLiftOffDistances.map((value) => (
                      <button
                        key={value}
                        class={`performance-chip ${value === liftOffDistance ? "active" : ""}`}
                        onClick={() => setLiftOffDistance(value)}
                      >
                        {value}
                      </button>
                    ))}
                  </div>
                </>
              )}
            </div>
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
        </div>
      </div>
    </section>
  );
}
