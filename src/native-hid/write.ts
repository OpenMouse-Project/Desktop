// Generic per-brand write actions for the device-control tabs.
//
// `connectToInterface()` (scan.ts) reads status generically: every driver
// class declares the same open/close/readStatus shape, so one loop probes
// candidates by vendor id and the first that answers wins. Commands that
// actually change the device are a different story — each brand's driver
// class exposes its own set of setter methods, and Logitech's receiver-paired
// HID++ indexing (logitech-actions.ts) needs its own module for that alone.
//
// This is the generic layer for every other brand. It picks the same candidate drivers
// `connectToInterface()` does (brands.ts's candidatesForVendorId), models
// the common setter surface they share as one type where every method is
// optional, and drives each action the same open -> act -> close way the
// status read already does — fresh short-lived connection under the shared
// hid-open-lock, never a persistent one (see scan.ts's docs on why).
//
// Unlike logitech-actions.ts this needs no device-index resolution: Logitech
// is the only receiver-paired family where a write can target the wrong HID++
// index, and that path already lives in logitech-actions.ts. Every other
// driver here constructs on a real interface, so `open()` + one setter round
// trip is all a write costs.

import { invoke } from "@tauri-apps/api/core";
import type { MouseLighting, MouseStatus } from "@openmouse/protocol/drivers/mouse-types";
import { candidatesForDevice } from "./brands";
import { TauriHidDevice, type HidInterfaceInfo } from "./tauri-hid-device";
import { isQueuedNotProcessedError, withHidOpenLockRetrying } from "./hid-open-lock";

/**
 * What this device accepts for the controls whose valid values are set by the
 * device's own protocol, asked of the driver class that actually answered it.
 *
 * These lists live on the driver classes (not in `MouseStatus`) and are pure —
 * they read product ids / catalog entries, never the wire — so this needs no
 * connection and no lock. Before this, the Performance tab hardcoded stand-in
 * lists (10 min-8 h sleep values, 0-20 ms debounce) that any driver with a
 * different set rejected outright: Razer takes 1-15 min of sleep, ATK 30 s-30
 * min in its own steps, Ninjutso 1-15 min, WLMouse 30 s-30 min, and their
 * debounce ceilings differ per protocol — every one of those writes simply
 * failed.
 *
 * `driver` is `ConnectedDevice.driver`; without it the first candidate for the
 * vendor/product is used, which is the same order `connectToInterface()` tries
 * them in.
 */
export interface DeviceCapabilities {
  /**
   * Sleep-timeout values this device accepts, in the unit its own
   * `setSleepTimeout` takes (seconds for every driver that exposes a list).
   * Null when its driver exposes none — then no value the app could offer is
   * known to be valid, so the control must not be shown.
   */
  sleepTimeouts: readonly number[] | null;
  /** Low-power warning thresholds (battery %) this device accepts, or null. */
  lowPowerThresholds: readonly number[] | null;
  /** Highest debounce value in ms this device accepts, or null (0 means the protocol has no such control). */
  debounceMaxMs: number | null;
  /** Exact debounce values this device accepts, when its driver lists them rather than just a ceiling. */
  debounceOptions: readonly number[] | null;
  /**
   * Highest DPI this device accepts, when its driver reports one — per
   * *product*, not per brand (a Razer DeathAdder V3 HyperSpeed takes 30000,
   * a Viper V3 Pro 35000). Null when the driver has no such ceiling to
   * report, in which case the UI falls back to its own bounds.
   */
  dpiMax: number | null;
}

export function deviceCapabilities(info: HidInterfaceInfo): DeviceCapabilities {
  const empty: DeviceCapabilities = {
    sleepTimeouts: null,
    lowPowerThresholds: null,
    debounceMaxMs: null,
    debounceOptions: null,
    dpiMax: null,
  };
  // Whatever the walk would have chosen for this device, by the library's own
  // rule — see brands.ts's candidatesForDevice.
  const candidate = candidatesForDevice(info)[0];
  if (!candidate) return empty;

  // Constructing a client is side-effect free (no open, no listener — the
  // classes only store the device and these getters read product ids and
  // catalogs), but this runs during render against every registered driver
  // class, so a getter that throws must read as "capability not offered"
  // rather than take the whole device page down with it.
  try {
    const client = candidate.create(new TauriHidDevice(info)) as unknown as {
      getSleepOptions?: () => readonly number[];
      getLowPowerOptions?: () => readonly number[];
      getDebounceMaxMs?: () => number;
      getDebounceOptions?: () => readonly number[];
      maxDpi?: () => number;
    } | null;
    if (!client) return empty;

    const sleepTimeouts = client.getSleepOptions?.() ?? null;
    const lowPowerThresholds = client.getLowPowerOptions?.() ?? null;
    const debounceMaxMs = client.getDebounceMaxMs?.() ?? null;
    const debounceOptions = client.getDebounceOptions?.() ?? null;
    const dpiMax = client.maxDpi?.() ?? null;
    return {
      sleepTimeouts: sleepTimeouts?.length ? sleepTimeouts : null,
      lowPowerThresholds: lowPowerThresholds?.length ? lowPowerThresholds : null,
      debounceMaxMs: typeof debounceMaxMs === "number" && debounceMaxMs > 0 ? debounceMaxMs : null,
      debounceOptions: debounceOptions?.length ? debounceOptions : null,
      dpiMax: typeof dpiMax === "number" && dpiMax > 0 ? dpiMax : null,
    };
  } catch {
    return empty;
  }
}

/**
 * DPI bounds to offer for a device, in precedence order: the device's own
 * protocol hint (`ui.dpiStageEditor` — the exact values its setter validates
 * against), then its driver's per-product ceiling (`maxDpi()`), then the
 * caller's fallback. Shared by the Performance tab and the game-profile
 * editor so both offer the same limits, and so neither keeps a per-brand
 * guess that refuses DPI the mouse accepts.
 */
export function dpiBounds(
  status: MouseStatus,
  capabilities: DeviceCapabilities,
  fallback: { min: number; max: number; step: number },
): { min: number; max: number; step: number } {
  const editor = status.ui?.dpiStageEditor;
  return {
    min: editor?.minDpi ?? fallback.min,
    max: editor?.maxDpi ?? capabilities.dpiMax ?? fallback.max,
    step: editor?.stepDpi ?? fallback.step,
  };
}

const OPEN_TIMEOUT_MS = 10000;
// A single write is one request/response round trip — the same generous but
// far-short-of-a-readStatus-walk budget logitech-actions.ts uses.
const ACTION_TIMEOUT_MS = 15000;
// How long the write path waits for the mouse's queued command to be processed
// before re-running the same set operation, and how many times. A wireless
// mouse that has to wake to run the command answers the first read with the
// unprocessed echo, so the budget is generous: six runs spaced 500 ms apart.
const DEVICE_BUSY_RETRY_DELAY_MS = 500;
const DEVICE_BUSY_RETRIES = 6;

/** Waits before re-running a set operation whose command the mouse had queued. */
function delay(ms: number): Promise<void> {
  // Executor form, matching `withTimeout` below: this project's TS lib predates
  // ES2024, so `Promise.withResolvers` does not type-check here.
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

function withTimeout<T>(promise: Promise<T>, ms: number, label: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`${label} timed out after ${ms}ms`)), ms);
    promise.then(
      (value) => { clearTimeout(timer); resolve(value); },
      (error) => { clearTimeout(timer); reject(error); },
    );
  });
}

export type LiftOffDistance = NonNullable<MouseStatus["liftOffDistance"]>;
export type GamingSurfaceMode = NonNullable<MouseStatus["gamingSurfaceMode"]>;

/**
 * The cross-brand setter surface a driver class may implement. Every method
 * is optional — each driver exposes a subset (a Ninjutso has setLighting, a
 * moddo does not; a Pulsar XS1 lacks setSleepTimeout, etc.). Cast each class
 * through `unknown` for the exact reason logitech-actions.ts documents: some
 * classes mark `open()` private in their `.d.ts` even though every runtime
 * build implements it.
 */
export interface WritableClient {
  open(): Promise<void>;
  close(): Promise<void>;
  setDpi?(dpi: number, dpiY?: number): Promise<unknown>;
  setPollingRate?(rate: number): Promise<unknown>;
  setLiftOffDistance?(value: LiftOffDistance): Promise<unknown>;
  setGamingSurfaceMode?(mode: GamingSurfaceMode): Promise<unknown>;
  setMotionSync?(enabled: boolean): Promise<unknown>;
  setAngleSnapping?(enabled: boolean): Promise<unknown>;
  setRippleControl?(enabled: boolean): Promise<unknown>;
  setDebounceTime?(milliseconds: number): Promise<unknown>;
  setSleepTimeout?(seconds: number): Promise<unknown>;
  setPerformanceMode?(enabled: boolean): Promise<unknown>;
  setHyperMode?(enabled: boolean): Promise<unknown>;
  setSensorMode?(mode: "Eco" | "High" | "Ultra"): Promise<unknown>;
  setPerformanceDuration?(seconds: number): Promise<unknown>;
  setActiveDpiStage?(stage: number): Promise<unknown>;
  setDpiStageValue?(stage: number, dpi: number): Promise<unknown>;
  setDpiStageCount?(count: number): Promise<unknown>;
  setUsbSpeed?(speed: "Full" | "High"): Promise<unknown>;
  setPrimaryButton?(button: "Left" | "Right"): Promise<unknown>;
  setLowPowerThreshold?(percent: number): Promise<unknown>;
  setWheelAcceleration?(enabled: boolean): Promise<unknown>;
  setAngleTuning?(value: number): Promise<unknown>;
  setLighting?(lighting: MouseLighting): Promise<unknown>;
  // ── brand-specific setters (each only exists on that brand's driver) ──
  // Endgame Gear (egg-op1)
  setSlamclickFilter?(enabled: boolean): Promise<unknown>;
  setMotionJitterFilter?(enabled: boolean): Promise<unknown>;
  setSpdtMode?(button: "left" | "right", mode: "Off" | "GX Safe" | "GX Speed"): Promise<unknown>;
  setCpiLevels?(levels: number): Promise<unknown>;
  setCpiStage?(level: number, x: number, y: number): Promise<unknown>;
  setMulticlickFilter?(button: number, value: number): Promise<unknown>;
  setButtonMapping?(button: number, action: string): Promise<unknown>;
  setToggleControl?(control: string, label: string): Promise<unknown>;
  setCustomPollingDivider?(divider: number): Promise<unknown>;
  setGlassMode?(enabled: boolean): Promise<unknown>;
  setEggLodIndex?(index: number): Promise<unknown>;
  setLedLiftOffDisabled?(enabled: boolean): Promise<unknown>;
  setLeftHanded?(enabled: boolean): Promise<unknown>;
  // Ninjutso
  setNinjutsoSystemMode?(mode: "High Speed" | "Competitive" | "Ultra"): Promise<unknown>;
  setNinjutsoHyperClick?(enabled: boolean): Promise<unknown>;
  setNinjutsoOpticalEngine?(mode: "Standard" | "Burst"): Promise<unknown>;
  setNinjutsoSlamClick?(level: "Low" | "Medium" | "High"): Promise<unknown>;
  setNinjutsoActiveDpiStage?(stage: number): Promise<unknown>;
  // Finalmouse
  setDongleLedMode?(mode: number): Promise<unknown>;
  setTournamentScrollMode?(mode: number): Promise<unknown>;
  setTournamentScrollTimeout?(milliseconds: number): Promise<unknown>;
  // Teevolution
  setDpiLighting?(mode: number, brightness: number, speed: number): Promise<unknown>;
  // Pulsar Pro
  setProfile?(profile: number): Promise<unknown>;
  setRemoteLedMode?(channel: 0 | 1, mode: number): Promise<unknown>;
  setDpiLed?(mode: number, brightness: number, speed: number): Promise<unknown>;
  // WALLHACK
  setGameMode?(enabled: boolean): Promise<unknown>;
  setActiveProfile?(profile: number): Promise<unknown>;
  setFactoryReset?(): Promise<unknown>;
  // Keychron Nape
  setLayer?(layer: number): Promise<unknown>;
}

export type SensorMode = NonNullable<MouseStatus["sensorMode"]>;
export type UsbSpeed = NonNullable<MouseStatus["usbSpeed"]>;
export type PrimaryButton = NonNullable<MouseStatus["primaryButton"]>;

/**
 * Opens each candidate driver for `info` in turn and runs `action` against
 * the first whose `open()` and the action both succeed within their own
 * budgets — the same "does it actually answer" bar connectToInterface() uses.
 * A candidate whose class lacks the setter the action calls (optional on
 * `WritableClient`) simply throws a TypeError caught here, so it's skipped
 * rather than failed; the first candidate that answers wins.
 *
 * Throws with the attempted candidates' errors when none answer, so a tab
 * surfaces a real error instead of a silent no-op.
 */
async function withClient<T>(
  info: HidInterfaceInfo,
  label: string,
  action: (client: WritableClient) => Promise<T>,
): Promise<T> {
  // `Retrying`, not plain `withHidOpenLock`: this is the click-path writer for
  // every brand the Logitech-specific path doesn't cover (it is where the
  // "No driver answered setDpi" modal comes from), and a click that lands
  // while the 5 s status walk holds the device lock must wait for it and then
  // land — logitech-actions.ts's own client has retried on that contention
  // since it was written, so a first click on a Razer failed where the same
  // click on a Logitech mouse succeeded.
  return withHidOpenLockRetrying(info.key, () => withClientLocked(info, label, action));
}

async function withClientLocked<T>(
  info: HidInterfaceInfo,
  label: string,
  action: (client: WritableClient) => Promise<T>,
): Promise<T> {
  const attempts: string[] = [];
  const candidates = candidatesForDevice(info);
  if (candidates.length === 0) {
    throw new Error(`This device's driver doesn't support ${label}.`);
  }
  for (const candidate of candidates) {
    const device = new TauriHidDevice(info, candidate.preferredCollection);
    const client = candidate.create(device) as unknown as WritableClient | null;
    if (!client) continue;
    const name = client.constructor.name || candidate.brand;
    let message = "";
    // One delayed re-run per candidate when the mouse echoes the request back
    // unprocessed — see DEVICE_BUSY_RETRY_DELAY_MS. Two, not one: the reply
    // that says "queued, not processed" can arrive while the status walk's
    // last request is still being processed on the mouse.
    for (let attempt = 0; attempt < DEVICE_BUSY_RETRIES; attempt += 1) {
      try {
        await withTimeout(client.open(), OPEN_TIMEOUT_MS, `${name}.open()`);
        const result = await withTimeout(action(client), ACTION_TIMEOUT_MS, `${name}.${label}`);
        await client.close().catch(() => undefined);
        if (attempt > 0) {
          void invoke("log_line", {
            line: `[write] ${label} on ${info.key}: ${name} answered on try ${attempt + 1} — the mouse had the command queued, not processed`,
          }).catch(() => {});
        }
        return result;
      } catch (error) {
        await client.close().catch(() => undefined);
        message = error instanceof Error ? error.message : String(error);
        if (!isQueuedNotProcessedError(message)) break;
        if (attempt < DEVICE_BUSY_RETRIES - 1) {
          await delay(DEVICE_BUSY_RETRY_DELAY_MS);
        }
      }
    }
    attempts.push(`${name}: ${message}`);
  }
  // Recorded, not only shown: a failed write is what users report, and the
  // modal text alone cannot say whether the queue retry above ran or how far
  // each candidate got.
  void invoke("log_line", {
    line: `[write] ${label} on ${info.key} failed. Tried:\n  ${attempts.join("\n  ")}`,
  }).catch(() => {});
  throw new Error(`No driver answered ${label}. Tried:\n  ${attempts.join("\n  ")}`);
}

/** Returns the DPI the device actually applied — it can clamp/round. */
export const setDpi = (info: HidInterfaceInfo, dpi: number, dpiY?: number): Promise<number> =>
  withClient(info, "setDpi", async (client) => {
    if (!client.setDpi) throw new Error("setDpi not supported");
    return (await client.setDpi(dpi, dpiY)) as number;
  });

export const setPollingRate = (info: HidInterfaceInfo, rate: number): Promise<number> =>
  withClient(info, "setPollingRate", async (client) => {
    if (!client.setPollingRate) throw new Error("setPollingRate not supported");
    return (await client.setPollingRate(rate)) as number;
  });

export const setLiftOffDistance = (info: HidInterfaceInfo, value: LiftOffDistance): Promise<LiftOffDistance> =>
  withClient(info, "setLiftOffDistance", async (client) => {
    if (!client.setLiftOffDistance) throw new Error("setLiftOffDistance not supported");
    return (await client.setLiftOffDistance(value)) as LiftOffDistance;
  });

export const setGamingSurfaceMode = (info: HidInterfaceInfo, mode: GamingSurfaceMode): Promise<GamingSurfaceMode> =>
  withClient(info, "setGamingSurfaceMode", async (client) => {
    if (!client.setGamingSurfaceMode) throw new Error("setGamingSurfaceMode not supported");
    return (await client.setGamingSurfaceMode(mode)) as GamingSurfaceMode;
  });

export const setMotionSync = (info: HidInterfaceInfo, enabled: boolean): Promise<boolean> =>
  withClient(info, "setMotionSync", async (client) => {
    if (!client.setMotionSync) throw new Error("setMotionSync not supported");
    return (await client.setMotionSync(enabled)) as boolean;
  });

export const setAngleSnapping = (info: HidInterfaceInfo, enabled: boolean): Promise<boolean> =>
  withClient(info, "setAngleSnapping", async (client) => {
    if (!client.setAngleSnapping) throw new Error("setAngleSnapping not supported");
    return (await client.setAngleSnapping(enabled)) as boolean;
  });

export const setRippleControl = (info: HidInterfaceInfo, enabled: boolean): Promise<boolean> =>
  withClient(info, "setRippleControl", async (client) => {
    if (!client.setRippleControl) throw new Error("setRippleControl not supported");
    return (await client.setRippleControl(enabled)) as boolean;
  });

export const setDebounceTime = (info: HidInterfaceInfo, milliseconds: number): Promise<number> =>
  withClient(info, "setDebounceTime", async (client) => {
    if (!client.setDebounceTime) throw new Error("setDebounceTime not supported");
    return (await client.setDebounceTime(milliseconds)) as number;
  });

/** Seconds-unit sleep timeout. Wallhack takes minutes; handled by `writeSleepTimeout`. */
export const setSleepTimeout = (info: HidInterfaceInfo, seconds: number): Promise<number> =>
  withClient(info, "setSleepTimeout", async (client) => {
    if (!client.setSleepTimeout) throw new Error("setSleepTimeout not supported");
    return (await client.setSleepTimeout(seconds)) as number;
  });

export const setPerformanceMode = (info: HidInterfaceInfo, enabled: boolean): Promise<boolean> =>
  withClient(info, "setPerformanceMode", async (client) => {
    if (!client.setPerformanceMode) throw new Error("setPerformanceMode not supported");
    return (await client.setPerformanceMode(enabled)) as boolean;
  });

export const setLighting = (info: HidInterfaceInfo, lighting: MouseLighting): Promise<MouseLighting> =>
  withClient(info, "setLighting", async (client) => {
    if (!client.setLighting) throw new Error("setLighting not supported");
    return (await client.setLighting(lighting)) as MouseLighting;
  });

export const setHyperMode = (info: HidInterfaceInfo, enabled: boolean): Promise<boolean> =>
  withClient(info, "setHyperMode", async (client) => {
    if (!client.setHyperMode) throw new Error("setHyperMode not supported");
    return (await client.setHyperMode(enabled)) as boolean;
  });

export const setSensorMode = (info: HidInterfaceInfo, mode: SensorMode): Promise<SensorMode> =>
  withClient(info, "setSensorMode", async (client) => {
    if (!client.setSensorMode) throw new Error("setSensorMode not supported");
    return (await client.setSensorMode(mode)) as SensorMode;
  });

export const setPerformanceDuration = (info: HidInterfaceInfo, seconds: number): Promise<number> =>
  withClient(info, "setPerformanceDuration", async (client) => {
    if (!client.setPerformanceDuration) throw new Error("setPerformanceDuration not supported");
    return (await client.setPerformanceDuration(seconds)) as number;
  });

export const setActiveDpiStage = (info: HidInterfaceInfo, stage: number): Promise<number> =>
  withClient(info, "setActiveDpiStage", async (client) => {
    if (!client.setActiveDpiStage) throw new Error("setActiveDpiStage not supported");
    return (await client.setActiveDpiStage(stage)) as number;
  });

export const setDpiStageValue = (info: HidInterfaceInfo, stage: number, dpi: number): Promise<unknown> =>
  withClient(info, "setDpiStageValue", async (client) => {
    if (!client.setDpiStageValue) throw new Error("setDpiStageValue not supported");
    return client.setDpiStageValue(stage, dpi);
  });

export const setDpiStageCount = (info: HidInterfaceInfo, count: number): Promise<number> =>
  withClient(info, "setDpiStageCount", async (client) => {
    if (!client.setDpiStageCount) throw new Error("setDpiStageCount not supported");
    return (await client.setDpiStageCount(count)) as number;
  });

export const setUsbSpeed = (info: HidInterfaceInfo, speed: UsbSpeed): Promise<UsbSpeed> =>
  withClient(info, "setUsbSpeed", async (client) => {
    if (!client.setUsbSpeed) throw new Error("setUsbSpeed not supported");
    return (await client.setUsbSpeed(speed)) as UsbSpeed;
  });

export const setPrimaryButton = (info: HidInterfaceInfo, button: PrimaryButton): Promise<PrimaryButton> =>
  withClient(info, "setPrimaryButton", async (client) => {
    if (!client.setPrimaryButton) throw new Error("setPrimaryButton not supported");
    return (await client.setPrimaryButton(button)) as PrimaryButton;
  });

export const setLowPowerThreshold = (info: HidInterfaceInfo, percent: number): Promise<number> =>
  withClient(info, "setLowPowerThreshold", async (client) => {
    if (!client.setLowPowerThreshold) throw new Error("setLowPowerThreshold not supported");
    return (await client.setLowPowerThreshold(percent)) as number;
  });

export const setWheelAcceleration = (info: HidInterfaceInfo, enabled: boolean): Promise<boolean> =>
  withClient(info, "setWheelAcceleration", async (client) => {
    if (!client.setWheelAcceleration) throw new Error("setWheelAcceleration not supported");
    return (await client.setWheelAcceleration(enabled)) as boolean;
  });

export const setAngleTuning = (info: HidInterfaceInfo, value: number): Promise<number> =>
  withClient(info, "setAngleTuning", async (client) => {
    if (!client.setAngleTuning) throw new Error("setAngleTuning not supported");
    return (await client.setAngleTuning(value)) as number;
  });

// ── brand-specific actions ───────────────────────────────────────────────
// Each is only writable by the brand's driver; `withClient` skips candidates
// whose class lacks the optional method and uses whichever one answers.

export const setSlamclickFilter = (info: HidInterfaceInfo, enabled: boolean): Promise<unknown> =>
  withClient(info, "setSlamclickFilter", async (client) => {
    if (!client.setSlamclickFilter) throw new Error("setSlamclickFilter not supported");
    return client.setSlamclickFilter(enabled);
  });

export const setMotionJitterFilter = (info: HidInterfaceInfo, enabled: boolean): Promise<unknown> =>
  withClient(info, "setMotionJitterFilter", async (client) => {
    if (!client.setMotionJitterFilter) throw new Error("setMotionJitterFilter not supported");
    return client.setMotionJitterFilter(enabled);
  });

export const setSpdtMode = (
  info: HidInterfaceInfo,
  button: "left" | "right",
  mode: "Off" | "GX Safe" | "GX Speed",
): Promise<unknown> =>
  withClient(info, "setSpdtMode", async (client) => {
    if (!client.setSpdtMode) throw new Error("setSpdtMode not supported");
    return client.setSpdtMode(button, mode);
  });

export const setCpiLevels = (info: HidInterfaceInfo, levels: number): Promise<unknown> =>
  withClient(info, "setCpiLevels", async (client) => {
    if (!client.setCpiLevels) throw new Error("setCpiLevels not supported");
    return client.setCpiLevels(levels);
  });

export const setCpiStage = (info: HidInterfaceInfo, level: number, x: number, y: number): Promise<unknown> =>
  withClient(info, "setCpiStage", async (client) => {
    if (!client.setCpiStage) throw new Error("setCpiStage not supported");
    return client.setCpiStage(level, x, y);
  });

export const setMulticlickFilter = (info: HidInterfaceInfo, button: string | number, value: number): Promise<unknown> =>
  withClient(info, "setMulticlickFilter", async (client) => {
    if (!client.setMulticlickFilter) throw new Error("setMulticlickFilter not supported");
    return client.setMulticlickFilter(button as never, value);
  });

export const setButtonMapping = (info: HidInterfaceInfo, button: string | number, action: string): Promise<unknown> =>
  withClient(info, "setButtonMapping", async (client) => {
    if (!client.setButtonMapping) throw new Error("setButtonMapping not supported");
    return client.setButtonMapping(button as never, action);
  });

/**
 * Scroll-up / scroll-down / sensitivity-button toggles — Razer gives them
 * their own command and deliberately keeps them out of
 * `RAZER_BUTTON_MAPPINGS`, so sending one through `setButtonMapping` always
 * threw ("… is not a button mapping this driver can send.") before any
 * traffic. Their values do come back in `status.razerButtonMappings`
 * (RazerHidClient.readButtonMappings reads them), which is why only the write
 * side needed routing.
 */
export const setToggleControl = (info: HidInterfaceInfo, control: string, label: string): Promise<unknown> =>
  withClient(info, "setToggleControl", async (client) => {
    if (!client.setToggleControl) throw new Error("setToggleControl not supported");
    return client.setToggleControl(control, label);
  });

export const setCustomPollingDivider = (info: HidInterfaceInfo, divider: number): Promise<unknown> =>
  withClient(info, "setCustomPollingDivider", async (client) => {
    if (!client.setCustomPollingDivider) throw new Error("setCustomPollingDivider not supported");
    return client.setCustomPollingDivider(divider);
  });

export const setNinjutsoSystemMode = (
  info: HidInterfaceInfo,
  mode: "High Speed" | "Competitive" | "Ultra",
): Promise<unknown> =>
  withClient(info, "setNinjutsoSystemMode", async (client) => {
    if (!client.setNinjutsoSystemMode) throw new Error("setNinjutsoSystemMode not supported");
    return client.setNinjutsoSystemMode(mode);
  });

export const setNinjutsoHyperClick = (info: HidInterfaceInfo, enabled: boolean): Promise<unknown> =>
  withClient(info, "setNinjutsoHyperClick", async (client) => {
    if (!client.setNinjutsoHyperClick) throw new Error("setNinjutsoHyperClick not supported");
    return client.setNinjutsoHyperClick(enabled);
  });

export const setNinjutsoOpticalEngine = (
  info: HidInterfaceInfo,
  mode: "Standard" | "Burst",
): Promise<unknown> =>
  withClient(info, "setNinjutsoOpticalEngine", async (client) => {
    if (!client.setNinjutsoOpticalEngine) throw new Error("setNinjutsoOpticalEngine not supported");
    return client.setNinjutsoOpticalEngine(mode);
  });

export const setNinjutsoSlamClick = (
  info: HidInterfaceInfo,
  level: "Low" | "Medium" | "High",
): Promise<unknown> =>
  withClient(info, "setNinjutsoSlamClick", async (client) => {
    if (!client.setNinjutsoSlamClick) throw new Error("setNinjutsoSlamClick not supported");
    return client.setNinjutsoSlamClick(level);
  });

export const setDongleLedMode = (info: HidInterfaceInfo, mode: number): Promise<unknown> =>
  withClient(info, "setDongleLedMode", async (client) => {
    if (!client.setDongleLedMode) throw new Error("setDongleLedMode not supported");
    return client.setDongleLedMode(mode);
  });

export const setTournamentScrollMode = (info: HidInterfaceInfo, mode: number): Promise<unknown> =>
  withClient(info, "setTournamentScrollMode", async (client) => {
    if (!client.setTournamentScrollMode) throw new Error("setTournamentScrollMode not supported");
    return client.setTournamentScrollMode(mode);
  });

export const setTournamentScrollTimeout = (info: HidInterfaceInfo, milliseconds: number): Promise<unknown> =>
  withClient(info, "setTournamentScrollTimeout", async (client) => {
    if (!client.setTournamentScrollTimeout) throw new Error("setTournamentScrollTimeout not supported");
    return client.setTournamentScrollTimeout(milliseconds);
  });

export const setDpiLighting = (
  info: HidInterfaceInfo,
  mode: number,
  brightness: number,
  speed: number,
): Promise<unknown> =>
  withClient(info, "setDpiLighting", async (client) => {
    if (!client.setDpiLighting) throw new Error("setDpiLighting not supported");
    return client.setDpiLighting(mode, brightness, speed);
  });

export const setProfile = (info: HidInterfaceInfo, profile: number): Promise<unknown> =>
  withClient(info, "setProfile", async (client) => {
    if (!client.setProfile) throw new Error("setProfile not supported");
    return client.setProfile(profile);
  });