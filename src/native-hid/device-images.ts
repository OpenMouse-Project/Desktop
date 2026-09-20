/**
 * Product photos for the device list and device card, keyed by the same
 * `vendorId:productId` string `HidInterfaceInfo.key` / `ConnectedDevice.key`
 * already use (see `tauri-hid-device.ts`'s `key()` and hid.rs's
 * `interface_key()` — both hex-pad to `"0000:0000"`).
 *
 * Ported from the openmouse webapp's own `src/ui/device-images.ts` — same
 * lookup table, same name-pattern fallback for shared receiver ids, just
 * re-keyed onto this app's own identifiers since there's no WebHID
 * `HIDDevice` here to read vendor/product off of directly.
 *
 * Values are bare filenames resolved against the same `img.openmouse.app`
 * Cloudflare R2 bucket the webapp serves from (see `DEVICE_IMAGE_BASE_URL`
 * below), not files bundled with this app. Previously every asset was
 * checked into `public/devices/` and had to be hand-copied from the webapp
 * repo whenever new art landed there — the two copies drifted (this file's
 * DeathAdder V3 HyperSpeed entry kept resolving to the *corded* V3's photo
 * long after the webapp got a correct HyperSpeed-specific render, because
 * nothing forced the two lookup tables to agree). Pointing both apps at the
 * one hosted copy removes that class of bug entirely.
 */
const DEVICE_IMAGES: ReadonlyMap<string, string> = new Map([
  ["3710:5405", "pulsar-pro-dongle.png"],
  ["046d:c07d", "logitech-g502.png"],
  ["046d:c095", "logitech-g502-x-plus.png"],
  ["046d:c099", "logitech-g502-x.png"],
  ["046d:c0a8", "logitech-pro-x2-superstrike.png"],
  // Original G703 (0xc087) and G703 HERO wired (0xc090) share the same shell.
  ["046d:c087", "logitech-g703.png"],
  ["046d:c090", "logitech-g703.png"],
  // M3K and M2K use the supplied M3K product artwork.
  ["0483:a462", "zaunkoenig-m3k.png"],
  ["0483:a3cf", "zaunkoenig-m3k.png"],
  // Wired and receiver are separate product ids for the same mouse.
  ["1532:00a5", "razer-viper-v2-pro.png"],
  ["1532:00a6", "razer-viper-v2-pro.png"],
  ["1532:00c0", "razer-viper-v3-pro.png"],
  ["1532:00c1", "razer-viper-v3-pro.png"],
  ["1532:008a", "razer-viper-mini.webp"],
  ["1532:0078", "razer-viper.webp"],
  ["1532:00a3", "razer-cobra.webp"],
  // CRDRAKO KO-ONE wired and receiver transports share the same shell.
  ["373e:006a", "crdrako-ko-one.png"],
  ["373e:006b", "crdrako-ko-one.png"],
  // Attack Shark R5 Ultra wired and wireless transports share the same shell.
  ["373e:0046", "attackshark-r5-ultra.png"],
  ["373e:0047", "attackshark-r5-ultra.png"],
  // Attack Shark X11: wired (0xfa55) and the 2.4 GHz receiver (0xfa60) share
  // the same shell. 0xfa61 is the R1, a different mouse.
  ["1d57:fa55", "attackshark-x11.png"],
  ["1d57:fa60", "attackshark-x11.png"],
  // OP1 8K, Purple Frost, and v2. XM2 models use different shells.
  ["3367:1964", "endgame-gear-op1-8k.png"],
  ["3367:1976", "endgame-gear-op1-8k.png"],
  ["3367:1978", "endgame-gear-op1-8k.png"],
  // OP1we (same egg-we driver, 0x3367 family).
  ["3367:1961", "endgame-gear-op1we.png"],
  ["3367:1962", "endgame-gear-op1we.png"],
  // NinjaForce exposes separate wired and receiver ids for Sora V2, Sora V3,
  // and the TEN family. Receiver variants show the paired mouse artwork.
  ["1915:ae11", "ninjutso-sora-v2.png"],
  ["1915:ae12", "ninjutso-sora-v2.png"],
  ["1915:ae13", "ninjutso-sora-v2.png"],
  ["1915:ae14", "ninjutso-sora-v2.png"],
  ["1915:ae15", "ninjutso-sora-v2.png"],
  ["1915:ae16", "ninjutso-sora-v2.png"],
  ["1915:ae1c", "ninjutso-sora-v2.png"],
  ["1915:ae8a", "ninjutso-sora-v2.png"],
  ["1915:ae8c", "ninjutso-sora-v2.png"],
  ["093a:e010", "ninjutso-sora-v3.png"],
  ["093a:eb02", "ninjutso-sora-v3.png"],
  ["093a:e020", "ninjutso-ten.png"],
  ["093a:ea01", "ninjutso-ten.png"],
  ["093a:eb01", "ninjutso-ten.png"],
  // WLMouse Beast G receiver / wired transports share the same shell.
  ["36a7:a860", "wlmouse-beast-g.png"],
  ["36a7:a861", "wlmouse-beast-g.png"],
  // Beast Max wired / 4K8K receiver transports share the same shell.
  ["36a7:a880", "wlmouse-beast-max.png"],
  ["36a7:a881", "wlmouse-beast-max.png"],
  // Nape Pro wired / Link-KM receivers share the same shell artwork.
  ["3434:0440", "keychron-nape-pro.png"],
  ["3434:d026", "keychron-nape-pro.png"],
  ["3434:d029", "keychron-nape-pro.png"],
  // Teevolution Terra Pro wired / receiver Compx transports.
  ["3554:f520", "teevolution-terra-pro.png"],
  ["3554:f522", "teevolution-terra-pro.png"],
  ["3554:f523", "teevolution-terra-pro.png"],
  ["3554:f5bb", "teevolution-terra-pro.png"],
  // WALLHACK M-001 wireless mouse (real config id and in-app demo id).
  ["3879:1110", "wallhack-m-001.png"],
  ["3879:0807", "wallhack-m-001.png"],
  // WALLHACK K-001 analog keyboard (both enumerated vendor ids).
  ["3879:0806", "wallhack-k-001.png"],
  ["1caa:0806", "wallhack-k-001.png"],
  // Logitech G203 family. G203 LIGHTSYNC / PRODIGY and G102 share the same shell.
  ["046d:c084", "logitech-g203.png"],
  ["046d:c089", "logitech-g203.png"],
  ["046d:c092", "logitech-g203.png"],
  ["046d:c07e", "logitech-g402.png"],
  ["046d:c080", "logitech-g303.png"],
  ["046d:c08f", "logitech-g403.png"],
  ["046d:c08e", "logitech-g903.png"],
  // G Pro (2017), G Pro Hero, and G Pro Wireless share the same classic shell.
  ["046d:c085", "logitech-g-pro.png"],
  ["046d:c08c", "logitech-g-pro.png"],
  // Endgame Gear XM2 8K wired.
  ["3367:1966", "endgame-gear-xm2-8k.png"],
  ["3367:1980", "endgame-gear-xm2-8k.png"],
  // WLMouse Sword X wired / receiver transports keep their render.
  ["36a7:a878", "wlmouse-sword-x.png"],
  ["36a7:a879", "wlmouse-sword-x.png"],
  // VGN Dragonfly F2 Master+ wired / receiver transports.
  ["3554:fb56", "vgn-dragonfly-f2.png"],
  ["3554:fb57", "vgn-dragonfly-f2.png"],
  // Lamzu Maya X wired / wireless / 8K transports.
  ["373e:001c", "lamzu-maya-x.png"],
  ["373e:001d", "lamzu-maya-x.png"],
  ["373e:001e", "lamzu-maya-x.png"],
  ["1532:006e", "razer-deathadder-v2.png"],
  ["1532:0071", "razer-deathadder-v2.png"],
  ["1532:007c", "razer-deathadder-v2.png"],
  ["1532:007d", "razer-deathadder-v2.png"],
  ["1532:0084", "razer-deathadder-v2.png"],
  ["1532:0098", "razer-deathadder-v2.png"],
  // DeathAdder V4 Pro and its Carbon Fiber SKU share the same shell.
  ["1532:00be", "razer-deathadder-v4-pro.png"],
  ["1532:00bf", "razer-deathadder-v4-pro.png"],
  ["1532:00ef", "razer-deathadder-v4-pro.png"],
  ["1532:00f0", "razer-deathadder-v4-pro.png"],
  ["1532:00b8", "razer-viper-v3-hyperspeed.png"],
  ["1532:00e5", "razer-viper-v4-pro.png"],
  ["1532:00e6", "razer-viper-v4-pro.png"],
  // HyperX Pulsefire Haste: Kingston-era wired (0x0951:0x1727) and HP-era
  // wired / wired-mode / wireless dongle transports share one shell.
  ["0951:1727", "hyperx-pulsefire-haste.png"],
  ["03f0:0f8f", "hyperx-pulsefire-haste.png"],
  ["03f0:048e", "hyperx-pulsefire-haste.png"],
  ["03f0:028e", "hyperx-pulsefire-haste.png"],
]);

/**
 * Base URL of the public R2 bucket that hosts device art — the same bucket
 * and custom domain the openmouse webapp's `src/ui/device-images.ts` uses.
 */
const DEVICE_IMAGE_BASE_URL = "https://img.openmouse.app/";

/**
 * TEMPORARY local overrides for filenames this app still refers to that
 * haven't been uploaded to the R2 bucket yet (checked with `curl -o
 * /dev/null -w '%{http_code}'` against img.openmouse.app). Served from
 * `public/devices/`, which Vite copies into the build unprocessed. Delete an
 * entry (and its file under `public/devices/`) once its upload lands.
 */
const LOCAL_OVERRIDES: Readonly<Record<string, string>> = {
  "attackshark-x11.png": "/devices/attackshark-x11.png",
  "keychron-nape-pro.png": "/devices/keychron-nape-pro.png",
  "logitech-mx-master-4.png": "/devices/logitech-mx-master-4.png",
  "pulsar-pro-dongle.png": "/devices/pulsar-pro-dongle.png",
};

function imageUrl(filename: string): string {
  return LOCAL_OVERRIDES[filename] ?? DEVICE_IMAGE_BASE_URL + filename;
}

export const UNKNOWN_DEVICE_IMAGE = imageUrl("unknown-device.png");

/** Where the visible product sits inside its artwork, as canvas fractions. */
export interface ArtBounds {
  x: number;
  y: number;
  w: number;
  h: number;
  /** Canvas width / height, so a caller can reason about rendered width. */
  aspect: number;
}

/** Fraction of the (square) art box one device's artwork should occupy. */
const ART_TARGET = 0.5;

/** Cap on either dimension's share of the art box, so a subject can never be
 *  normalised right up against the tile's edges. */
const ART_MAX_EXTENT = 50;

/**
 * Where the product actually sits inside each artwork, as a fraction of the
 * canvas — precomputed offline (alpha > 24 counts as ink; a product shot's
 * last few fading pixels and any soft floor shadow don't) against the actual
 * files this app resolves to, both on `img.openmouse.app` and the local
 * overrides above. Keyed by filename, not URL, so it doesn't care whether a
 * given asset currently resolves to the CDN or a local override.
 *
 * This used to be measured at runtime by drawing each image to a canvas and
 * reading its alpha channel (`getImageData`) — accurate, but it read pixels
 * out of an `<img>` loaded from `img.openmouse.app`, a different origin than
 * this app. That CDN sends no `Access-Control-Allow-Origin` header, which
 * taints the canvas and makes `getImageData` throw a SecurityError; every
 * tile silently fell back to plain `contain` sizing (CONFIRMED: this is why
 * a Logitech Superlight tile rendered visibly larger than a DeathAdder V3
 * tile next to it — the DeathAdder's fallback-sized render happened to look
 * closer to "right" by coincidence, not because measuring worked). A static
 * table sidesteps CORS entirely and is exactly as accurate, since these
 * renders don't change after being uploaded.
 *
 * Regenerate by downloading each filename above and thresholding its alpha
 * channel's bounding box (`PIL.Image.split()[-1].point(...).getbbox()`, or
 * equivalent) — same INK_ALPHA cutoff `measureInk` used to use (24).
 */
const ART_BOUNDS: Readonly<Record<string, ArtBounds>> = {
  "atk-f1-v2-ultra-max.png": { x: 0.2534, y: 0.0339, w: 0.4932, h: 0.9322, aspect: 1 },
  "attackshark-r5-ultra.png": { x: 0.034, y: 0.02, w: 0.934, h: 0.9567, aspect: 0.5222 },
  "attackshark-x11.png": { x: 0.2637, y: 0.0137, w: 0.5078, h: 0.9688, aspect: 1 },
  "crdrako-ko-one.png": { x: 0.2915, y: 0.103, w: 0.417, h: 0.79, aspect: 1 },
  "endgame-gear-op1-8k.png": { x: 0.3242, y: 0, w: 0.335, h: 0.8475, aspect: 1 },
  "endgame-gear-op1we.png": { x: 0.3192, y: 0.1808, w: 0.345, h: 0.6758, aspect: 1 },
  "endgame-gear-xm2-8k.png": { x: 0.3212, y: 0.0339, w: 0.3576, h: 0.9321, aspect: 1.0008 },
  "finalmouse-ulx.png": { x: 0.2714, y: 0.0543, w: 0.4571, h: 0.8914, aspect: 1 },
  "keychron-nape-pro.png": { x: 0, y: 0.0007, w: 0.9975, h: 0.9993, aspect: 0.2661 },
  "lamzu-maya-x.png": { x: 0.2583, y: 0.0369, w: 0.4834, h: 0.9261, aspect: 1.0009 },
  "logitech-g-pro-2.png": { x: 0.2701, y: 0.0434, w: 0.4599, h: 0.9132, aspect: 1 },
  "logitech-g-pro.png": { x: 0.2712, y: 0.0339, w: 0.4585, h: 0.9322, aspect: 1 },
  "logitech-g203.png": { x: 0.2754, y: 0.0339, w: 0.4492, h: 0.9321, aspect: 1.0008 },
  "logitech-g303.png": { x: 0.2659, y: 0.0488, w: 0.4683, h: 0.9024, aspect: 1 },
  "logitech-g305.png": { x: 0.2407, y: 0.0339, w: 0.5186, h: 0.9322, aspect: 1 },
  "logitech-g309.png": { x: 0.2588, y: 0.0442, w: 0.4823, h: 0.9115, aspect: 1 },
  "logitech-g402.png": { x: 0.2697, y: 0.0366, w: 0.4616, h: 0.9269, aspect: 1 },
  "logitech-g403.png": { x: 0.2837, y: 0.0373, w: 0.4327, h: 0.9288, aspect: 1.0017 },
  "logitech-g502-x-plus.png": { x: 0.2457, y: 0.0714, w: 0.51, h: 0.8529, aspect: 1 },
  "logitech-g502-x.png": { x: 0.2729, y: 0.0914, w: 0.4543, h: 0.8371, aspect: 1 },
  "logitech-g502.png": { x: 0.3, y: 0.0743, w: 0.4, h: 0.8514, aspect: 1 },
  "logitech-g703.png": { x: 0.2671, y: 0.0714, w: 0.4657, h: 0.8571, aspect: 1 },
  "logitech-g903.png": { x: 0.265, y: 0.0347, w: 0.4699, h: 0.9314, aspect: 1.0008 },
  "logitech-mx-anywhere-3.png": { x: 0.2, y: 0.0339, w: 0.6, h: 0.9314, aspect: 1 },
  "logitech-mx-ergo-s.png": { x: 0.149, y: 0.0339, w: 0.7019, h: 0.9322, aspect: 1.0008 },
  "logitech-mx-master-3s.png": { x: 0.0454, y: 0.0968, w: 0.8865, h: 0.8013, aspect: 0.6092 },
  "logitech-mx-master-4.png": { x: 0.0214, y: 0.0176, w: 0.9573, h: 0.9647, aspect: 0.6882 },
  "logitech-pro-x-superlight-2c.png": { x: 0.1569, y: 0.084, w: 0.6862, h: 0.833, aspect: 0.631 },
  "logitech-pro-x2-superstrike.png": { x: 0.3952, y: 0.1299, w: 0.2102, h: 0.7405, aspect: 1.778 },
  "ninjutso-sora-v2.png": { x: 0.3187, y: 0.155, w: 0.3625, h: 0.6937, aspect: 1 },
  "ninjutso-sora-v3.png": { x: 0.3175, y: 0.1537, w: 0.3638, h: 0.6937, aspect: 1 },
  "ninjutso-ten.png": { x: 0.3162, y: 0.1375, w: 0.37, h: 0.725, aspect: 1 },
  "pulsar-pro-dongle.png": { x: 0.3493, y: 0.246, w: 0.2647, h: 0.4347, aspect: 1 },
  "pulsar-x2-v2.png": { x: 0.245, y: 0.025, w: 0.5112, h: 0.9513, aspect: 1 },
  "razer-cobra.webp": { x: 0.31, y: 0.01, w: 0.3883, h: 0.9133, aspect: 1 },
  "razer-deathadder-v2.png": { x: 0.2854, y: 0.051, w: 0.428, h: 0.8967, aspect: 1.0013 },
  "razer-deathadder-v3-hyperspeed.png": { x: 0.2855, y: 0.0729, w: 0.4291, h: 0.8543, aspect: 1.0018 },
  "razer-deathadder-v3.png": { x: 0.31, y: 0.1367, w: 0.3833, h: 0.7267, aspect: 1 },
  "razer-deathadder-v4-pro.png": { x: 0.2828, y: 0.0905, w: 0.4344, h: 0.819, aspect: 1 },
  "razer-viper-mini.webp": { x: 0.3281, y: 0.1953, w: 0.3584, h: 0.7139, aspect: 1 },
  "razer-viper-v2-pro.png": { x: 0.295, y: 0.1383, w: 0.415, h: 0.7817, aspect: 1 },
  "razer-viper-v3-pro.png": { x: 0.29, y: 0.0833, w: 0.42, h: 0.8333, aspect: 1 },
  "razer-viper-v4-pro.png": { x: 0.2944, y: 0.0899, w: 0.4112, h: 0.8202, aspect: 1 },
  "razer-viper.webp": { x: 0.0105, y: 0.0907, w: 0.9789, h: 0.904, aspect: 0.4213 },
  "teevolution-terra-pro.png": { x: 0.2975, y: 0.13, w: 0.4037, h: 0.7412, aspect: 1 },
  "unknown-device.png": { x: 0.2013, y: 0.138, w: 0.5975, h: 0.725, aspect: 0.631 },
  "vgn-dragonfly-f2.png": { x: 0.2547, y: 0.0373, w: 0.4907, h: 0.9254, aspect: 1 },
  "wallhack-k-001.png": { x: 0, y: 0.1622, w: 0.985, h: 0.7933, aspect: 1.7778 },
  "wlmouse-beast-g.png": { x: 0.1601, y: 0.078, w: 0.683, h: 0.837, aspect: 0.631 },
  "wlmouse-beast-max.png": { x: 0.0183, y: 0, w: 0.9634, h: 1, aspect: 0.5365 },
  "wlmouse-sword-x.png": { x: 0.2584, y: 0.0364, w: 0.4832, h: 0.9272, aspect: 1 },
  "zaunkoenig-m3k.png": { x: 0.2375, y: 0, w: 0.525, h: 0.8413, aspect: 1 },
};

/**
 * Where the product actually is inside one artwork, as a fraction of the
 * canvas. Synchronous now that it's a lookup rather than a canvas
 * measurement — no image to wait for, so a caller no longer needs to render
 * once with a fallback style before the "real" size arrives.
 */
export function artBounds(filename: string): ArtBounds | null {
  return ART_BOUNDS[filename] ?? null;
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
 * The retry is the point. A single transient failure — a network hiccup
 * against `img.openmouse.app`, a request that raced the CDN's own cache
 * warming — used to latch the placeholder onto that `<img>` for the rest of
 * the page load, so a mouse whose artwork genuinely exists showed the
 * generic tile and looked unsupported. The retry carries a query string so
 * the browser cannot reuse its cached failure for that URL.
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
function resolveDeviceImageFilename(key: string | null | undefined, displayName: string): string {
  const mapped = key ? DEVICE_IMAGES.get(key) ?? null : null;
  if (mapped) return mapped;
  // Lightspeed receivers are shared product IDs, so paired G502 X variants
  // must use the friendly name read from the mouse itself.
  if (/g502\s*x\s*plus/i.test(displayName)) return "logitech-g502-x-plus.png";
  if (/g502\s*x/i.test(displayName)) return "logitech-g502-x.png";
  if (/\bg502\b/i.test(displayName)) return "logitech-g502.png";
  if (/\bg703\b/i.test(displayName)) return "logitech-g703.png";
  if (/mx\s*master\s*4/i.test(displayName)) return "logitech-mx-master-4.png";
  if (/superstrike/i.test(displayName)) return "logitech-pro-x2-superstrike.png";
  if (/superlight/i.test(displayName)) return "logitech-pro-x-superlight-2c.png";
  if (/op1we/i.test(displayName)) return "endgame-gear-op1we.png";
  if (/\bop1\b/i.test(displayName)) return "endgame-gear-op1-8k.png";
  if (/\bviper\s*v2\s*pro\b/i.test(displayName)) return "razer-viper-v2-pro.png";
  if (/\bviper\s*mini\b/i.test(displayName)) return "razer-viper-mini.webp";
  if (/\bcobra\b/i.test(displayName)) return "razer-cobra.webp";
  if (/\bnape\s*pro\b/i.test(displayName)) return "keychron-nape-pro.png";
  if (/\bko-one\b/i.test(displayName)) return "crdrako-ko-one.png";
  if (/\br5\s*ultra\b/i.test(displayName)) return "attackshark-r5-ultra.png";
  if (/\battack\s*shark\s*x11\b/i.test(displayName)) return "attackshark-x11.png";
  if (/\bm[23]k\b/i.test(displayName)) return "zaunkoenig-m3k.png";
  if (/\bmx\s*master\s*3s\b/i.test(displayName)) return "logitech-mx-master-3s.png";
  if (/\bterra\s*pro\b/i.test(displayName)) return "teevolution-terra-pro.png";
  if (/\bm-001\b/i.test(displayName)) return "wallhack-m-001.png";
  if (/\bpulsefire\s*haste\b/i.test(displayName)) return "hyperx-pulsefire-haste.png";
  if (/\bk-001\b/i.test(displayName)) return "wallhack-k-001.png";
  // Pulsar 4K Wireless Receiver ships with the X2 V2 4K dongle kit; the
  // receiver product id is not yet published, so match the reported name.
  if (/pulsar/i.test(displayName)) return "pulsar-x2-v2.png";
  // Newer supported-model artwork resolved from the reported product name. These
  // run after the shared-receiver checks above but before the Pulsar/unknown
  // catch-alls. Test-needed (likely) models are deliberately left out.
  if (/\bg(?:102|203)\b/i.test(displayName)) return "logitech-g203.png";
  if (/\bg303\b/i.test(displayName)) return "logitech-g303.png";
  if (/\bg402\b/i.test(displayName)) return "logitech-g402.png";
  if (/\bg403\b/i.test(displayName)) return "logitech-g403.png";
  if (/\bg903\b/i.test(displayName)) return "logitech-g903.png";
  if (/\bg30[45]\b/i.test(displayName)) return "logitech-g305.png";
  if (/\bg309\b/i.test(displayName)) return "logitech-g309.png";
  if (/\bg\s*pro\s*2\b/i.test(displayName)) return "logitech-g-pro-2.png";
  if (/\bg\s*pro\b/i.test(displayName)) return "logitech-g-pro.png";
  if (/\bmx\s*anywhere\s*3\b/i.test(displayName)) return "logitech-mx-anywhere-3.png";
  if (/\bmx\s*ergo\b/i.test(displayName)) return "logitech-mx-ergo-s.png";
  if (/\bdeathadder\s*v4\b/i.test(displayName)) return "razer-deathadder-v4-pro.png";
  // DeathAdder V3 Pro and HyperSpeed are the wireless SKUs of the V3 shell —
  // this must be checked before the plain "v3" match below, or "V3
  // HyperSpeed" would match that first and show the *corded* V3's photo on a
  // wireless mouse (CONFIRMED: exactly this happened before this line
  // existed here — see the webapp's `src/ui/device-images.ts`, which already
  // had this split).
  if (/\bdeathadder\s*v3\s*(?:pro|hyperspeed)\b/i.test(displayName)) return "razer-deathadder-v3-hyperspeed.png";
  if (/\bdeathadder\s*v3\b/i.test(displayName)) return "razer-deathadder-v3.png";
  if (/\bdeathadder\s*v2\b(?!\s*x\s*hyperspeed\b)/i.test(displayName)) return "razer-deathadder-v2.png";
  if (/\bdeathadder\s*essential\b/i.test(displayName)) return "razer-deathadder-v2.png";
  if (/\bviper\s*v3\s*hyperspeed\b/i.test(displayName)) return "razer-viper-v3-hyperspeed.png";
  if (/\bviper\s*v4\b/i.test(displayName)) return "razer-viper-v4-pro.png";
  if (/\bxm2\s*8k\b/i.test(displayName)) return "endgame-gear-xm2-8k.png";
  if (/\bsword\s*x\b/i.test(displayName)) return "wlmouse-sword-x.png";
  if (/\bdragonfly\s*f2\b/i.test(displayName)) return "vgn-dragonfly-f2.png";
  if (/\bmaya\s*x\b/i.test(displayName)) return "lamzu-maya-x.png";
  if (/\bf1\s*v2\b/i.test(displayName)) return "atk-f1-v2-ultra-max.png";
  if (/\b(finalmouse|starlight|ulx)\b/i.test(displayName)) return "finalmouse-ulx.png";
  if (/\bbeast\s*max\b/i.test(displayName)) return "wlmouse-beast-max.png";
  return "unknown-device.png";
}

/**
 * Best-known product photo for a device. `key` is `HidInterfaceInfo.key` /
 * `ConnectedDevice.key`; `displayName` is the product/friendly name (from
 * `HidInterfaceInfo.productString` or `MouseStatus.name`) — used as a
 * fallback for shared receiver product ids, same as the webapp.
 */
export function deviceImage(key: string | null | undefined, displayName = ""): string {
  return imageUrl(resolveDeviceImageFilename(key, displayName));
}

/**
 * The bare filename a device's artwork resolves to (no bucket URL). Lets a
 * caller — currently just DeviceTile, for its ART_BOUNDS lookup — resolve
 * the same filename `deviceImage` did without re-deriving it or reaching
 * into this module's URL-building.
 */
export function deviceImageFilename(key: string | null | undefined, displayName = ""): string {
  return resolveDeviceImageFilename(key, displayName);
}
