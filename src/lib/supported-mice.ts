// Read-only reference catalog of every mouse model the protocol can drive,
// grouped by brand, mirroring the webapp's supported-mice table. The Desktop
// app does NOT use this to detect mice — detection is vendor-id probe driven
// (native-hid/brands.ts) against the same driver registry, so any supported
// mouse already appears live the moment it's plugged in. This page exists so
// the user can browse what's covered without a device attached.
//
// Keep this list in sync with mouse-protocol's src/drivers/registry.ts. A
// mouse listed here is "supported" in the sense that the pinned driver
// registry has a working driver for it (some via a receiver/collection claim
// rather than a specific PID).

export interface SupportedModel {
  model: string;
  note?: string;
}

export interface SupportedBrand {
  brand: string;
  models: SupportedModel[];
}

export const SUPPORTED_BRANDS: SupportedBrand[] = [
  {
    brand: "Logitech",
    models: [
      { model: "G502 (all variants)", note: "wired direct; Lightspeed dongle also works" },
      { model: "G203 LIGHTSYNC" },
      { model: "G203 PRODIGY" },
      { model: "G102 LIGHTSYNC" },
      { model: "G402 Hyperion Fury" },
      { model: "G303 Daedalus Apex" },
      { model: "G Pro (2017)" },
      { model: "G502 HERO (wired)" },
      { model: "G Pro Hero" },
      { model: "G903 HERO (wired)" },
      { model: "G403 HERO (wired)" },
      { model: "G703 (wired)" },
      { model: "G Pro X Superlight" },
      { model: "G305" },
      { model: "G304" },
      { model: "G Pro Wireless" },
      { model: "G Pro 2 Lightspeed" },
      { model: "G502 Lightspeed" },
      { model: "G502 X (wired)" },
      { model: "G502 X PLUS (wireless)" },
      { model: "G502 X LIGHTSPEED" },
      { model: "G309 Lightspeed" },
      { model: "MX Master 3S", note: "via Logi Bolt receiver" },
      { model: "MX Anywhere 3", note: "via Logi Bolt receiver" },
      { model: "MX Ergo S", note: "via Logi Bolt receiver" },
    ],
  },
  {
    brand: "Razer",
    models: [
      { model: "DeathAdder V2" },
      { model: "DeathAdder V2 Pro" },
      { model: "DeathAdder V3 (wired)" },
      { model: "DeathAdder V4 Pro" },
      { model: "DeathAdder Essential" },
      { model: "Cobra" },
      { model: "Viper Mini" },
      { model: "Viper V3 Pro" },
      { model: "Viper V3 HyperSpeed" },
      { model: "Viper V4 Pro" },
    ],
  },
  {
    brand: "Endgame Gear",
    models: [
      { model: "OP1 8K" },
      { model: "XM2 8K" },
      { model: "OP1we" },
      { model: "XM2w 4K" },
      { model: "OP1w 4K v2" },
    ],
  },
  {
    brand: "Pulsar",
    models: [
      { model: "Tenz Signature" },
      { model: "X2h" },
      { model: "X2 V3 ES Mini" },
      { model: "X3 Medium" },
      { model: "X2F" },
      { model: "Pulsar Pro 4K/8K dongle" },
    ],
  },
  {
    brand: "WLMouse",
    models: [
      { model: "Beast X" },
      { model: "Beast Mini" },
      { model: "Beast X Pro" },
      { model: "Beast Mini Pro" },
      { model: "Beast Max" },
      { model: "Beast G" },
      { model: "Beast Miao" },
      { model: "Sword X" },
      { model: "Strider" },
      { model: "Huan" },
      { model: "Ying" },
    ],
  },
  {
    brand: "Lamzu",
    models: [{ model: "Maya X" }],
  },
  {
    brand: "CRDRAKO",
    models: [{ model: "KO-ONE" }],
  },
  {
    brand: "Attack Shark",
    models: [{ model: "R5 Ultra" }],
  },
  {
    brand: "ATK",
    models: [{ model: "F1 V2 Ultra Max" }],
  },
  {
    brand: "VGN",
    models: [{ model: "Dragonfly F2 Master+" }],
  },
  {
    brand: "Teevolution",
    models: [{ model: "Terra Pro" }],
  },
  {
    brand: "Ninjutso",
    models: [{ model: "Sora V2" }, { model: "Sora V3" }, { model: "TEN" }],
  },
  {
    brand: "Orbital",
    models: [{ model: "Ghost / Pathfinder V2" }, { model: "Pathfinder V1" }],
  },
  {
    brand: "Keychron",
    models: [{ model: "Nape Pro" }, { model: "M6" }],
  },
  {
    brand: "Fantech",
    models: [{ model: "WG14P Yari Pro Wireless 8K Gaming Mouse" }],
  },
  {
    brand: "Lingbao",
    models: [{ model: "M5 Pro (2.4G / wired)" }],
  },
  {
    brand: "G-Wolves",
    models: [{ model: "HTX Ultra (wired)" }, { model: "HTX Ultra (wireless)" }],
  },
  {
    brand: "Finalmouse",
    models: [{ model: "Starlight-12 / ULX" }],
  },
  {
    brand: "Zaunkoenig",
    models: [{ model: "M3K" }, { model: "M2K" }],
  },
  {
    brand: "WALLHACK",
    models: [{ model: "M-001" }],
  },
  {
    brand: "moddoMOUSE",
    models: [{ model: "moddo" }],
  },
];