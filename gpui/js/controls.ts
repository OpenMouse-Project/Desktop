// What the device page's Buttons, Lighting and Advanced tabs offer for a
// connected device, and how each control writes. A transliteration of
// src/components/Device{Buttons,Lighting,Advanced}Tab.tsx: the same brand
// gating on the same MouseStatus fields, the same write functions — but as
// plain data, so the Rust side can draw any brand without knowing it.

import { RAZER_BUTTON_CONTROLS, RAZER_BUTTON_CONTROL_LABEL, RAZER_BUTTON_MAPPINGS, RAZER_LOCKED_BUTTON_CONTROL, RAZER_TOGGLE_CONTROLS, RAZER_TOGGLE_CONTROL_INFO } from "@openmouse/protocol/razer";
import { EGG_BUTTON_NAMES, EGG_BUTTON_MAPPINGS } from "@openmouse/protocol/drivers/endgame/egg-op1-hid";
import { TEEVOLUTION_PERFORMANCE_TIME_LABELS } from "@openmouse/protocol/teevolution";
import * as w from "../../src/native-hid/write";
import { withLogitechClient } from "../../src/native-hid/logitech-actions";
import { callClientMethod } from "./generic-write";
import type { HidInterfaceInfo } from "../../src/native-hid/tauri-hid-device";
import type { DeviceCapabilities } from "../../src/native-hid/write";

type Option = { value: string | number; label: string };
type Control =
  | { id: string; label: string; description?: string; kind: "choice"; options: Option[]; current: string | number | null }
  | { id: string; label: string; description?: string; kind: "toggle"; current: boolean }
  | { id: string; label: string; description?: string; kind: "number"; min: number; max: number; step?: number; decimals?: number; unit?: string; zeroLabel?: string; current: number }
  | { id: string; label: string; description?: string; kind: "info"; current: string };
type Section = { title: string; controls: Control[] };

const opts = (values: readonly (string | number)[]): Option[] => values.map((v) => ({ value: v, label: String(v) }));

function razerAndEgg(brand: string, s: any): Section[] {
  const out: Section[] = [];
  const isRazer = brand === "Razer" && Boolean(s.razerButtonMappings);
  const isEgg = brand === "Endgame Gear" && Boolean(s.eggButtonMappings);

  if (isRazer) {
    out.push({
      title: "Razer — Buttons",
      controls: RAZER_BUTTON_CONTROLS.filter((c: string) => c !== RAZER_LOCKED_BUTTON_CONTROL).map((c: string) => ({
        id: `razer.button.${c}`,
        label: (RAZER_BUTTON_CONTROL_LABEL as any)[c],
        description: "Choose what this button sends.",
        kind: "choice" as const,
        options: opts(RAZER_BUTTON_MAPPINGS as readonly string[]),
        current: s.razerButtonMappings?.[c] ?? null,
      })),
    });
    // The Tauri tab edits these as free text; shown read-only here until the
    // app has a text field to do that with.
    out.push({
      title: "Razer — Scroll & toggles",
      controls: RAZER_TOGGLE_CONTROLS.map((c: string) => ({
        id: `razer.toggle.${c}`,
        label: (RAZER_TOGGLE_CONTROL_INFO as any)[c].label,
        description: (RAZER_TOGGLE_CONTROL_INFO as any)[c].enabledLabel,
        kind: "info" as const,
        current: String(s.razerButtonMappings?.[c] ?? "Disabled"),
      })),
    });
  }
  if (isEgg) {
    out.push({
      title: "Endgame Gear — Buttons",
      controls: (EGG_BUTTON_NAMES as readonly string[]).map((name, i) => ({
        id: `egg.button.${i}`,
        label: name,
        kind: "choice" as const,
        options: opts(EGG_BUTTON_MAPPINGS as readonly string[]),
        current: s.eggButtonMappings?.[i] ?? null,
      })),
    });
    if (Array.isArray(s.eggMulticlickFilters) && s.eggMulticlickFilters.length > 0) {
      out.push({
        title: "Endgame Gear — Debounce",
        controls: (EGG_BUTTON_NAMES as readonly string[]).slice(0, s.eggMulticlickFilters.length).map((name, i) => ({
          id: `egg.debounce.${i}`,
          label: name,
          kind: "choice" as const,
          options: [0, 1, 2, 4, 8, 12, 16].map((v) => ({ value: v, label: v === 0 ? "Off" : `${v} ms` })),
          current: s.eggMulticlickFilters?.[i] ?? 16,
        })),
      });
    }
  }
  return out;
}

/**
 * The shared remapper: a driver that reports `buttonMappings` (display label →
 * action label) with `buttonOptions` and implements `setButtonMapping(label,
 * action)`. Buttons it reports in an encoding it can't name (`fixedButtons`, e.g.
 * macros) are shown but not editable. Razer and Endgame Gear keep their own
 * sections above; they report different fields.
 */
function genericButtons(s: any): Section[] {
  if (!s.buttonMappings || typeof s.buttonMappings !== "object" || !Array.isArray(s.buttonOptions) || s.buttonOptions.length === 0) return [];
  const fixed: string[] = Array.isArray(s.fixedButtons) ? s.fixedButtons : [];
  const controls: Control[] = Object.entries(s.buttonMappings as Record<string, string>).map(([button, action]) =>
    fixed.includes(button)
      ? { id: `gen.fixed/${button}`, label: button, description: "Set up with a macro or custom binding — not editable here.", kind: "info" as const, current: String(action) }
      : { id: `gen.button/${button}`, label: button, kind: "choice" as const, options: opts(s.buttonOptions), current: action },
  );
  return controls.length ? [{ title: "Buttons", controls }] : [];
}

function advanced(s: any, brand: string): Section[] {
  const out: Section[] = [];
  const isEgg = typeof s.slamclickFilter === "boolean" || typeof s.motionJitterFilter === "boolean";
  const eggAny = isEgg || (Array.isArray(s.eggCpiStages) && s.eggCpiStages.length > 0);

  if (eggAny) {
    const c: Control[] = [];
    if (typeof s.slamclickFilter === "boolean") c.push({ id: "egg.slamclick", label: "Slamclick filter", kind: "toggle", current: s.slamclickFilter });
    if (typeof s.motionJitterFilter === "boolean") c.push({ id: "egg.jitter", label: "Motion-jitter filter", kind: "toggle", current: s.motionJitterFilter });
    const gx = ["Off", "GX Safe", "GX Speed"];
    if (s.leftSpdtMode != null || s.rightSpdtMode != null) {
      c.push({ id: "egg.spdt.left", label: "GX switch — left button", kind: "choice", options: opts(gx), current: s.leftSpdtMode ?? "Off" });
      if (s.rightSpdtMode != null) c.push({ id: "egg.spdt.right", label: "GX switch — right button", kind: "choice", options: opts(gx), current: s.rightSpdtMode });
    }
    if (Array.isArray(s.eggCpiStages) && s.eggCpiStages.length > 0) {
      c.push({ id: "egg.cpiLevels", label: "CPI levels", kind: "choice", options: opts([1, 2, 3, 4]), current: s.eggCpiLevels ?? 1 });
    }
    if (s.eggPollingDivider != null) {
      const d = s.eggPollingDivider;
      c.push({
        id: "egg.pollingDivider",
        label: "8K polling divider",
        description: `Result: ${Number.isInteger(d) && d > 0 ? `${(8000 / d).toLocaleString(undefined, { maximumFractionDigits: 2 })} Hz` : "—"}`,
        kind: "number",
        min: 1,
        max: 255,
        current: d,
      });
    }
    out.push({ title: "Endgame Gear", controls: c });
  }

  if (s.ninjutsoSystemMode || s.ninjutsoOpticalEngine || s.ninjutsoHyperClick != null || s.ninjutsoSlamClick) {
    const c: Control[] = [];
    if (s.ninjutsoSystemMode) c.push({ id: "nin.systemMode", label: "System mode", kind: "choice", options: opts(s.ninjutsoSystemModes ?? []), current: s.ninjutsoSystemMode });
    if (s.ninjutsoOpticalEngine) c.push({ id: "nin.engine", label: "Optical Engine", kind: "choice", options: opts(["Standard", "Burst"]), current: s.ninjutsoOpticalEngine });
    if (s.ninjutsoHyperClick != null) c.push({ id: "nin.hyperclick", label: "HyperClick", kind: "toggle", current: Boolean(s.ninjutsoHyperClick) });
    if (s.ninjutsoSlamClick) c.push({ id: "nin.slamclick", label: "Slam-Click", kind: "choice", options: opts(["Low", "Medium", "High"]), current: s.ninjutsoSlamClick });
    out.push({ title: "Ninjutso", controls: c });
  }

  if (s.finalmouseDongleLedMode != null) {
    out.push({
      title: "Finalmouse",
      controls: [
        { id: "fm.dongleLed", label: "Dongle LED", kind: "choice", options: [{ value: 0, label: "Off" }, { value: 1, label: "Battery indicator" }, { value: 2, label: "Solid white" }], current: s.finalmouseDongleLedMode ?? 0 },
        { id: "fm.scrollMode", label: "Tournament scroll", kind: "choice", options: [{ value: 0, label: "Off" }, { value: 1, label: "Scroll up" }, { value: 2, label: "Scroll down" }, { value: 3, label: "Both directions" }], current: s.finalmouseTournamentScrollMode ?? 0 },
        { id: "fm.scrollTimeout", label: "Passthrough window", kind: "choice", options: [{ value: 100, label: "100 ms" }, { value: 500, label: "500 ms" }, { value: 1000, label: "1 second" }, { value: 1500, label: "1.5 seconds" }], current: s.finalmouseTournamentScrollTimeoutMs ?? 100 },
      ],
    });
  }

  if (s.sensorMode != null || s.performanceDuration != null || s.dpiLedMode != null) {
    const c: Control[] = [];
    if (s.dpiLedMode != null) {
      const modes: number[] = s.ui?.dpiLighting?.modes ?? [0, 1, 2];
      c.push({ id: "tee.dpiLed", label: "DPI indicator effect", kind: "choice", options: modes.map((m) => ({ value: m, label: m === 0 ? "Off" : m === 1 ? "Steady" : "Breathing" })), current: s.dpiLedMode });
    }
    if (s.performanceDuration != null) {
      c.push({ id: "tee.duration", label: "Performance duration", kind: "choice", options: (TEEVOLUTION_PERFORMANCE_TIME_LABELS as readonly (readonly [number, string])[]).map(([value, label]) => ({ value, label })), current: s.performanceDuration });
    }
    out.push({ title: "Teevolution", controls: c });
  }

  // The 2.4 GHz dongle / receiver: LED behaviour and, where a driver reports it,
  // the receiver's own state. Finalmouse's dongle LED has its own section above.
  const dongle: Control[] = [];
  if (typeof s.dongleLedEnabled === "boolean") {
    dongle.push({ id: "dongle.led", label: "Dongle LED", description: "The receiver's status light.", kind: "toggle", current: s.dongleLedEnabled });
  }
  if (typeof s.incottReceiverLedMode === "number") {
    dongle.push({
      id: "dongle.incott",
      label: "Receiver LED",
      description: "What the 2.4 GHz receiver's light shows.",
      kind: "choice",
      options: [{ value: 0, label: "Connection & polling rate" }, { value: 1, label: "Battery status" }, { value: 2, label: "Battery warning" }],
      current: s.incottReceiverLedMode,
    });
  }
  // Write-only: null until something has been written, but the control is still real.
  if (s.atkDongleLight !== undefined) {
    dongle.push({
      id: "dongle.atk",
      label: "Dongle light",
      description: "Write-only on this mouse: the value shown is the last one set, not read back from the dongle.",
      kind: "choice",
      options: [{ value: 0, label: "Off" }, { value: 1, label: "Polling rate" }, { value: 2, label: "Battery" }, { value: 3, label: "Low battery" }],
      current: s.atkDongleLight ?? null,
    });
  }
  if (s.atkReceiver && typeof s.atkReceiver === "object") {
    const r = s.atkReceiver;
    dongle.push({ id: "dongle.info.state", label: "Receiver", kind: "info", current: r.online ? "Connected" : "Not connected" });
    if (r.rfId) dongle.push({ id: "dongle.info.rf", label: "Receiver ID", kind: "info", current: String(r.rfId) });
    if (r.pairingSecondsRemaining) dongle.push({ id: "dongle.info.pairing", label: "Pairing", kind: "info", current: `Open for ${r.pairingSecondsRemaining} s` });
  }
  if (dongle.length) out.push({ title: "Dongle", controls: dongle });

  // A driver that reports a profile count gets a selector with exactly that many
  // (and its own profile names); Pulsar Pro, which doesn't, keeps its fixed 1-6.
  const hasProfileCount = typeof s.profileCount === "number" && s.profileCount > 1;
  if (hasProfileCount && typeof s.activeProfile === "number") {
    const names: string[] = Array.isArray(s.profileNames) ? s.profileNames : [];
    out.push({
      title: "Profiles",
      controls: [{
        id: "gen.profile",
        label: "Active profile",
        description: "Switches the mouse's stored profile.",
        kind: "choice",
        options: Array.from({ length: s.profileCount }, (_, i) => ({ value: i + 1, label: names[i] || `Profile ${i + 1}` })),
        current: s.activeProfile,
      }],
    });
  }
  if (s.wheelAcceleration != null) {
    out.push({ title: "Scroll wheel", controls: [{ id: "pulsar.wheel", label: "Wheel acceleration", kind: "toggle", current: Boolean(s.wheelAcceleration) }] });
  }
  // Pulsar's own fixed 1-6 profile selector. Only for Pulsar: other brands (Logitech
  // reports its running onboard profile as `activeProfile` too) must not get it.
  if (brand === "Pulsar" && !hasProfileCount && typeof s.activeProfile === "number" && s.activeProfile > 0) {
    out.push({ title: "Pulsar Pro", controls: [{ id: "pulsar.profile", label: "Active profile", kind: "choice", options: opts([1, 2, 3, 4, 5, 6]), current: s.activeProfile }] });
  }
  return out;
}

/** Brand-specific extras that were reported by drivers but had no control yet. */
function extras(s: any): Section[] {
  const out: Section[] = [];

  if (s.finalmouseIsSlx === true) {
    const mode = [{ value: 0, label: "Mechanical" }, { value: 1, label: "TMR analog" }];
    const rel = [{ value: 0, label: "Normal" }, { value: 1, label: "Early" }, { value: 2, label: "Late" }];
    // Edit both switches as one, or each on its own. Starts linked when they already match.
    const keys = ["ClickMode", "ClickRelease", "TmrThr", "TmrHyst"];
    if (s.slxLinked === undefined) s.slxLinked = keys.every((k) => s[`finalmouse${k}L`] === s[`finalmouse${k}R`]);
    const linked: boolean = s.slxLinked;
    out.push({ title: "Starlight X", controls: [{ id: "slx.link", label: "Link left & right", description: "Change both buttons together.", kind: "toggle", current: linked }] });
    for (const side of linked ? (["L"] as const) : (["L", "R"] as const)) {
      const c: Control[] = [];
      const name = linked ? "Both buttons" : side === "L" ? "Left button" : "Right button";
      const m = s[`finalmouseClickMode${side}`];
      if (m != null) c.push({ id: `slx.mode${side}`, label: "Switch", kind: "choice", options: mode, current: m });
      const r = s[`finalmouseClickRelease${side}`];
      if (r != null) c.push({ id: `slx.rel${side}`, label: "Release point", kind: "choice", options: rel, current: r });
      // Analog tuning only means something while the switch is in TMR mode.
      if (m === 1) {
        const thr = s[`finalmouseTmrThr${side}`];
        if (thr != null) c.push({ id: `slx.thr${side}`, label: "Actuation point", kind: "number", min: 0.01, max: 0.4, step: 0.01, decimals: 2, unit: "mm", current: thr / 100 });
        const rt = s[`finalmouseTmrHyst${side}`];
        if (rt != null) c.push({ id: `slx.rt${side}`, label: "Rapid trigger", kind: "number", min: 150, max: 250, step: 5, unit: "µm", current: rt });
        const msp = s[`finalmouseTmrMsp${side}`];
        if (msp != null && !linked) c.push({ id: `slx.info.msp${side}`, label: "Calibration reference", kind: "info", current: `${msp} µm` });
      }
      if (c.length) out.push({ title: `Starlight X · ${name}`, controls: c });
    }
    if (s.finalmousePawLodMm != null) out.push({ title: "Starlight X · Lift-off", controls: [{ id: "slx.lod", label: "PAW lift-off", kind: "number", min: 0.7, max: 1.7, step: 0.1, decimals: 1, unit: "mm", current: s.finalmousePawLodMm }] });
    if (typeof s.finalmouseProfileCount === "number" && s.finalmouseProfileCount > 1 && typeof s.finalmouseProfileActive === "number") {
      const names = s.finalmouseProfileNames ?? {};
      const mask = Number(s.finalmouseProfileEnabledMask ?? -1);
      const options = Array.from({ length: s.finalmouseProfileCount }, (_, i) => ({ value: i, label: names[i] || `Profile ${i + 1}` })).filter((o) => mask === -1 || (mask >> Number(o.value)) & 1 || o.value === s.finalmouseProfileActive);
      out.push({ title: "Profiles", controls: [{ id: "slx.profile", label: "Active profile", kind: "choice", options, current: s.finalmouseProfileActive }] });
    }
  }

  const atk: Control[] = [];
  if (s.atkSensorMode != null) atk.push({ id: "atk.sensorMode", label: "Sensor mode", kind: "choice", options: [{ value: 0, label: "Basic" }, { value: 1, label: "Shard" }, { value: 2, label: "Shard MAX" }], current: s.atkSensorMode });
  if (s.atkAntiMistouchMs != null) atk.push({ id: "atk.antiMistouch", label: "Anti-mistouch", description: "Ignored-click window in ms; 0 is off.", kind: "number", min: 0, max: 2550, step: 10, current: s.atkAntiMistouchMs });
  if (typeof s.atkProfileCount === "number" && s.atkProfileCount > 1 && typeof s.activeProfile === "number") {
    atk.push({ id: "atk.profile", label: "Active profile", kind: "choice", options: Array.from({ length: s.atkProfileCount }, (_, i) => ({ value: i + 1, label: `Profile ${i + 1}` })), current: s.activeProfile });
  }
  if (Array.isArray(s.atkButtonMappings)) {
    for (const b of s.atkButtonMappings) atk.push({ id: `atk.info.${b.id}`, label: `${b.name} button`, kind: "info", current: String(b.action) });
  }
  if (atk.length) out.push({ title: "ATTACK SHARK", controls: atk });

  if (s.incottFireKeyTimes != null) {
    out.push({
      title: "Fire key",
      controls: [
        { id: "incott.fireTimes", label: "Clicks per press", kind: "choice", options: opts([1, 2, 3]), current: s.incottFireKeyTimes },
        { id: "incott.fireInterval", label: "Interval", description: "Milliseconds between clicks.", kind: "number", min: 0, max: 255, step: 5, current: s.incottFireKeyIntervalMs ?? 0 },
      ],
    });
  }

  if (s.eggSupportsGlassMode && s.eggGlassMode != null) {
    out.push({ title: "Endgame Gear", controls: [{ id: "egg.glass", label: "Glass mode", description: "Tracking tuned for glass surfaces.", kind: "toggle", current: Boolean(s.eggGlassMode) }] });
  }

  if (s.asusOmni && typeof s.asusOmni === "object") {
    const o = s.asusOmni;
    const c: Control[] = [];
    if (o.firmware) c.push({ id: "omni.info.fw", label: "Receiver firmware", kind: "info", current: String(o.firmware) });
    for (const [i, d] of (o.devices ?? []).entries()) c.push({ id: `omni.info.dev${i}`, label: d.name || `Device ${i + 1}`, kind: "info", current: String(d.kind ?? "") });
    c.push({ id: "omni.pair", label: "Pairing", description: "Open the receiver for a new device.", kind: "choice", options: [{ value: 1, label: "Start pairing" }, { value: 0, label: "Stop" }], current: null });
    out.push({ title: "Omni receiver", controls: c });
  }

  const lf = s.lunafury;
  if (lf && typeof lf === "object") {
    const c: Control[] = [];
    if (lf.lightningMode != null) c.push({ id: "lf.lightning", label: "Lightning trigger", kind: "choice", options: opts([0, 1, 2]), current: lf.lightningMode });
    for (const b of ["left", "right", "middle"]) {
      const v = lf[`${b}DebounceMs`];
      if (v != null) c.push({ id: `lf.debounce.${b}`, label: `${b[0].toUpperCase()}${b.slice(1)} button latency`, description: "Milliseconds.", kind: "number", min: 0, max: 20, step: 1, current: v });
    }
    if (lf.wheelGuard) {
      c.push({ id: "lf.guard", label: "Wheel guard", kind: "toggle", current: Boolean(lf.wheelGuard.enabled) });
      c.push({ id: "lf.guardMs", label: "Wheel guard window", description: "Milliseconds.", kind: "number", min: 0, max: 500, step: 10, current: lf.wheelGuard.windowMs ?? 0 });
    }
    if (lf.bottomButtonMode != null) c.push({ id: "lf.bottom", label: "Bottom button", kind: "choice", options: opts([1, 2]), current: lf.bottomButtonMode });
    if (lf.receiverLightMode != null) c.push({ id: "lf.receiverLight", label: "Receiver light", kind: "choice", options: opts([0, 2, 6, 11]), current: lf.receiverLightMode });
    if (lf.buttonPollingRateHz != null) c.push({ id: "lf.buttonRate", label: "Button polling rate", kind: "choice", options: opts([...(lf.supportedButtonPollingRates ?? [lf.buttonPollingRateHz])]), current: lf.buttonPollingRateHz });
    if (lf.separateDpiAxes != null) c.push({ id: "lf.separateAxes", label: "Separate DPI axes", kind: "toggle", current: Boolean(lf.separateDpiAxes) });
    if (c.length) out.push({ title: "LunaFury", controls: c });
  }
  return out;
}

/** Logitech PRO X 3 Superstrike: hall-effect (HITS) tuning per primary button. */
function hits(s: any, brand: string): Section[] {
  const t = s.analogButtonTuning;
  if (brand !== "Logitech" || !t || !Array.isArray(t.buttons) || t.buttons.length === 0) return [];
  const same = (k: string) => t.buttons.every((b: any) => b[k] === t.buttons[0][k]);
  if (s.hitsLinked === undefined) s.hitsLinked = t.buttons.length > 1 && ["actuation", "rapidTrigger", "haptics", "rapidTriggerEnabled"].every(same);
  const linked: boolean = s.hitsLinked && t.buttons.length > 1;
  const out: Section[] = [];
  if (t.buttons.length > 1) {
    out.push({ title: "Hall-effect buttons", controls: [{ id: "hits.link", label: "Link left & right", description: "Change both buttons together.", kind: "toggle", current: linked }] });
  }
  const names = ["Left button", "Right button"];
  for (const i of linked ? [0] : t.buttons.keys()) {
    const b = t.buttons[i];
    const c: Control[] = [
      { id: `hits.act.${i}`, label: "Actuation point", description: "Higher means a deeper press.", kind: "number", min: 1, max: t.maxActuation, step: 1, current: b.actuation },
      // One slider: 0 is off, 1 and up is the sensitivity.
      { id: `hits.rt.${i}`, label: "Rapid trigger", description: "How far the button must lift to reset. Off at 0.", kind: "number", min: 0, max: t.maxRapidTrigger, step: 1, zeroLabel: "Off", current: b.rapidTriggerEnabled ? b.rapidTrigger : 0 },
    ];
    if (t.maxHaptics > 0) c.push({ id: `hits.hap.${i}`, label: "Click haptics", description: "Strength of this button's click feedback.", kind: "number", min: 0, max: t.maxHaptics, step: 1, current: b.haptics });
    out.push({ title: linked ? "Hall-effect · Both buttons" : `Hall-effect · ${names[i] ?? `Button ${i + 1}`}`, controls: c });
  }
  return out;
}

/**
 * Settings a driver reports under a shared status field and sets with a method of
 * the same name (`wheelMode` / `setWheelMode`, and so on). Each is offered when the
 * device reports the field, and written through whichever driver answered it.
 * Grouped by what a person would call them; all apply immediately.
 */
type GenericSpec = {
  field: string;
  method: string;
  section: string;
  label: string;
  description?: string;
  /** toggle / choice (with `options`) / number (with `min`/`max`/`step`). */
  kind: "toggle" | "choice" | "number";
  options?: readonly (string | number)[];
  min?: number;
  max?: number;
  step?: number;
  /** Extra condition beyond the field being reported. */
  when?: (status: any) => boolean;
};

const GENERIC: GenericSpec[] = [
  // Scroll wheel (Logitech, Ajazz, Motospeed, K'snake)
  { field: "wheelMode", method: "setWheelMode", section: "Scroll wheel", label: "Wheel mode", description: "Free-spinning or ratcheted, as the button behind the wheel toggles.", kind: "choice", options: ["Freespin", "Ratchet"] },
  { field: "smartShiftThreshold", method: "setSmartShiftThreshold", section: "Scroll wheel", label: "SmartShift sensitivity", description: "How gentle a flick releases the ratchet. 255 turns SmartShift off.", kind: "number", min: 1, max: 255, step: 5 },
  { field: "hiResScroll", method: "setHiResScroll", section: "Scroll wheel", label: "High-resolution scrolling", kind: "toggle" },
  { field: "invertScroll", method: "setInvertScroll", section: "Scroll wheel", label: "Invert scroll direction", kind: "toggle", when: (s) => s.supportsInvertScroll !== false },
  { field: "thumbWheelInverted", method: "setThumbWheelInverted", section: "Scroll wheel", label: "Invert thumb wheel", kind: "toggle", when: (s) => s.supportsThumbWheelInvert !== false },
  { field: "scrollDirection", method: "setScrollDirection", section: "Scroll wheel", label: "Scroll direction", kind: "choice", options: ["Forward", "Reverse"] },
  // Switches and haptics
  { field: "lightforceSwitchMode", method: "setLightforceSwitchMode", section: "Switches", label: "Lightforce switches", description: "Hybrid uses an optical and a mechanical stage; Optical is purely optical.", kind: "choice", options: ["Hybrid", "Optical"] },
  { field: "hapticEnabled", method: "setHapticEnabled", section: "Haptics", label: "Haptic feedback", kind: "toggle" },
  { field: "hapticIntensity", method: "setHapticIntensity", section: "Haptics", label: "Haptic strength", kind: "number", min: 0, max: 100, step: 5 },
  { field: "hapticBatterySaving", method: "setHapticBatterySaving", section: "Haptics", label: "Haptics battery saving", kind: "toggle" },
  // Sensor modes
  { field: "hyperMode", method: "setHyperMode", section: "Sensor", label: "Hyper mode", description: "Runs the sensor at its highest performance setting.", kind: "toggle" },
  { field: "turboMode", method: "setTurboMode", section: "Sensor", label: "Turbo mode", description: "Pins the sensor to its highest frame rate.", kind: "toggle" },
  { field: "sensorScanningMode", method: "setSensorScanningMode", section: "Sensor", label: "Sensor scanning", description: "HIGH is the default frame rate; ACCEL pins the sensor to high performance at a battery cost.", kind: "choice", options: ["HIGH", "ACCEL"] },
  { field: "dynamicSensitivityEnabled", method: "setDynamicSensitivityEnabled", section: "Sensor", label: "DPI acceleration", description: "Needs mouse firmware 57 or later.", kind: "toggle" },
  { field: "dynamicSensitivityMode", method: "setDynamicSensitivityMode", section: "Sensor", label: "Acceleration curve", kind: "choice", options: ["classic", "natural", "jump", "custom"] },
  { field: "buttonCombination", method: "setButtonCombination", section: "Sensor", label: "Button combinations", description: "Lets button chords change mouse settings without the driver.", kind: "toggle" },
  { field: "dpiLedSleepTimeout", method: "setDpiLedSleepTimeout", section: "Sensor", label: "DPI light sleep", description: "Seconds before the DPI indicator turns off, separate from mouse sleep.", kind: "number", min: 0, max: 600, step: 5 },
  // Wireless
  { field: "longRangeMode", method: "setLongRangeMode", section: "Wireless", label: "Long-range mode", description: "Trades battery for link range on receivers that offer it.", kind: "toggle" },
];

const titleCase = (value: string) => value.charAt(0).toUpperCase() + value.slice(1);

function genericFeatures(status: any): Section[] {
  const sections = new Map<string, Control[]>();
  for (const spec of GENERIC) {
    const value = status[spec.field];
    if (value === undefined || value === null) continue;
    if (spec.when && !spec.when(status)) continue;
    const id = `gen.set/${spec.field}/${spec.method}`;
    let control: Control | null = null;
    if (spec.kind === "toggle" && typeof value === "boolean") {
      control = { id, label: spec.label, description: spec.description, kind: "toggle", current: value };
    } else if (spec.kind === "choice" && spec.options) {
      control = {
        id, label: spec.label, description: spec.description, kind: "choice",
        options: spec.options.map((o) => ({ value: o, label: typeof o === "string" ? titleCase(o) : String(o) })),
        current: value,
      };
    } else if (spec.kind === "number" && typeof value === "number") {
      control = { id, label: spec.label, description: spec.description, kind: "number", min: spec.min ?? 0, max: spec.max ?? 100, step: spec.step, current: value };
    }
    if (control) sections.set(spec.section, [...(sections.get(spec.section) ?? []), control]);
  }
  return [...sections.entries()].map(([title, controls]) => ({ title, controls }));
}

/** Read-only facts a device reports about itself: identity, link and Easy-Switch state. */
function deviceInfo(status: any): Section[] {
  const c: Control[] = [];
  const info = (id: string, label: string, current: string | number | null | undefined, unit = "") => {
    if (current !== undefined && current !== null && current !== "") c.push({ id: `info.${id}`, label, kind: "info", current: `${current}${unit}` });
  };
  info("unit", "Unit ID", status.unitId);
  info("model", "Model ID", status.modelId);
  if (status.transportIds && typeof status.transportIds === "object") {
    info("transports", "Transports", Object.entries(status.transportIds as Record<string, string>).map(([k, v]) => `${k} ${v}`).join(" · "));
  }
  info("connection", "Connection", status.connectionDetail);
  info("signal", "Signal strength", status.signalStrength, "%");
  info("voltage", "Battery voltage", status.batteryVoltageMv, " mV");
  info("name", "Device name", status.friendlyName);
  if (typeof status.hostCount === "number" && status.hostCount > 0) {
    const paired = Array.isArray(status.hostSlotsPaired) ? status.hostSlotsPaired.filter(Boolean).length : null;
    const slot = typeof status.currentHost === "number" ? `slot ${status.currentHost + 1} of ${status.hostCount}` : `${status.hostCount} slots`;
    info("hosts", "Easy-Switch", paired === null ? slot : `${slot} · ${paired} paired`);
  }
  info("nape", "Layer", typeof status.napeLayer === "number" && status.napeLayerCount ? `${status.napeLayer + 1} of ${status.napeLayerCount}` : null);
  return c.length ? [{ title: "Device info", controls: c }] : [];
}

/** "Never", "45 s", "5 min", "2 hr" for a sleep timeout in seconds. */
function sleepLabel(seconds: number): string {
  if (seconds === 0) return "Never";
  if (seconds >= 3600) return `${Math.round(seconds / 3600)} hr`;
  if (seconds >= 60) return `${Math.round(seconds / 60)} min`;
  return `${seconds} s`;
}

/** A driver's option list with the device's own current value kept in it, so a
 *  mouse sitting on something off-list still shows the truth. */
function withCurrent(steps: readonly number[], current: number | null | undefined): number[] {
  return current != null && !steps.includes(current) ? [...steps, current].sort((a, b) => a - b) : [...steps];
}

/**
 * The Performance tab's controls beyond DPI / polling / lift-off / surface (those
 * have dedicated UI): sleep, the sensor-processing switches, on-device DPI stages,
 * debounce, low-power threshold, USB speed and primary button. Every control is
 * gated exactly as src/components/DevicePerformanceTab.tsx gates it — a status
 * field the driver reports, minus whatever the driver's own `ui` hints hide — and
 * all of them apply immediately, as they do there.
 */
function performanceExtras(status: any, caps: DeviceCapabilities): Section[] {
  const ui = status.ui ?? {};
  const c: Control[] = [];

  const hasSleep = typeof status.sleepTimeout === "number" && !ui.hideSleepCard && caps.sleepTimeouts !== null;
  if (hasSleep) {
    c.push({
      id: "perf.sleep",
      label: "Sleep timeout",
      description: "How long the mouse stays awake when idle before sleeping.",
      kind: "choice",
      options: withCurrent(caps.sleepTimeouts ?? [], status.sleepTimeout).map((v) => ({ value: v, label: sleepLabel(v) })),
      current: status.sleepTimeout,
    });
  }

  const toggles: { key: string; label: string; description: string; hidden: boolean }[] = [
    { key: "motionSync", label: "Motion sync", description: "Synchronizes X/Y sensor sampling for smoother diagonals.", hidden: !!ui.hideMotionSync },
    { key: "angleSnapping", label: "Angle snapping", description: "Rounds movement to straight lines and diagonals for a steadier cursor.", hidden: !!ui.hideAngleSnapping },
    { key: "rippleControl", label: "Ripple control", description: "Filters sensor ripple at high polling rates for more stable tracking.", hidden: !!ui.hideRippleControl },
    { key: "performanceMode", label: "Performance / hyper mode", description: "Runs the sensor at its full performance setting.", hidden: false },
  ];
  if (!ui.hideProcessingCard) {
    for (const t of toggles) {
      if (!t.hidden && typeof status[t.key] === "boolean") {
        c.push({ id: `perf.${t.key}`, label: t.label, description: t.description, kind: "toggle", current: status[t.key] });
      }
    }
  }

  if (Array.isArray(status.dpiStages) && status.dpiStages.length > 1) {
    c.push({
      id: "perf.dpiStage",
      label: "DPI stages",
      description: "On-device sensitivity stages; pick the active one.",
      kind: "choice",
      options: status.dpiStages.map((_: unknown, i: number) => ({ value: i, label: `Stage ${i + 1}` })),
      current: status.activeDpiStage ?? null,
    });
  }

  if (typeof status.debounceMs === "number") {
    const values = caps.debounceOptions ?? (caps.debounceMaxMs !== null ? Array.from({ length: caps.debounceMaxMs + 1 }, (_, ms) => ms) : null);
    if (values && values.length) {
      c.push({
        id: "perf.debounce",
        label: "Debounce",
        description: `Filters out physical switch bounce (0–${values[values.length - 1]} ms). Higher is steadier, slightly more click lag.`,
        kind: "choice",
        options: values.map((ms) => ({ value: ms, label: `${ms} ms` })),
        current: status.debounceMs,
      });
    }
  }

  if (typeof status.lowBatteryWarning === "number") {
    c.push({
      id: "perf.lowPower",
      label: "Low power mode",
      description: "Slows the mouse down to save battery below this level.",
      kind: "choice",
      options: withCurrent(caps.lowPowerThresholds ?? [5, 10, 15, 20, 25, 30], status.lowBatteryWarning).map((p) => ({ value: p, label: `${p}%` })),
      current: status.lowBatteryWarning,
    });
  }

  if (Array.isArray(status.powerModes) && status.powerModes.length > 0 && typeof status.powerMode === "string") {
    c.push({
      id: "gen.powerMode",
      label: "Power mode",
      description: "Trades battery life against performance.",
      kind: "choice",
      options: opts(status.powerModes),
      current: status.powerMode,
    });
  }

  if (status.usbSpeed != null) {
    c.push({
      id: "perf.usbSpeed",
      label: "USB speed",
      description: "Full pins polling to 1000 Hz; High enables faster rates.",
      kind: "choice",
      options: opts(["Full", "High"]),
      current: status.usbSpeed,
    });
  }

  if (status.primaryButton != null) {
    c.push({
      id: "perf.primaryButton",
      label: "Primary button",
      description: "Which button acts as the primary click.",
      kind: "choice",
      options: opts(["Left", "Right"]),
      current: status.primaryButton,
    });
  }

  const out: Section[] = [];
  // Starlight X has no selectable polling rate; say why instead of leaving a gap.
  if (status.finalmouseIsSlx === true) {
    out.push({
      title: "Polling rate",
      controls: [{
        id: "slx.info.perfectPolling",
        label: "Perfect Polling",
        description: "This mouse runs a fixed-rate pipeline that keeps reports perfectly timed, so the polling rate is managed for you and can't be changed.",
        kind: "info",
        current: "Always on",
      }],
    });
  }
  if (c.length) out.push({ title: "More settings", controls: c });
  return out;
}

export function describeControls(brand: string, status: any, caps: DeviceCapabilities) {
  return {
    buttons: [...razerAndEgg(brand, status), ...genericButtons(status)],
    advanced: [...advanced(status, brand), ...hits(status, brand), ...extras(status), ...genericFeatures(status), ...deviceInfo(status)],
    performance: performanceExtras(status, caps),
    lighting: status.lighting ?? null,
  };
}

/** Writes HITS tuning live for the affected buttons, then saves both into the running profile. */
async function writeHits(info: HidInterfaceInfo, status: any, id: string, value: any) {
  const t = status.analogButtonTuning;
  const [, field, idx] = id.split(".");
  if (field === "link") {
    status.hitsLinked = Boolean(value);
    if (!status.hitsLinked) return;
    // Linking makes the right button match the left one.
    for (let i = 1; i < t.buttons.length; i += 1) t.buttons[i] = { ...t.buttons[0] };
    return applyHits(info, t, t.buttons.keys());
  }
  const i = Number(idx);
  const touched = status.hitsLinked ? [...t.buttons.keys()] : [i];
  for (const n of touched) {
    const b = t.buttons[n];
    if (field === "act") b.actuation = Number(value);
    else if (field === "hap") b.haptics = Number(value);
    else if (field === "rt") {
      // 0 switches rapid trigger off but keeps the stored sensitivity.
      b.rapidTriggerEnabled = Number(value) > 0;
      if (Number(value) > 0) b.rapidTrigger = Number(value);
    } else throw new Error(`Unknown control ${id}`);
  }
  return applyHits(info, t, touched);
}

async function applyHits(info: HidInterfaceInfo, t: any, buttons: Iterable<number>) {
  if (info.key.startsWith("mock:")) return;
  const touched = [...buttons];
  await withLogitechClient(info, "set hall-effect buttons", async (client: any) => {
    for (const n of touched) await client.setAnalogButtonTuning(n, t.buttons[n]);
    // The live write is lost on a power cycle; keep the stored profile in step.
    await client.persistAnalogButtonTuning(t.buttons.map((b: any, button: number) => ({ button, ...b })));
  });
}

/** Writes one control by id (the dispatch for everything `describeControls` emits). */
export async function writeControl(info: HidInterfaceInfo, status: any, id: string, value: any): Promise<unknown> {
  // The shared remapper: the button's display label is the part after the slash.
  if (id.startsWith("gen.button/")) return w.setButtonMapping(info, id.slice("gen.button/".length), value);
  if (id.startsWith("hits.")) return writeHits(info, status, id, value);
  // Table-driven generic setting: `gen.set/<field>/<method>`.
  if (id.startsWith("gen.set/")) return callClientMethod(info, id.split("/")[2], value);
  const [family, kind, rest] = id.split(".");
  // Logitech button remap: `logi.button.<layer>.<n>` edits the running onboard
  // profile. The library checks the profile's CRC and format first, skips the
  // flash write when nothing changed, reads the sector back, and restores the
  // old one if that fails.
  if (family === "logi" && kind === "button") {
    const [, , layer, n] = id.split(".");
    if (info.key.startsWith("mock:")) return;
    return withLogitechClient(info, "set button", (client: any) =>
      client.writeActiveProfile({
        buttonAssignments: [{ layer: layer === "g-shift" ? "g-shift" : "primary", button: Number(n), binding: { kind: "action", action: value } }],
      }),
    );
  }
  switch (id) {
    case "slx.link": {
      status.slxLinked = Boolean(value);
      if (status.slxLinked) {
        // Linking makes the right button match the left one.
        const l = (k: string) => status[`finalmouse${k}L`];
        await callClientMethod(info, "setClickMode", l("ClickMode"), l("ClickMode"), l("ClickRelease"), l("ClickRelease"));
        if (l("TmrThr") != null) await callClientMethod(info, "setTmrActuation", l("TmrThr") / 100, l("TmrThr") / 100, l("TmrHyst"), l("TmrHyst"));
        for (const k of ["ClickMode", "ClickRelease", "TmrThr", "TmrHyst", "TmrMsp"]) status[`finalmouse${k}R`] = status[`finalmouse${k}L`];
      }
      return;
    }
    case "slx.modeL": case "slx.modeR": case "slx.relL": case "slx.relR": {
      const m = { L: status.finalmouseClickModeL ?? 0, R: status.finalmouseClickModeR ?? 0 };
      const r = { L: status.finalmouseClickReleaseL ?? 0, R: status.finalmouseClickReleaseR ?? 0 };
      const side = id.endsWith("L") ? "L" : "R";
      const target = id.startsWith("slx.mode") ? m : r;
      target[side] = Number(value);
      if (status.slxLinked) target.L = target.R = Number(value);
      await callClientMethod(info, "setClickMode", m.L, m.R, r.L, r.R);
      status.finalmouseClickModeL = m.L; status.finalmouseClickModeR = m.R;
      status.finalmouseClickReleaseL = r.L; status.finalmouseClickReleaseR = r.R;
      return;
    }
    case "slx.thrL": case "slx.thrR": case "slx.rtL": case "slx.rtR": {
      const t = { L: (status.finalmouseTmrThrL ?? 20) / 100, R: (status.finalmouseTmrThrR ?? 20) / 100 };
      const h = { L: status.finalmouseTmrHystL ?? 150, R: status.finalmouseTmrHystR ?? 150 };
      const side = id.endsWith("L") ? "L" : "R";
      const target = id.startsWith("slx.thr") ? t : h;
      target[side] = id.startsWith("slx.thr") ? Math.round(Number(value) * 100) / 100 : Number(value);
      if (status.slxLinked) target.L = target.R = target[side];
      await callClientMethod(info, "setTmrActuation", t.L, t.R, h.L, h.R);
      status.finalmouseTmrThrL = Math.round(t.L * 100); status.finalmouseTmrThrR = Math.round(t.R * 100);
      status.finalmouseTmrHystL = h.L; status.finalmouseTmrHystR = h.R;
      return;
    }
    case "slx.lod": await callClientMethod(info, "setPawLodMm", Math.round(Number(value) * 10) / 10); status.finalmousePawLodMm = Number(value); return;
    case "slx.profile": await callClientMethod(info, "setActiveProfile", Number(value)); status.finalmouseProfileActive = Number(value); return;
    case "atk.sensorMode": await callClientMethod(info, "setAtkSensorMode", Number(value)); status.atkSensorMode = Number(value); return;
    case "atk.antiMistouch": await callClientMethod(info, "setAntiMistouchMs", Number(value)); status.atkAntiMistouchMs = Number(value); return;
    case "atk.profile": await callClientMethod(info, "setR1ActiveProfile", Number(value)); status.activeProfile = Number(value); return;
    case "incott.fireTimes": case "incott.fireInterval": {
      const times = id === "incott.fireTimes" ? Number(value) : status.incottFireKeyTimes ?? 1;
      const ms = id === "incott.fireInterval" ? Number(value) : status.incottFireKeyIntervalMs ?? 0;
      await callClientMethod(info, "setFireKey", times, ms);
      status.incottFireKeyTimes = times; status.incottFireKeyIntervalMs = ms;
      return;
    }
    case "egg.glass": await callClientMethod(info, "setGlassMode", Boolean(value)); status.eggGlassMode = Boolean(value); return;
    case "omni.pair": return callClientMethod(info, "setOmniPairingMode", Number(value) === 1);
    case "lf.lightning": return callClientMethod(info, "setLunaFuryLightningMode", Number(value));
    case "lf.bottom": return callClientMethod(info, "setLunaFuryBottomButtonMode", Number(value));
    case "lf.receiverLight": return callClientMethod(info, "setLunaFuryReceiverLightMode", Number(value));
    case "lf.buttonRate": return callClientMethod(info, "setLunaFuryButtonPollingRate", Number(value));
    case "lf.separateAxes": return callClientMethod(info, "setLunaFurySeparateDpiAxes", Boolean(value));
    case "lf.guard": case "lf.guardMs": {
      const g = status.lunafury?.wheelGuard ?? { enabled: false, windowMs: 0 };
      const next = id === "lf.guard" ? { ...g, enabled: Boolean(value) } : { ...g, windowMs: Number(value) };
      await callClientMethod(info, "setLunaFuryWheelGuard", next);
      status.lunafury.wheelGuard = next;
      return;
    }
    case "dongle.led": return callClientMethod(info, "setDongleLed", Boolean(value));
    case "dongle.incott": return callClientMethod(info, "setReceiverLed", Number(value));
    case "dongle.atk": return callClientMethod(info, "setDongleLight", Number(value));
    case "gen.profile": return w.setProfile(info, Number(value));
    case "gen.powerMode": return callClientMethod(info, "setPowerMode", value);
    case "perf.sleep": return w.setSleepTimeout(info, Number(value));
    case "perf.motionSync": return w.setMotionSync(info, Boolean(value));
    case "perf.angleSnapping": return w.setAngleSnapping(info, Boolean(value));
    case "perf.rippleControl": return w.setRippleControl(info, Boolean(value));
    case "perf.performanceMode": return w.setPerformanceMode(info, Boolean(value));
    case "perf.dpiStage": return w.setActiveDpiStage(info, Number(value));
    case "perf.debounce": return w.setDebounceTime(info, Number(value));
    case "perf.lowPower": return w.setLowPowerThreshold(info, Number(value));
    case "perf.usbSpeed": return w.setUsbSpeed(info, value);
    case "perf.primaryButton": return w.setPrimaryButton(info, value);
    case "egg.slamclick": return w.setSlamclickFilter(info, value);
    case "egg.jitter": return w.setMotionJitterFilter(info, value);
    case "egg.spdt.left": return w.setSpdtMode(info, "left", value);
    case "egg.spdt.right": return w.setSpdtMode(info, "right", value);
    case "egg.cpiLevels": return w.setCpiLevels(info, Number(value));
    case "egg.pollingDivider": return w.setCustomPollingDivider(info, Number(value));
    case "nin.systemMode": return w.setNinjutsoSystemMode(info, value);
    case "nin.engine": return w.setNinjutsoOpticalEngine(info, value);
    case "nin.hyperclick": return w.setNinjutsoHyperClick(info, Boolean(value));
    case "nin.slamclick": return w.setNinjutsoSlamClick(info, value);
    case "fm.dongleLed": return w.setDongleLedMode(info, Number(value));
    case "fm.scrollMode": return w.setTournamentScrollMode(info, Number(value));
    case "fm.scrollTimeout": return w.setTournamentScrollTimeout(info, Number(value));
    case "tee.dpiLed": return w.setDpiLighting(info, Number(value), status.dpiLedBrightness ?? 0, status.dpiLedSpeed ?? 0);
    case "tee.duration": return w.setPerformanceDuration(info, Number(value));
    case "pulsar.wheel": return w.setWheelAcceleration(info, Boolean(value));
    case "pulsar.profile": return w.setProfile(info, Number(value));
  }
  if (id.startsWith("lf.debounce.")) return callClientMethod(info, "setLunaFuryButtonDebounce", id.split(".")[2], Number(value));
  if (family === "razer" && kind === "button") return w.setButtonMapping(info, rest as any, value);
  if (family === "egg" && kind === "button") return w.setButtonMapping(info, Number(rest), value);
  if (family === "egg" && kind === "debounce") return w.setMulticlickFilter(info, Number(rest), Number(value));
  throw new Error(`Unknown control ${id}`);
}

/** The lighting write DeviceLightingTab.applyAll does, from the edited fields. */
export async function writeLighting(info: HidInterfaceInfo, status: any, edit: { mode: string; color?: string | null; color2?: string | null; speed?: number | null }) {
  const l = status.lighting;
  if (!l) throw new Error("This device has no lighting controls.");
  const showColor = l.colorModes.includes(edit.mode);
  const showDual = l.dualColorModes.includes(edit.mode);
  const showSpeed = l.reactiveModes.includes(edit.mode);
  return w.setLighting(info, {
    zone: l.zone,
    modes: l.modes,
    mode: edit.mode,
    color: showColor ? edit.color ?? l.color ?? "#ff0000" : null,
    color2: showDual ? edit.color2 ?? l.color2 ?? "#0000ff" : null,
    colorModes: l.colorModes,
    dualColorModes: l.dualColorModes,
    reactiveModes: l.reactiveModes,
    speeds: l.speeds,
    speed: showSpeed ? edit.speed ?? l.speed ?? 1 : null,
    writeOnly: l.writeOnly,
  } as any);
}
