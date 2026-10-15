// Entry for the JS bundle embedded in the GPUI app (see build.mjs). Exposes
// the same list/connect flow the Tauri app runs (src/native-hid/scan.ts),
// bundled unmodified, behind two JSON-in/JSON-out functions for Rust.

import "./globals";
import { isMockKey, mockBrand, mockCandidates, mockOnboard, mockOnboardAction, mockStatus } from "./mock";
import { artBounds, deviceImageFilename, deviceImage } from "../../src/native-hid/device-images";
import * as logitechWrite from "../../src/native-hid/logitech-actions";
import * as genericWrite from "../../src/native-hid/write";
import { deviceCapabilities, dpiBounds } from "../../src/native-hid/write";
import { DPI_MAX, DPI_MIN, DPI_PRESETS, DPI_STEP, GAMING_SURFACE_MODES } from "../../src/lib/logitech-controls";
import { withLogitechClient } from "../../src/native-hid/logitech-actions";
import { LOGITECH_BUTTON_ACTIONS } from "@openmouse/protocol/drivers/logitech/onboard-profiles";
import { applyGameProfile } from "../../src/lib/game-profiles";
import { describeControls, writeControl, writeLighting } from "./controls";
import { connectToInterface, listCandidateInterfaces, type CandidateInterface } from "../../src/native-hid/scan";

let lastList = new Map<string, CandidateInterface>();
const lastStatus = new Map<string, any>();
const lastCaps = new Map<string, any>();
const mockWanted = (): string => String((globalThis as any).__mock ?? "");

(globalThis as any).__api = {
  async listCandidates(): Promise<string> {
    const candidates = await listCandidateInterfaces();
    candidates.push(...(mockCandidates(mockWanted()) as any[]));
    lastList = new Map(candidates.map((candidate) => [candidate.info.key, candidate]));
    return JSON.stringify(candidates);
  },
  /** Artwork for a device: URL plus the box-relative placement `deviceArtStyle` computes in the Tauri app. */
  async deviceImage(argsJson: string): Promise<string> {
    const [key, name] = JSON.parse(argsJson) as [string, string];
    const filename = deviceImageFilename(key, name);
    const bounds = artBounds(filename);
    let art = null;
    if (bounds) {
      // Same math as deviceArtStyle() in device-images.ts (target 0.5, cap 50).
      const linear = Math.sqrt(bounds.w * bounds.h * bounds.aspect);
      if (linear > 0) {
        const height = Math.min(
          (100 * 0.5) / linear,
          50 / Math.max(bounds.aspect * bounds.w, 0.01),
          50 / Math.max(bounds.h, 0.01),
        );
        art = {
          height,
          aspect: bounds.aspect,
          offsetX: (0.5 - (bounds.x + bounds.w / 2)) * 100,
          offsetY: (0.5 - (bounds.y + bounds.h / 2)) * 100,
        };
      }
    }
    return JSON.stringify({ filename, url: deviceImage(key, name), art });
  },
  async connect(key: string): Promise<string> {
    const candidate = lastList.get(key);
    if (!candidate) throw new Error(`Unknown interface ${key}; list candidates first.`);
    const mock = isMockKey(key);
    const device = mock ? { key, brand: mockBrand(key), driver: "Mock", status: mockStatus(key) as any } : await connectToInterface(candidate.info);
    const caps = mock ? ({ sleepTimeouts: null, lowPowerThresholds: null, debounceMaxMs: null, debounceOptions: null, dpiMax: 26000 } as any) : deviceCapabilities(candidate.info);
    // Fallback DPI limits and presets per brand, as DevicePerformanceTab.tsx has
    // them: the device's own limits win (dpiBounds), these cover drivers that
    // publish none.
    const razer = device.brand === "Razer";
    const attackShark = device.brand === "Attack Shark";
    const bounds = dpiBounds(device.status, caps, {
      min: attackShark ? 50 : razer ? 100 : DPI_MIN,
      max: attackShark ? 22000 : razer ? 8500 : DPI_MAX,
      step: attackShark ? 50 : razer ? 100 : DPI_STEP,
    });
    const presets = attackShark ? [400, 800, 1600, 3200, 6400, 12800, 22000] : razer ? [400, 800, 1600, 3200, 6400, 8000] : DPI_PRESETS;
    lastStatus.set(key, device.status);
    lastCaps.set(key, caps);
    return JSON.stringify({
      ...device,
      // `brand` is the registry's label for the driver that answered and what the
      // app's own gating compares against; a product can carry its own brand (an
      // Attack Shark R5 Ultra is driven by the Lamzu driver and says so), which is
      // what a person should be shown.
      displayBrand: device.status.brand || device.brand,
      controls: { dpi: bounds, dpiPresets: presets, surfaceModes: GAMING_SURFACE_MODES },
      tabs: describeControls(device.brand, device.status, caps),
    });
  },
  /** Logitech onboard profiles, read on demand (a full pass is ~80 round trips). */
  async onboardProfiles(argsJson: string): Promise<string> {
    const { key } = JSON.parse(argsJson) as { key: string };
    if (isMockKey(key)) return JSON.stringify(mockOnboard(LOGITECH_BUTTON_ACTIONS as string[]));
    const candidate = lastList.get(key);
    if (!candidate) throw new Error(`Unknown interface ${key}; list candidates first.`);
    const profiles: any[] = await withLogitechClient(candidate.info, "readOnboardProfiles", (client: any) => client.readOnboardProfiles());
    // `raw` is a Uint8Array kept for maintainers' captures; the page has no use for it.
    return JSON.stringify({ profiles: profiles.map(({ raw, ...rest }) => rest), actions: LOGITECH_BUTTON_ACTIONS });
  },
  /** Switch the running onboard profile (volatile, no flash write) or Onboard/Host mode. */
  async onboardAction(argsJson: string): Promise<string> {
    const { key, action, value } = JSON.parse(argsJson) as { key: string; action: "useProfile" | "mode"; value: any };
    if (isMockKey(key)) return JSON.stringify(mockOnboardAction(action, value));
    const candidate = lastList.get(key);
    if (!candidate) throw new Error(`Unknown interface ${key}; list candidates first.`);
    return JSON.stringify(
      await withLogitechClient(candidate.info, `onboard ${action}`, async (client: any) => {
        if (action === "useProfile") {
          // A stored profile only runs while the mouse is in Onboard mode.
          await client.setOnboardMode("Onboard");
          await client.setCurrentProfile(Number(value));
          return value;
        }
        return client.setOnboardMode(value === "Onboard" ? "Onboard" : "Host");
      }),
    );
  },
  /** A game profile, applied in one device session (the Tauri app's own applyGameProfile). Resolves to the values that were applied. */
  async applyProfile(argsJson: string): Promise<string> {
    const { key, profile } = JSON.parse(argsJson) as { key: string; profile: any };
    const candidate = lastList.get(key);
    if (!candidate) throw new Error(`Unknown interface ${key}; list candidates first.`);
    const applied: Record<string, unknown> = {};
    await applyGameProfile(candidate.info, profile, (patch) => Object.assign(applied, patch));
    return JSON.stringify(applied);
  },
  /**
   * One live setting. Logitech goes through its receiver-aware module, every
   * other brand through the generic one — the same split DevicePerformanceTab
   * makes. Resolves to the value the device actually applied.
   */
  async write(argsJson: string): Promise<string> {
    const { key, brand, action, value } = JSON.parse(argsJson) as {
      key: string;
      brand: string;
      action: string;
      value: any;
    };
    const candidate = lastList.get(key);
    if (!candidate) throw new Error(`Unknown interface ${key}; list candidates first.`);
    const w = brand === "Logitech" ? logitechWrite : genericWrite;
    const info = candidate.info;
    if (isMockKey(key) && !action.startsWith("control:")) return JSON.stringify(value);
    // Buttons / Advanced tab controls ("control:<id>") and the Lighting tab.
    if (action.startsWith("control:")) {
      const status = lastStatus.get(key) ?? {};
      const id = action.slice("control:".length);
      await writeControl(info, status, id, value);
      // A few controls decide which others are shown (an Starlight X switch in
      // TMR mode gains its analog settings); their write returns the new layout.
      if (id.startsWith("slx.mode") || id === "slx.link" || id === "hits.link") return JSON.stringify({ tabs: describeControls(brand, status, lastCaps.get(key)) });
      return JSON.stringify(value);
    }
    if (action === "lighting") {
      return JSON.stringify(await writeLighting(info, lastStatus.get(key) ?? {}, value));
    }
    switch (action) {
      case "dpi":
        // A number moves both axes; {x, y} sets them apart (mice that report
        // separate axes, with X and Y unlinked).
        return JSON.stringify(
          typeof value === "number"
            ? await w.setDpi(info, Math.round(value))
            : await w.setDpi(info, Math.round(value.x), Math.round(value.y)),
        );
      case "pollingRate":
        return JSON.stringify(await w.setPollingRate(info, value));
      case "liftOff":
        return JSON.stringify(await w.setLiftOffDistance(info, value));
      case "surface":
        return JSON.stringify(await w.setGamingSurfaceMode(info, value));
      default:
        throw new Error(`Unknown action ${action}`);
    }
  },
};
