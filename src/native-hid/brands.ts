// Maps a HID vendor id to the candidate `@openmouse/protocol` driver
// classes to try constructing against each interface found there. Ported
// from OpenMouse-Bridge's `native-hid/src/brands.mjs`, which does the same
// job for a Node host — see that file's comments for how this list tracks
// mouse-protocol/src/drivers/registry.ts's own DEVICE_DRIVERS order.
//
// Unlike Bridge, this list includes Pulsar: Bridge deliberately excludes it
// because Bridge has a separate dependency-free native Rust driver for it
// and wants to avoid spawning Node for something Rust already handles.
// openmouse-desktop has no such Rust driver (yet) and reuses the driver
// classes for every brand uniformly through `TauriHidDevice`, so Pulsar
// belongs here too.
//
// `isSupported()` on every one of these classes gates on `device.collections`,
// which `TauriHidDevice` cannot populate (see its module docs) — so brands
// are matched here by vendor id, and every candidate class for that vendor
// id is tried in turn (`open()` + `readStatus()`) rather than trusting
// `isSupported()`. See `probeInterface()` in `src/native-hid/scan.ts`.

import { AtkHidClient } from "@openmouse/protocol/drivers/atk/hid";
import { AttackSharkHidClient } from "@openmouse/protocol/drivers/attackshark/hid";
import { EggOp1HidClient } from "@openmouse/protocol/drivers/endgame/egg-op1-hid";
import { EggWeHidClient } from "@openmouse/protocol/drivers/endgame/egg-we-hid";
import { FantechHidClient } from "@openmouse/protocol/drivers/fantech/hid";
import { FinalmouseHidClient } from "@openmouse/protocol/drivers/finalmouse/hid";
import { GWolvesHidClient } from "@openmouse/protocol/drivers/gwolves/hid";
import { KeychronHidClient } from "@openmouse/protocol/drivers/keychron/hid";
import { LamzuHidClient } from "@openmouse/protocol/drivers/lamzu/hid";
import { LogitechHidppClient } from "@openmouse/protocol/drivers/logitech/hidpp";
import { ModdoHidClient } from "@openmouse/protocol/drivers/moddo/hid";
import { NinjutsoHidClient } from "@openmouse/protocol/drivers/ninjutso/hid";
import { OrbitalHidClient } from "@openmouse/protocol/drivers/orbital/hid";
import { PulsarHidClient } from "@openmouse/protocol/drivers/pulsar/pulsar-hid";
import { PulsarProHidClient } from "@openmouse/protocol/drivers/pulsar/pulsar-pro-hid";
import { PulsarXs1HidClient } from "@openmouse/protocol/drivers/pulsar/pulsar-xs1-hid";
import { RazerCobraHidClient } from "@openmouse/protocol/drivers/razer/cobra-hid";
import { RazerHidClient } from "@openmouse/protocol/drivers/razer/hid";
import { RazerViperHidClient } from "@openmouse/protocol/drivers/razer/viper-hid";
import { RazerViperMiniHidClient } from "@openmouse/protocol/drivers/razer/viper-mini-hid";
import { RazerViperV4ProHidClient } from "@openmouse/protocol/drivers/razer/viper-v4-pro-hid";
import { TeevolutionHidClient } from "@openmouse/protocol/drivers/teevolution/hid";
import { VgnF2HidClient } from "@openmouse/protocol/drivers/vgn/hid";
import { WallhackKeyboardHidClient } from "@openmouse/protocol/drivers/wallhack/keyboard-hid";
import { WallhackMouseHidClient } from "@openmouse/protocol/drivers/wallhack/mouse-hid";
import { WLMouseHidClient } from "@openmouse/protocol/drivers/wlmouse/hid";
import { WootingHidClient } from "@openmouse/protocol/drivers/wooting/hid";
import { ZaunkoenigHidClient } from "@openmouse/protocol/drivers/zaunkoenig/hid";
import type { MouseStatus } from "@openmouse/protocol/drivers/mouse-types";

/** The shared shape every driver class above implements. */
export interface SupportedClient {
  open(onReport?: (report: unknown) => void): Promise<void>;
  close(): Promise<void>;
  readStatus(): Promise<MouseStatus>;
}

export interface DriverCandidate {
  name: string;
  Client: new (device: HIDDevice) => SupportedClient;
  /**
   * Product ids this candidate must NOT be tried against, even though its
   * vendor id matches. `isSupported()` on every one of these classes mixes
   * collection-based checks (which TauriHidDevice can never satisfy — see
   * this file's own docs) with plain property checks like this one, which
   * have nothing to do with collections and are worth keeping. Skipping
   * `isSupported()` entirely threw this away too — e.g. EggWeHidClient's
   * own isSupported() correctly excludes Endgame Gear's OP1-8K product ids
   * (they speak EggOp1HidClient's protocol instead), but bypassing it let
   * EggWeHidClient claim and misidentify an OP1-8K mouse as "OP1we" when
   * EggOp1HidClient's own exchange failed for an unrelated reason. This
   * list is that same exclusion, reproduced by hand since the source sets
   * aren't exported.
   */
  excludeProductIds?: number[];
}

export interface BrandEntry {
  brand: string;
  vendorIds: number[];
  candidates: DriverCandidate[];
}

// Cast through `unknown`: a few driver classes (e.g. LogitechHidppClient)
// mark `open()` private in their own .d.ts since nothing inside
// mouse-protocol calls it externally, but every class here does implement
// it — apply.mjs (plain JS, no compile-time privacy) already calls it this
// same way at runtime. The cast just tells TS to trust that, once, here.
const client = (
  name: string,
  Client: new (device: HIDDevice) => unknown,
  excludeProductIds?: number[],
): DriverCandidate => ({
  name,
  Client: Client as new (device: HIDDevice) => SupportedClient,
  excludeProductIds,
});

export const BRAND_DRIVERS: BrandEntry[] = [
  { brand: "Zaunkoenig", vendorIds: [0x0483], candidates: [client("ZaunkoenigHidClient", ZaunkoenigHidClient)] },
  { brand: "Finalmouse", vendorIds: [0x361d], candidates: [client("FinalmouseHidClient", FinalmouseHidClient)] },
  // EggWeHidClient (OP1we / wireless) is missing from OpenMouse-Bridge's own
  // brands.mjs — its module only exposes `pickDevices`/`fromAuthorizedDevices`
  // helpers for merging several browser-side HIDDevice objects into one
  // logical mouse, which read as WebHID-only at a glance, but the
  // constructor itself just takes a single `HIDDevice` like every other
  // driver here — those helpers are unneeded (not incompatible) with
  // TauriHidDevice, which already merges every split of one interface
  // group into one synthetic device. So it belongs here.
  { brand: "Endgame Gear", vendorIds: [0x3367], candidates: [
    client("EggOp1HidClient", EggOp1HidClient),
    // Excludes the OP1-8K product ids EggWeHidClient's own isSupported()
    // also excludes (mouse-protocol/src/drivers/endgame/egg-we-hid.ts,
    // EGG_8K_PRODUCT_IDS) — without this, an OP1-8K mouse that
    // EggOp1HidClient fails to read gets misclaimed and misidentified as
    // an "OP1we" by EggWeHidClient instead of correctly reporting no
    // driver answered.
    client("EggWeHidClient", EggWeHidClient, [0x1964, 0x1966, 0x1976, 0x1978, 0x1980]),
  ] },
  // Real Pulsar-vendor (0x3710) mice, including the Pulsar 4K Wireless
  // Receiver (shared vendor id 0x3554 with Teevolution/VGN — see
  // pulsar-hid.ts's own vendor-id branching for how it tells those apart).
  { brand: "Pulsar", vendorIds: [0x3710, 0x3554], candidates: [
    client("PulsarXs1HidClient", PulsarXs1HidClient),
    client("PulsarProHidClient", PulsarProHidClient),
    client("PulsarHidClient", PulsarHidClient),
  ] },
  { brand: "Teevolution", vendorIds: [0x3554], candidates: [client("TeevolutionHidClient", TeevolutionHidClient)] },
  { brand: "VGN", vendorIds: [0x3554], candidates: [client("VgnF2HidClient", VgnF2HidClient)] },
  { brand: "Logitech", vendorIds: [0x046d], candidates: [client("LogitechHidppClient", LogitechHidppClient)] },
  { brand: "WLMouse", vendorIds: [0x36a7], candidates: [client("WLMouseHidClient", WLMouseHidClient)] },
  { brand: "Lamzu", vendorIds: [0x373e], candidates: [client("LamzuHidClient", LamzuHidClient)] },
  // Same CompX ODM hardware/class as Lamzu; readStatus() reports the brand
  // that matches the product id.
  { brand: "CRDRAKO", vendorIds: [0x373e], candidates: [client("LamzuHidClient", LamzuHidClient)] },
  { brand: "moddoMOUSE", vendorIds: [0x2fe3], candidates: [client("ModdoHidClient", ModdoHidClient)] },
  // NINJUTSO_VENDOR_ID (current) and NINJUTSO_LEGACY_VENDOR_ID (shared with
  // Orbital) — see mouse-protocol/src/ninjutso/index.ts.
  { brand: "Ninjutso", vendorIds: [0x093a, 0x1915], candidates: [client("NinjutsoHidClient", NinjutsoHidClient)] },
  { brand: "Orbital", vendorIds: [0x1915], candidates: [client("OrbitalHidClient", OrbitalHidClient)] },
  // Kept registered for completeness, but not actually reachable right now:
  // every one of these classes' isSupported() keys on Generic Desktop page
  // 0x01/usage 0x02 (or 0x0c for viper-v4-pro) — the mouse's own live
  // pointer collection — which src-tauri/src/hid.rs now refuses to open at
  // all (opening it caused a real cursor + app freeze during testing). No
  // Razer interface will ever appear in the connectable list until that's
  // solved a different way.
  { brand: "Razer", vendorIds: [0x1532], candidates: [
    client("RazerHidClient", RazerHidClient),
    client("RazerCobraHidClient", RazerCobraHidClient),
    client("RazerViperMiniHidClient", RazerViperMiniHidClient),
    client("RazerViperHidClient", RazerViperHidClient),
    client("RazerViperV4ProHidClient", RazerViperV4ProHidClient),
  ] },
  { brand: "ATK", vendorIds: [0x373b], candidates: [client("AtkHidClient", AtkHidClient)] },
  { brand: "Attack Shark", vendorIds: [0x1d57, 0x25a7, 0x373e], candidates: [client("AttackSharkHidClient", AttackSharkHidClient)] },
  { brand: "Keychron", vendorIds: [0x3434], candidates: [client("KeychronHidClient", KeychronHidClient)] },
  { brand: "Fantech", vendorIds: [0x3151], candidates: [client("FantechHidClient", FantechHidClient)] },
  { brand: "Wooting", vendorIds: [0x31e3], candidates: [client("WootingHidClient", WootingHidClient)] },
  { brand: "WALLHACK", vendorIds: [0x3879, 0x1caa], candidates: [
    client("WallhackMouseHidClient", WallhackMouseHidClient),
    client("WallhackKeyboardHidClient", WallhackKeyboardHidClient),
  ] },
  { brand: "G-Wolves", vendorIds: [0x3603], candidates: [client("GWolvesHidClient", GWolvesHidClient)] },
];

/** Every vendor id any known brand cares about, for a single HID scan. */
export function allKnownVendorIds(): number[] {
  return [...new Set(BRAND_DRIVERS.flatMap((entry) => entry.vendorIds))];
}

/** A driver candidate paired with the brand entry it came from. */
export interface BrandedCandidate extends DriverCandidate {
  brand: string;
}

/**
 * Every candidate whose vendor id list includes `vendorId`, in registry
 * order, excluding any candidate that has explicitly ruled out this
 * `productId` (see `DriverCandidate.excludeProductIds`).
 */
export function candidatesForVendorId(vendorId: number, productId: number): BrandedCandidate[] {
  return BRAND_DRIVERS
    .filter((entry) => entry.vendorIds.includes(vendorId))
    .flatMap((entry) => entry.candidates.map((candidate) => ({ ...candidate, brand: entry.brand })))
    .filter((candidate) => !candidate.excludeProductIds?.includes(productId));
}
