/**
 * Product photos for the device list and device card, keyed by the same
 * `vendorId:productId` string `HidInterfaceInfo.key` / `ConnectedDevice.key`
 * already use (see `tauri-hid-device.ts`'s `key()` and hid.rs's
 * `interface_key()` — both hex-pad to `"0000:0000"`).
 *
 * Ported from the openmouse webapp's own `src/ui/device-images.ts` (dev
 * branch) — same lookup table, same name-pattern fallback for shared
 * receiver ids, same asset filenames, just re-keyed onto this app's own
 * identifiers since there's no WebHID `HIDDevice` here to read vendor/product
 * off of directly. Keep this file and `public/devices/` in sync with that
 * repo when new hardware gets art there.
 *
 * Files live in `public/devices/`, which Vite serves from the site root and
 * copies into the build unprocessed. A key whose file is missing therefore
 * fails at load, not at build time — callers render the `<img>` with an
 * `onError` fallback to `unknown-device.png` rather than trust this always
 * resolving to a file that exists.
 */
const DEVICE_IMAGES: ReadonlyMap<string, string> = new Map([
  ["3710:5405", "/devices/pulsar-pro-dongle.png"],
  ["046d:c07d", "/devices/logitech-g502.png"],
  ["046d:c095", "/devices/logitech-g502-x-plus.png"],
  ["046d:c099", "/devices/logitech-g502-x.png"],
  ["046d:c0a8", "/devices/logitech-pro-x2-superstrike.png"],
  // Original G703 (0xc087) and G703 HERO wired (0xc090) share the same shell.
  ["046d:c087", "/devices/logitech-g703.png"],
  ["046d:c090", "/devices/logitech-g703.png"],
  // M3K and M2K use the supplied M3K product artwork.
  ["0483:a462", "/devices/zaunkoenig-m3k.png"],
  ["0483:a3cf", "/devices/zaunkoenig-m3k.png"],
  // Wired and receiver are separate product ids for the same mouse.
  ["1532:00a5", "/devices/razer-viper-v2-pro.png"],
  ["1532:00a6", "/devices/razer-viper-v2-pro.png"],
  ["1532:00c0", "/devices/razer-viper-v3-pro.png"],
  ["1532:00c1", "/devices/razer-viper-v3-pro.png"],
  ["1532:008a", "/devices/razer-viper-mini.webp"],
  ["1532:0078", "/devices/razer-viper.webp"],
  ["1532:00a3", "/devices/razer-cobra.webp"],
  // CRDRAKO KO-ONE wired and receiver transports share the same shell.
  ["373e:006a", "/devices/crdrako-ko-one.png"],
  ["373e:006b", "/devices/crdrako-ko-one.png"],
  // Attack Shark R5 Ultra wired and wireless transports share the same shell.
  ["373e:0046", "/devices/attackshark-r5-ultra.png"],
  ["373e:0047", "/devices/attackshark-r5-ultra.png"],
  // OP1 8K, Purple Frost, and v2. XM2 models use different shells.
  ["3367:1964", "/devices/endgame-gear-op1-8k.png"],
  ["3367:1976", "/devices/endgame-gear-op1-8k.png"],
  ["3367:1978", "/devices/endgame-gear-op1-8k.png"],
  // NinjaForce exposes separate wired and receiver ids for Sora V2, Sora V3,
  // and the TEN family. Receiver variants show the paired mouse artwork.
  ["1915:ae11", "/devices/ninjutso-sora-v2.png"],
  ["1915:ae12", "/devices/ninjutso-sora-v2.png"],
  ["1915:ae13", "/devices/ninjutso-sora-v2.png"],
  ["1915:ae14", "/devices/ninjutso-sora-v2.png"],
  ["1915:ae15", "/devices/ninjutso-sora-v2.png"],
  ["1915:ae16", "/devices/ninjutso-sora-v2.png"],
  ["1915:ae1c", "/devices/ninjutso-sora-v2.png"],
  ["1915:ae8a", "/devices/ninjutso-sora-v2.png"],
  ["1915:ae8c", "/devices/ninjutso-sora-v2.png"],
  ["093a:e010", "/devices/ninjutso-sora-v3.png"],
  ["093a:eb02", "/devices/ninjutso-sora-v3.png"],
  ["093a:e020", "/devices/ninjutso-ten.png"],
  ["093a:ea01", "/devices/ninjutso-ten.png"],
  ["093a:eb01", "/devices/ninjutso-ten.png"],
  // WLMouse Beast G receiver / wired transports share the same shell.
  ["36a7:a860", "/devices/wlmouse-beast-g.png"],
  ["36a7:a861", "/devices/wlmouse-beast-g.png"],
  // Nape Pro wired / Link-KM receivers share the same shell artwork.
  ["3434:0440", "/devices/keychron-nape-pro.png"],
  ["3434:d026", "/devices/keychron-nape-pro.png"],
  ["3434:d029", "/devices/keychron-nape-pro.png"],
  // Teevolution Terra Pro wired / receiver Compx transports.
  ["3554:f520", "/devices/teevolution-terra-pro.png"],
  ["3554:f522", "/devices/teevolution-terra-pro.png"],
  ["3554:f523", "/devices/teevolution-terra-pro.png"],
  ["3554:f5bb", "/devices/teevolution-terra-pro.png"],
  // WALLHACK M-001 wireless mouse (real config id and in-app demo id).
  ["3879:1110", "/devices/wallhack-m-001.png"],
  ["3879:0807", "/devices/wallhack-m-001.png"],
  // WALLHACK K-001 analog keyboard (both enumerated vendor ids).
  ["3879:0806", "/devices/wallhack-k-001.png"],
  ["1caa:0806", "/devices/wallhack-k-001.png"],
]);

export const UNKNOWN_DEVICE_IMAGE = "/devices/unknown-device.png";

/**
 * Best-known product photo for a device. `key` is `HidInterfaceInfo.key` /
 * `ConnectedDevice.key`; `displayName` is the product/friendly name (from
 * `HidInterfaceInfo.productString` or `MouseStatus.name`) — used as a
 * fallback for shared receiver product ids, same as the webapp.
 */
export function deviceImage(key: string | null | undefined, displayName = ""): string {
  const mapped = key ? DEVICE_IMAGES.get(key) ?? null : null;
  if (mapped) return mapped;
  // Lightspeed receivers are shared product IDs, so paired G502 X variants
  // must use the friendly name read from the mouse itself.
  if (/g502\s*x\s*plus/i.test(displayName)) return "/devices/logitech-g502-x-plus.png";
  if (/g502\s*x/i.test(displayName)) return "/devices/logitech-g502-x.png";
  if (/\bg502\b/i.test(displayName)) return "/devices/logitech-g502.png";
  if (/\bg703\b/i.test(displayName)) return "/devices/logitech-g703.png";
  if (/mx\s*master\s*4/i.test(displayName)) return "/devices/logitech-mx-master-4.png";
  if (/superstrike/i.test(displayName)) return "/devices/logitech-pro-x2-superstrike.png";
  if (/superlight/i.test(displayName)) return "/devices/logitech-pro-x-superlight-2c.png";
  if (/\bop1\b/i.test(displayName)) return "/devices/endgame-gear-op1-8k.png";
  if (/\bviper\s*v2\s*pro\b/i.test(displayName)) return "/devices/razer-viper-v2-pro.png";
  if (/\bviper\s*mini\b/i.test(displayName)) return "/devices/razer-viper-mini.webp";
  if (/\bcobra\b/i.test(displayName)) return "/devices/razer-cobra.webp";
  if (/\bnape\s*pro\b/i.test(displayName)) return "/devices/keychron-nape-pro.png";
  if (/\bko-one\b/i.test(displayName)) return "/devices/crdrako-ko-one.png";
  if (/\br5\s*ultra\b/i.test(displayName)) return "/devices/attackshark-r5-ultra.png";
  if (/\bm[23]k\b/i.test(displayName)) return "/devices/zaunkoenig-m3k.png";
  if (/\bmx\s*master\s*3s\b/i.test(displayName)) return "/devices/logitech-mx-master-3s.png";
  if (/\bterra\s*pro\b/i.test(displayName)) return "/devices/teevolution-terra-pro.png";
  if (/\bm-001\b/i.test(displayName)) return "/devices/wallhack-m-001.png";
  if (/\bk-001\b/i.test(displayName)) return "/devices/wallhack-k-001.png";
  // Pulsar 4K Wireless Receiver ships with the X2 V2 4K dongle kit; the
  // receiver product id is not yet published, so match the reported name.
  if (/pulsar/i.test(displayName)) return "/devices/pulsar-x2-v2.png";
  return UNKNOWN_DEVICE_IMAGE;
}
