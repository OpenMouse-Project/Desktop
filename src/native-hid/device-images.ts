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
  // Attack Shark X11: wired (0xfa55) and the 2.4 GHz receiver (0xfa60) share
  // the same shell. 0xfa61 is the R1, a different mouse.
  ["1d57:fa55", "/devices/attackshark-x11.png"],
  ["1d57:fa60", "/devices/attackshark-x11.png"],
  // OP1 8K, Purple Frost, and v2. XM2 models use different shells.
  ["3367:1964", "/devices/endgame-gear-op1-8k.png"],
  ["3367:1976", "/devices/endgame-gear-op1-8k.png"],
  ["3367:1978", "/devices/endgame-gear-op1-8k.png"],
  // OP1we (same egg-we driver, 0x3367 family).
  ["3367:1961", "/devices/endgame-gear-op1we.png"],
  ["3367:1962", "/devices/endgame-gear-op1we.png"],
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
  // Beast Max wired / 4K8K receiver transports share the same shell.
  ["36a7:a880", "/devices/wlmouse-beast-max.png"],
  ["36a7:a881", "/devices/wlmouse-beast-max.png"],
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
  // Logitech G203 family. G203 LIGHTSYNC / PRODIGY and G102 share the same shell.
  ["046d:c084", "/devices/logitech-g203.png"],
  ["046d:c089", "/devices/logitech-g203.png"],
  ["046d:c092", "/devices/logitech-g203.png"],
  ["046d:c07e", "/devices/logitech-g402.png"],
  ["046d:c080", "/devices/logitech-g303.png"],
  ["046d:c08f", "/devices/logitech-g403.png"],
  ["046d:c08e", "/devices/logitech-g903.png"],
  // G Pro (2017), G Pro Hero, and G Pro Wireless share the same classic shell.
  ["046d:c085", "/devices/logitech-g-pro.png"],
  ["046d:c08c", "/devices/logitech-g-pro.png"],
  // Endgame Gear XM2 8K wired.
  ["3367:1966", "/devices/endgame-gear-xm2-8k.png"],
  ["3367:1980", "/devices/endgame-gear-xm2-8k.png"],
  // WLMouse Sword X wired / receiver transports keep their render.
  ["36a7:a878", "/devices/wlmouse-sword-x.png"],
  ["36a7:a879", "/devices/wlmouse-sword-x.png"],
  // VGN Dragonfly F2 Master+ wired / receiver transports.
  ["3554:fb56", "/devices/vgn-dragonfly-f2.png"],
  ["3554:fb57", "/devices/vgn-dragonfly-f2.png"],
  // Lamzu Maya X wired / wireless / 8K transports.
  ["373e:001c", "/devices/lamzu-maya-x.png"],
  ["373e:001d", "/devices/lamzu-maya-x.png"],
  ["373e:001e", "/devices/lamzu-maya-x.png"],
  ["1532:006e", "/devices/razer-deathadder-v2.png"],
  ["1532:0071", "/devices/razer-deathadder-v2.png"],
  ["1532:007c", "/devices/razer-deathadder-v2.png"],
  ["1532:007d", "/devices/razer-deathadder-v2.png"],
  ["1532:0084", "/devices/razer-deathadder-v2.png"],
  ["1532:0098", "/devices/razer-deathadder-v2.png"],
  // DeathAdder V4 Pro and its Carbon Fiber SKU share the same shell.
  ["1532:00be", "/devices/razer-deathadder-v4-pro.png"],
  ["1532:00bf", "/devices/razer-deathadder-v4-pro.png"],
  ["1532:00ef", "/devices/razer-deathadder-v4-pro.png"],
  ["1532:00f0", "/devices/razer-deathadder-v4-pro.png"],
  ["1532:00b8", "/devices/razer-viper-v3-hyperspeed.png"],
  ["1532:00e5", "/devices/razer-viper-v4-pro.png"],
  ["1532:00e6", "/devices/razer-viper-v4-pro.png"],
  // HyperX Pulsefire Haste: Kingston-era wired (0x0951:0x1727) and HP-era
  // wired / wired-mode / wireless dongle transports share one shell.
  ["0951:1727", "/devices/hyperx-pulsefire-haste.png"],
  ["03f0:0f8f", "/devices/hyperx-pulsefire-haste.png"],
  ["03f0:048e", "/devices/hyperx-pulsefire-haste.png"],
  ["03f0:028e", "/devices/hyperx-pulsefire-haste.png"],
]);

export const UNKNOWN_DEVICE_IMAGE = "/devices/unknown-device.png";

/** Where the visible product sits inside its artwork, as canvas fractions. */
export interface ArtBounds {
  x: number;
  y: number;
  w: number;
  h: number;
  /** Canvas width / height, so a caller can reason about rendered width. */
  aspect: number;
}

const BOUNDS_KEY = "openmouse:art-bounds";

/**
 * Alpha at or below this counts as empty. Product shots fade out over their
 * last few pixels and some carry a soft floor shadow, neither of which is the
 * mouse — including them would inflate the bounds and shrink the product.
 */
const INK_ALPHA = 24;

/** Fraction of the (square) art box one device's artwork should occupy. */
const ART_TARGET = 0.5;

/** Cap on either dimension's share of the art box, so a subject can never be
 *  normalised right up against the tile's edges. */
const ART_MAX_EXTENT = 50;

function readBoundsCache(): Record<string, ArtBounds> {
  try {
    const raw = localStorage.getItem(BOUNDS_KEY);
    return raw ? (JSON.parse(raw) as Record<string, ArtBounds>) : {};
  } catch {
    return {};
  }
}

const boundsCache = readBoundsCache();

function measureInk(image: HTMLImageElement): ArtBounds | null {
  const width = image.naturalWidth;
  const height = image.naturalHeight;
  if (!width || !height) return null;
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d", { willReadFrequently: true });
  if (!context) return null;
  context.drawImage(image, 0, 0);
  const { data } = context.getImageData(0, 0, width, height);
  let minX = width;
  let minY = height;
  let maxX = -1;
  let maxY = -1;
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      if (data[(y * width + x) * 4 + 3] <= INK_ALPHA) continue;
      if (x < minX) minX = x;
      if (x > maxX) maxX = x;
      if (y < minY) minY = y;
      if (y > maxY) maxY = y;
    }
  }
  if (maxX < 0) return null;
  return {
    x: minX / width,
    y: minY / height,
    w: (maxX - minX + 1) / width,
    h: (maxY - minY + 1) / height,
    aspect: width / height,
  };
}

/**
 * Where the product actually is inside one artwork, measured from its alpha
 * channel once per asset and cached in localStorage.
 *
 * The assets are framed inconsistently — the DeathAdder render is a square
 * canvas whose mouse fills ~35% of the width, the Superlight's is a portrait
 * canvas it fills edge to edge — so nothing about the canvas tells a tile how
 * big the mouse is. Only measuring does.
 */
export async function artBounds(src: string): Promise<ArtBounds | null> {
  const known = boundsCache[src];
  if (known) return known;
  const image = new Image();
  image.src = src;
  try {
    await image.decode();
  } catch {
    return null;
  }
  const bounds = measureInk(image);
  if (!bounds) return null;
  boundsCache[src] = bounds;
  try {
    localStorage.setItem(BOUNDS_KEY, JSON.stringify(boundsCache));
  } catch {
    // Best-effort: losing the cache only means measuring again next launch.
  }
  return bounds;
}

/**
 * Inline style that renders one artwork so its *subject* — not its canvas —
 * comes out the same size as every other tile's.
 *
 * The image is sized as a percentage of the square art box and scaled about
 * the subject's own centre: `sqrt(w*h)` is the subject's linear size in canvas
 * fractions, so making that constant across assets makes the mice match
 * regardless of how each render is framed. The second term keeps a wide
 * subject inside the box (the canvas may overflow; only its transparent
 * margins get clipped).
 *
 * Returns null when the bounds are unknown, which leaves the caller's plain
 * `contain` behaviour in place.
 */
export function deviceArtStyle(bounds: ArtBounds | null, target = ART_TARGET): string | null {
  if (!bounds) return null;
  // The subject's linear size is the square root of its *rendered* area, so
  // the canvas aspect belongs in it: a portrait canvas draws the same subject
  // fraction onto a narrower element.
  const linear = Math.sqrt(bounds.w * bounds.h * bounds.aspect);
  if (!(linear > 0)) return null;
  // Both clamps keep the *subject* inside the box (the canvas may overflow;
  // only its transparent margins are clipped). Without the height one, a
  // narrow mouse normalised by area grows taller than the tile and loses its
  // nose and cable.
  const height = Math.min(
    (100 * target) / linear,
    ART_MAX_EXTENT / Math.max(bounds.aspect * bounds.w, 0.01),
    ART_MAX_EXTENT / Math.max(bounds.h, 0.01),
  );
  const offsetX = (0.5 - (bounds.x + bounds.w / 2)) * 100;
  const offsetY = (0.5 - (bounds.y + bounds.h / 2)) * 100;
  return [
    `height:${height.toFixed(2)}%`,
    "left:50%",
    "top:50%",
    `transform:translate(-50%,-50%) translate(${offsetX.toFixed(2)}%,${offsetY.toFixed(2)}%)`,
  ].join(";");
}

/**
 * `onError` handler for a device-artwork `<img>`: retries the real source once
 * before settling for `unknown-device.png`.
 *
 * The retry is the point. A single transient failure — a dev-server reload
 * rewriting the page mid-request, a file momentarily unreadable — used to
 * latch the placeholder onto that `<img>` for the rest of the page load, so a
 * mouse whose artwork was sitting right there in `public/devices/` showed the
 * generic tile and looked unsupported. (CONFIRMED: a DeathAdder V3
 * HyperSpeed, whose render resolves correctly to razer-deathadder-v3.png,
 * displayed the placeholder in the device list.) The retry carries a query
 * string so the browser cannot reuse its cached failure for that URL; static
 * files ignore query strings, so the same artwork is fetched.
 */
export function deviceImageFallback(event: Event): void {
  const img = event.currentTarget as HTMLImageElement;
  if (img.src.endsWith(UNKNOWN_DEVICE_IMAGE)) return;
  const source = img.getAttribute("src") ?? "";
  if (!source.includes("retry=")) {
    img.src = `${source}${source.includes("?") ? "&" : "?"}retry=1`;
    return;
  }
  img.src = UNKNOWN_DEVICE_IMAGE;
}

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
  if (/op1we/i.test(displayName)) return "/devices/endgame-gear-op1we.png";
  if (/\bop1\b/i.test(displayName)) return "/devices/endgame-gear-op1-8k.png";
  if (/\bviper\s*v2\s*pro\b/i.test(displayName)) return "/devices/razer-viper-v2-pro.png";
  if (/\bviper\s*mini\b/i.test(displayName)) return "/devices/razer-viper-mini.webp";
  if (/\bcobra\b/i.test(displayName)) return "/devices/razer-cobra.webp";
  if (/\bnape\s*pro\b/i.test(displayName)) return "/devices/keychron-nape-pro.png";
  if (/\bko-one\b/i.test(displayName)) return "/devices/crdrako-ko-one.png";
  if (/\br5\s*ultra\b/i.test(displayName)) return "/devices/attackshark-r5-ultra.png";
  if (/\battack\s*shark\s*x11\b/i.test(displayName)) return "/devices/attackshark-x11.png";
  if (/\bm[23]k\b/i.test(displayName)) return "/devices/zaunkoenig-m3k.png";
  if (/\bmx\s*master\s*3s\b/i.test(displayName)) return "/devices/logitech-mx-master-3s.png";
  if (/\bterra\s*pro\b/i.test(displayName)) return "/devices/teevolution-terra-pro.png";
  if (/\bm-001\b/i.test(displayName)) return "/devices/wallhack-m-001.png";
  if (/\bpulsefire\s*haste\b/i.test(displayName)) return "/devices/hyperx-pulsefire-haste.png";
  if (/\bk-001\b/i.test(displayName)) return "/devices/wallhack-k-001.png";
  // Pulsar 4K Wireless Receiver ships with the X2 V2 4K dongle kit; the
  // receiver product id is not yet published, so match the reported name.
  if (/pulsar/i.test(displayName)) return "/devices/pulsar-x2-v2.png";
  // Newer supported-model artwork resolved from the reported product name. These
  // run after the shared-receiver checks above but before the Pulsar/unknown
  // catch-alls. Test-needed (likely) models are deliberately left out.
  if (/\bg(?:102|203)\b/i.test(displayName)) return "/devices/logitech-g203.png";
  if (/\bg303\b/i.test(displayName)) return "/devices/logitech-g303.png";
  if (/\bg402\b/i.test(displayName)) return "/devices/logitech-g402.png";
  if (/\bg403\b/i.test(displayName)) return "/devices/logitech-g403.png";
  if (/\bg903\b/i.test(displayName)) return "/devices/logitech-g903.png";
  if (/\bg30[45]\b/i.test(displayName)) return "/devices/logitech-g305.png";
  if (/\bg309\b/i.test(displayName)) return "/devices/logitech-g309.png";
  if (/\bg\s*pro\s*2\b/i.test(displayName)) return "/devices/logitech-g-pro-2.png";
  if (/\bg\s*pro\b/i.test(displayName)) return "/devices/logitech-g-pro.png";
  if (/\bmx\s*anywhere\s*3\b/i.test(displayName)) return "/devices/logitech-mx-anywhere-3.png";
  if (/\bmx\s*ergo\b/i.test(displayName)) return "/devices/logitech-mx-ergo-s.png";
  if (/\bdeathadder\s*v4\b/i.test(displayName)) return "/devices/razer-deathadder-v4-pro.png";
  // V3 and V3 Pro are one shell (the Pro drops the cable), so the Pro shares
  // the V3 render like the V2 family does below. It was excluded while still
  // test-needed; verified on hardware since (mouse-protocol `0x00b7`).
  if (/\bdeathadder\s*v3\b/i.test(displayName)) return "/devices/razer-deathadder-v3.png";
  if (/\bdeathadder\s*v2\b(?!\s*x\s*hyperspeed\b)/i.test(displayName)) return "/devices/razer-deathadder-v2.png";
  if (/\bdeathadder\s*essential\b/i.test(displayName)) return "/devices/razer-deathadder-v2.png";
  if (/\bviper\s*v3\s*hyperspeed\b/i.test(displayName)) return "/devices/razer-viper-v3-hyperspeed.png";
  if (/\bviper\s*v4\b/i.test(displayName)) return "/devices/razer-viper-v4-pro.png";
  if (/\bxm2\s*8k\b/i.test(displayName)) return "/devices/endgame-gear-xm2-8k.png";
  if (/\bsword\s*x\b/i.test(displayName)) return "/devices/wlmouse-sword-x.png";
  if (/\bdragonfly\s*f2\b/i.test(displayName)) return "/devices/vgn-dragonfly-f2.png";
  if (/\bmaya\s*x\b/i.test(displayName)) return "/devices/lamzu-maya-x.png";
  if (/\bf1\s*v2\b/i.test(displayName)) return "/devices/atk-f1-v2-ultra-max.png";
  if (/\b(finalmouse|starlight|ulx)\b/i.test(displayName)) return "/devices/finalmouse-ulx.png";
  if (/\bbeast\s*max\b/i.test(displayName)) return "/devices/wlmouse-beast-max.png";
  return UNKNOWN_DEVICE_IMAGE;
}
