// Which drivers can drive a device, and which collection they talk on, come
// from the protocol libraries themselves — `@openmouse/protocol`'s
// `DEVICE_DRIVERS` registry (brand, supports, create, score) and
// `SUPPORTED_HID_FILTERS`, plus `@openmouse/keyboard-protocol`'s own
// `DEVICE_DRIVERS` / `SUPPORTED_HID_FILTERS` for analog keyboards.
//
// This file used to carry a hand-maintained copy of all of that: driver
// imports, brand names, candidate order, product-id exclusions, and a guessed
// Razer control collection. It had to, because the registry matches devices
// with `isSupported(device)`, which reads `device.collections` — and the Tauri
// transport could not supply collections (see tauri-hid-device.ts's own docs
// saying so). That gap is closed: src-tauri/src/hid_descriptor.rs parses each
// interface's report descriptor and the collections reach the device object, so
// the libraries' registries are authoritative here. A brand or protocol added
// upstream now arrives in this app with no change at all.

import { DEVICE_DRIVERS as MOUSE_DRIVERS } from "@openmouse/protocol/drivers/registry";
import { SUPPORTED_HID_FILTERS as MOUSE_FILTERS, VENDOR_ID as MOUSE_VENDORS } from "@openmouse/protocol/drivers/vendors";
import type { MouseStatus } from "@openmouse/protocol/drivers/mouse-types";
import { DEVICE_DRIVERS as KEYBOARD_DRIVERS } from "@openmouse/keyboard-protocol/drivers/registry";
import { SUPPORTED_HID_FILTERS as KEYBOARD_FILTERS, VENDOR_ID as KEYBOARD_VENDORS } from "@openmouse/keyboard-protocol/drivers/vendors";
import type { KeyboardStatus } from "@openmouse/keyboard-protocol/drivers/keyboard-types";
import { TauriHidDevice, type HidInterfaceInfo } from "./tauri-hid-device";
import { unsupportedKeyboardName } from "./unsupported-keyboards";

/** Either status shape a connected device can report — mice and analog keyboards. */
export type DeviceStatus = MouseStatus | KeyboardStatus;

/**
 * Whether a status came from a keyboard driver. Keyboards report no DPI;
 * every mouse status carries it. A type guard so callers keep narrowing.
 */
export function isKeyboardStatus(status: DeviceStatus): status is KeyboardStatus {
  return !("dpi" in status);
}

/** Every filter either protocol library declares, for vendor-id discovery. */
const ALL_KNOWN_FILTERS: readonly HIDDeviceFilter[] = [...MOUSE_FILTERS, ...KEYBOARD_FILTERS];

/** Registry brand name → vendor id, across both libraries. */
const ALL_VENDORS: Readonly<Record<string, number>> = { ...MOUSE_VENDORS, ...KEYBOARD_VENDORS };

/**
 * The combined driver table. Keyboard drivers come first so that on a tied
 * score the purpose-built keyboard driver wins over mouse-protocol's
 * stage-one Wooting shim (which reports placeholder `dpi: 0` mouse fields —
 * see keyboard-protocol's keyboard-types.ts). A keyboard the keyboard driver
 * cannot answer still falls through to the shim below.
 */
interface RegistryDriver {
  brand: string;
  supports(device: HIDDevice): boolean;
  create(device: HIDDevice): unknown;
  score(device: HIDDevice): number;
}
const ALL_DRIVERS: readonly RegistryDriver[] = [...KEYBOARD_DRIVERS, ...MOUSE_DRIVERS];

/** The shared shape the libraries' driver classes implement. */
export interface SupportedClient {
  open(onReport?: (report: unknown) => void): Promise<void>;
  close(): Promise<void>;
  readStatus(): Promise<DeviceStatus>;
}

export interface BrandedCandidate {
  /** The brand the registry attributes this driver to. */
  brand: string;
  /** Builds the driver for a device — the registry's own `create`. */
  create: (device: HIDDevice) => SupportedClient | null;
  /** The registry's confidence that this driver fits, highest wins. */
  score: (device: HIDDevice) => number;
  /** Whether this driver claims the device: reads `device.collections`. */
  supports: (device: HIDDevice) => boolean;
  /** The collection the library declares for this device, if it declares one. */
  preferredCollection?: { usagePage: number; usage: number };
}

/** Every vendor id either protocol library knows about, for a single HID scan. */
export function allKnownVendorIds(): number[] {
  return [
    ...new Set(
      ALL_KNOWN_FILTERS.flatMap((filter) => (filter.vendorId === undefined ? [] : [filter.vendorId])),
    ),
  ];
}

/**
 * The collection a device's driver talks on, when the library declares one.
 *
 * A request *filter* that names a usage is exactly that declaration — the
 * library narrows those devices because it knows which collection the driver
 * uses (Logitech's HID++ interfaces on usage page 0xFF00, the Razer Viper V3's
 * Generic Desktop Mouse collection, and so on). A filter with no usage says the
 * opposite: "which interface carries the control channel has not been
 * established, offer each one and let the driver reject what cannot answer" —
 * and the transport does exactly that, fanned over every interface.
 *
 * Do not broaden this to the vendor's *other* entries. It looks tempting (seven
 * Razer filters name 0x01/0x02, and a DeathAdder V3 HyperSpeed's own filter is
 * bare), but MEASURED on that hardware: the interface which answers Razer's
 * commands is the composite one, not the single-collection Generic Desktop
 * Mouse interface those filters name — a request written to the latter came
 * back as a stale queued report and never as an answer. Ranking it first would
 * cost the connection that works.
 */
function preferredCollectionFor(
  vendorId: number,
  productId: number,
): { usagePage: number; usage: number } | undefined {
  const exact = ALL_KNOWN_FILTERS.find(
    (filter) => filter.vendorId === vendorId && filter.productId === productId,
  );
  const brandWide = ALL_KNOWN_FILTERS.find(
    (filter) => filter.vendorId === vendorId && filter.productId === undefined,
  );
  for (const filter of [exact, brandWide]) {
    if (filter?.usagePage !== undefined && filter.usage !== undefined) {
      return { usagePage: filter.usagePage, usage: filter.usage };
    }
  }
  return undefined;
}

/**
 * Every driver either library offers, in registry order, with the collection
 * preference its filters declare for this device. Callers keep the drivers that
 * `supports(device)` claims and order them by `score(device)` — the same
 * decision the library makes in a browser, which is why no brand list lives
 * here any more.
 *
 * `create` is cast through `unknown`: a few driver classes (LogitechHidppClient,
 * for one) mark `open()` private in their own `.d.ts` since nothing inside
 * mouse-protocol calls it externally, but every class does implement it —
 * OpenMouse-Bridge's `native-hid/src/hid-device-adapter.mjs` already calls it
 * this way at runtime, from plain JS where compile-time privacy does not exist.
 * The cast tells TS to trust that, once, here.
 */
export function candidatesForVendorId(vendorId: number, productId: number): BrandedCandidate[] {
  const preferredCollection = preferredCollectionFor(vendorId, productId);
  return ALL_DRIVERS.map((driver) => ({
    brand: driver.brand,
    create: driver.create as unknown as (device: HIDDevice) => SupportedClient | null,
    score: driver.score,
    supports: driver.supports,
    preferredCollection,
  }));
}

/**
 * The drivers to try for one interface, best first.
 *
 * The library decides: `supports(device)` — which reads `device.collections` —
 * then `score(device)`. Where nothing claims the device, eligibility falls back
 * to the *vendor*, because on this host the collections are not always
 * available (hidapi cannot read a report descriptor on macOS, see
 * hid_descriptor.rs) and every one of those predicates needs them. The vendor
 * gate is derived from the libraries on both sides — `VENDOR_ID`'s keys matched
 * against the registries' brand names, normalised, since the libraries spell
 * them "Endgame Gear" there and `endgameGear` here.
 *
 * Trying every driver instead is not a safe fallback: CONFIRMED against real
 * hardware, where the Logitech stopped answering after all forty drivers had
 * been given a turn opening it.
 */
export function candidatesForDevice(info: HidInterfaceInfo): BrandedCandidate[] {
  const all = candidatesForVendorId(info.vendorId, info.productId);
  // A driver whose predicate throws is one that cannot drive this device.
  const probe = new TauriHidDevice(info, all[0]?.preferredCollection);
  const claimed = all.filter((candidate) => {
    try {
      return candidate.supports(probe);
    } catch {
      return false;
    }
  });
  // A keyboard on Razer's VID can expose a mouse collection. When no keyboard
  // driver claims it, the vendor fallback would try every Razer mouse driver.
  const eligible = claimed.length > 0
    ? claimed
    : unsupportedKeyboardName(info) !== null
      ? []
      : all.filter((candidate) => brandServesVendor(candidate.brand, info.vendorId));
  return eligible.slice().sort((a, b) => b.score(probe) - a.score(probe));
}

/** Registry brand name → the vendor ids either `VENDOR_ID` lists for that brand. */
function brandServesVendor(brand: string, vendorId: number): boolean {
  const normalise = (value: string) => value.toLowerCase().replace(/[^a-z0-9]/g, "");
  return Object.entries(ALL_VENDORS).some(
    ([key, id]) => normalise(key) === normalise(brand) && id === vendorId,
  );
}
