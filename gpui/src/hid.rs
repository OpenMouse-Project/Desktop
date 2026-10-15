//! Device support for the GPUI app.
//!
//! The mouse protocols live in `@openmouse/protocol` (TypeScript, shared with
//! the web app), so they are not ported: `js/build.mjs` bundles that package
//! plus the Tauri app's own `src/native-hid/{scan,brands,tauri-hid-device}.ts`
//! — unmodified — and this module runs the bundle in an embedded QuickJS
//! runtime. The only things the bundle needs from the host are what the Tauri
//! app gets from its webview: the `hid_*` commands (served here by the shared
//! `openmouse-hid` crate, the same transport the Tauri app uses) and the
//! `hid-input-report` event.
//!
//! Cost model: nothing runs until `HidService` is first used, then one thread
//! owns the runtime. It sleeps on a channel while idle, and each HID call is a
//! short-lived thread, so a connected-but-quiet app does no polling beyond the
//! reader threads `openmouse-hid` already keeps per open interface.

use std::sync::Arc;
use std::time::Duration;

use futures::channel::{mpsc, oneshot};
use futures::{FutureExt, StreamExt};
use openmouse_hid as core;
use rquickjs::function::Async;
use rquickjs::promise::Promise;
use rquickjs::{AsyncContext, AsyncRuntime, Ctx, Exception, Function, Object};
use serde::Deserialize;
use serde_json::Value;

const BUNDLE: &str = include_str!("../js/dist/bundle.js");

#[derive(Debug, Clone, Deserialize)]
pub struct Interface {
    pub key: String,
    #[serde(rename = "vendorId")]
    pub vendor_id: u16,
    #[serde(rename = "productId")]
    pub product_id: u16,
    #[serde(rename = "productString")]
    pub product_string: String,
    #[serde(rename = "manufacturerString")]
    pub manufacturer_string: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Candidate {
    pub info: Interface,
    pub brands: Vec<String>,
}

/// Where the visible mouse sits in its artwork, pre-normalised by the shared
/// `device-images.ts` so every model renders at a comparable size.
#[derive(Debug, Clone, Deserialize)]
pub struct ArtPlacement {
    /// Image height as a percentage of the (square) art box.
    pub height: f32,
    /// Canvas width / height.
    pub aspect: f32,
    /// Subject-centering shift, as a percentage of the image's own size.
    #[serde(rename = "offsetX")]
    pub offset_x: f32,
    #[serde(rename = "offsetY")]
    pub offset_y: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceArt {
    pub filename: String,
    pub url: String,
    pub art: Option<ArtPlacement>,
}

impl DeviceArt {
    /// Local file for this artwork, downloading it first if needed. Blocking
    /// (curl, like the game art); call off the UI thread.
    pub fn fetch(&self) -> Option<std::path::PathBuf> {
        self.fetch_original().map(|path| Self::shrunk(path, &self.filename))
    }

    /// Artwork can be several times larger than it is ever drawn (2500 px for a
    /// ~350 px slot). The GPU's plain bilinear downscale of that looks grainy with
    /// jagged edges, so big images are resampled once, with a proper filter, to a
    /// cached copy at a size that still covers a Retina display.
    fn shrunk(path: std::path::PathBuf, filename: &str) -> std::path::PathBuf {
        const LONGEST: u32 = 1024;
        let Some(dir) = dirs::cache_dir().map(|d| d.join("openmouse").join("device-art")) else { return path };
        let small = dir.join(format!("{}@{LONGEST}p.png", filename.trim_end_matches(".png")));
        if small.is_file() {
            return small;
        }
        let Ok(img) = image::open(&path) else { return path };
        if img.width().max(img.height()) <= LONGEST {
            return path;
        }
        // Resample with premultiplied alpha: filtering straight-alpha pixels mixes in
        // the colour hidden under transparent ones and leaves a dark/light fringe on
        // the silhouette.
        let mut rgba = img.to_rgba8();
        for px in rgba.pixels_mut() {
            let a = px[3] as u32;
            for c in &mut px.0[..3] {
                *c = ((*c as u32 * a + 127) / 255) as u8;
            }
        }
        let scale = LONGEST as f32 / rgba.width().max(rgba.height()) as f32;
        let (w, h) = (((rgba.width() as f32 * scale).round() as u32).max(1), ((rgba.height() as f32 * scale).round() as u32).max(1));
        let mut resized = image::imageops::resize(&rgba, w, h, image::imageops::FilterType::Lanczos3);
        for px in resized.pixels_mut() {
            let a = px[3] as u32;
            if a > 0 && a < 255 {
                for c in &mut px.0[..3] {
                    *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
                }
            }
        }
        let tmp = small.with_extension("part.png");
        if resized.save(&tmp).is_ok() && std::fs::rename(&tmp, &small).is_ok() {
            small
        } else {
            let _ = std::fs::remove_file(&tmp);
            path
        }
    }

    fn fetch_original(&self) -> Option<std::path::PathBuf> {
        let dir = dirs::cache_dir()?.join("openmouse").join("device-art");
        let _ = std::fs::create_dir_all(&dir);
        let target = dir.join(&self.filename);
        if target.is_file() {
            return Some(target);
        }
        // Art not yet on the CDN is served from public/devices (the Tauri
        // app's LOCAL_OVERRIDES); in this app that is a dev-tree path.
        if self.url.starts_with("/devices/") {
            let local = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../public")
                .join(self.url.trim_start_matches('/'));
            return local.is_file().then_some(local);
        }
        let tmp = dir.join(format!("{}.part", self.filename));
        let ok = crate::proc::quiet(
            std::process::Command::new("curl")
                .args(["-sSfL", "-m", "20", "-o"])
                .arg(&tmp)
                .arg(&self.url)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null()),
        )
        .status()
        .is_ok_and(|s| s.success());
        if ok && tmp.is_file() && std::fs::rename(&tmp, &target).is_ok() {
            Some(target)
        } else {
            let _ = std::fs::remove_file(&tmp);
            None
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DpiBounds {
    pub min: f64,
    pub max: f64,
    pub step: f64,
}

/// What the device page may offer, resolved by the shared JS (per-product DPI
/// limits, the preset list, valid surface modes).
#[derive(Debug, Clone, Deserialize)]
pub struct Controls {
    pub dpi: DpiBounds,
    #[serde(rename = "dpiPresets")]
    pub dpi_presets: Vec<f64>,
    #[serde(rename = "surfaceModes")]
    pub surface_modes: Vec<String>,
}

/// What reading a Logitech mouse's stored profiles returns: the profiles, and
/// every action a button can be given on this protocol.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct OnboardProfiles {
    pub profiles: Vec<OnboardProfile>,
    #[serde(default)]
    pub actions: Vec<String>,
}

/// A Logitech onboard profile, as decoded by the shared protocol library.
#[derive(Debug, Clone, Deserialize)]
pub struct OnboardProfile {
    pub sector: u32,
    pub enabled: bool,
    #[serde(rename = "isCurrent")]
    pub is_current: bool,
    #[serde(default)]
    pub name: Option<String>,
    /// Each stage is the library's `DpiStage`; the page reads its `dpi`.
    #[serde(rename = "dpiStages", default)]
    pub dpi_stages: Vec<Value>,
    #[serde(rename = "defaultDpiIndex", default)]
    pub default_dpi_index: Option<u32>,
    #[serde(rename = "reportRateWireless", default)]
    pub report_rate_wireless: Option<u32>,
    #[serde(rename = "reportRateWired", default)]
    pub report_rate_wired: Option<u32>,
    #[serde(rename = "buttonAssignments", default)]
    pub button_assignments: Vec<Value>,
    #[serde(rename = "crcValid", default = "yes")]
    pub crc_valid: bool,
}

fn yes() -> bool {
    true
}

/// One selectable value of a [`ControlSpec`].
#[derive(Debug, Clone, Deserialize)]
pub struct OptionSpec {
    pub value: Value,
    pub label: String,
}

/// A Buttons/Advanced control, described by `js/controls.ts`. `current` is the
/// value as last read (or last written); `kind` is `choice`, `toggle`,
/// `number` or `info`.
#[derive(Debug, Clone, Deserialize)]
pub struct ControlSpec {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
    pub kind: String,
    #[serde(default)]
    pub options: Vec<OptionSpec>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    /// How far a number control moves per click (1 when absent).
    #[serde(default)]
    pub step: Option<f64>,
    /// Digits after the point to show and round to (0 when absent).
    #[serde(default)]
    pub decimals: Option<usize>,
    /// Unit shown after a number control's value.
    #[serde(default)]
    pub unit: Option<String>,
    /// Shown instead of the number when it sits at the minimum (a slider whose lowest stop is "Off").
    #[serde(rename = "zeroLabel", default)]
    pub zero_label: Option<String>,
    #[serde(default)]
    pub current: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Section {
    pub title: String,
    pub controls: Vec<ControlSpec>,
}

/// The device's lighting, in the protocol library's `MouseLighting` shape.
#[derive(Debug, Clone, Deserialize)]
pub struct LightingSpec {
    pub modes: Vec<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub color2: Option<String>,
    #[serde(rename = "colorModes", default)]
    pub color_modes: Vec<String>,
    #[serde(rename = "dualColorModes", default)]
    pub dual_color_modes: Vec<String>,
    #[serde(rename = "reactiveModes", default)]
    pub reactive_modes: Vec<String>,
    #[serde(default)]
    pub speeds: Vec<f64>,
    #[serde(default)]
    pub speed: Option<f64>,
    #[serde(rename = "writeOnly", default)]
    pub write_only: bool,
}

/// Everything the Buttons, Lighting and Advanced tabs show for a device.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Tabs {
    /// The Performance tab's controls beyond DPI / polling / lift-off / surface.
    #[serde(default)]
    pub performance: Vec<Section>,
    #[serde(default)]
    pub buttons: Vec<Section>,
    #[serde(default)]
    pub advanced: Vec<Section>,
    #[serde(default)]
    pub lighting: Option<LightingSpec>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConnectedDevice {
    /// The brand to show a person: the product's own, which can differ from
    /// `brand` (the registry's label for the driver that answered).
    #[serde(rename = "displayBrand", default)]
    pub display_brand: Option<String>,
    pub tabs: Option<Tabs>,
    pub controls: Option<Controls>,
    pub key: String,
    pub brand: String,
    pub driver: String,
    /// The protocol library's `MouseStatus`, as it came out of the driver.
    pub status: Value,
}

enum Msg {
    Call {
        function: &'static str,
        arg: Option<String>,
        reply: oneshot::Sender<Result<String, String>>,
    },
    InputReport(core::HidInputReportPayload),
}

#[derive(Clone)]
pub struct HidService {
    tx: mpsc::UnboundedSender<Msg>,
}

impl HidService {
    /// Starts the runtime thread. It loads the bundle before serving the first
    /// request, so construction itself is cheap.
    pub fn start() -> Self {
        let (tx, rx) = mpsc::unbounded();
        let sink_tx = tx.clone();
        std::thread::Builder::new()
            .name("openmouse-hid-js".into())
            .spawn(move || {
                futures::executor::block_on(async move {
                    if let Err(error) = run(rx, sink_tx).await {
                        eprintln!("[hid] js runtime stopped: {error}");
                    }
                })
            })
            .expect("spawn hid js thread");
        Self { tx }
    }

    /// Every connected interface at least one known driver might answer on.
    /// Read-only; opens nothing.
    pub fn list_candidates(&self) -> oneshot::Receiver<Result<Vec<Candidate>, String>> {
        self.call("listCandidates", None, |json| {
            serde_json::from_str(&json).map_err(|e| e.to_string())
        })
    }

    /// Product artwork for a device, from the same lookup the Tauri app uses.
    /// `name` is the name the device reports (receivers share product ids).
    pub fn device_image(&self, key: &str, name: &str) -> oneshot::Receiver<Result<DeviceArt, String>> {
        let args = serde_json::to_string(&[key, name]).unwrap_or_default();
        self.call("deviceImage", Some(args), |json| {
            serde_json::from_str(&json).map_err(|e| e.to_string())
        })
    }

    /// The Logitech mouse's stored (onboard) profiles. Read on demand — a full
    /// pass is dozens of round trips.
    pub fn onboard_profiles(&self, key: &str) -> oneshot::Receiver<Result<OnboardProfiles, String>> {
        let args = serde_json::json!({ "key": key });
        self.call("onboardProfiles", Some(args.to_string()), |json| {
            serde_json::from_str(&json).map_err(|e| e.to_string())
        })
    }

    /// `useProfile` (value: the profile's sector) or `mode` (value: "Onboard" / "Host").
    pub fn onboard_action(&self, key: &str, action: &str, value: Value) -> oneshot::Receiver<Result<Value, String>> {
        let args = serde_json::json!({ "key": key, "action": action, "value": value });
        self.call("onboardAction", Some(args.to_string()), |json| {
            serde_json::from_str(&json).map_err(|e| e.to_string())
        })
    }

    /// Applies a game profile (`profiles::GameProfile::to_js`) in one device
    /// session, through the same `applyGameProfile` the Tauri app uses.
    /// Resolves to the values the mouse applied.
    pub fn apply_profile(&self, key: &str, profile: Value) -> oneshot::Receiver<Result<Value, String>> {
        let args = serde_json::json!({ "key": key, "profile": profile });
        self.call("applyProfile", Some(args.to_string()), |json| {
            serde_json::from_str(&json).map_err(|e| e.to_string())
        })
    }

    /// Writes one live setting and resolves to the value the device applied.
    /// `action` is one of `dpi`, `pollingRate`, `liftOff`, `surface`.
    pub fn write(
        &self,
        key: &str,
        brand: &str,
        action: &str,
        value: Value,
    ) -> oneshot::Receiver<Result<Value, String>> {
        let args = serde_json::json!({ "key": key, "brand": brand, "action": action, "value": value });
        self.call("write", Some(args.to_string()), |json| {
            serde_json::from_str(&json).map_err(|e| e.to_string())
        })
    }

    /// Opens one listed interface, finds the driver that answers and reads its
    /// status once.
    pub fn connect(&self, key: &str) -> oneshot::Receiver<Result<ConnectedDevice, String>> {
        self.call("connect", Some(key.to_string()), |json| {
            serde_json::from_str(&json).map_err(|e| e.to_string())
        })
    }

    fn call<T: Send + 'static>(
        &self,
        function: &'static str,
        arg: Option<String>,
        parse: impl FnOnce(String) -> Result<T, String> + Send + 'static,
    ) -> oneshot::Receiver<Result<T, String>> {
        let (reply, raw) = oneshot::channel();
        let (out_tx, out_rx) = oneshot::channel();
        let _ = self.tx.unbounded_send(Msg::Call { function, arg, reply });
        std::thread::spawn(move || {
            let result = match futures::executor::block_on(raw) {
                Ok(Ok(json)) => parse(json),
                Ok(Err(error)) => Err(error),
                Err(_) => Err("device runtime stopped".to_string()),
            };
            let _ = out_tx.send(result);
        });
        out_rx
    }
}

async fn run(
    mut rx: mpsc::UnboundedReceiver<Msg>,
    sink_tx: mpsc::UnboundedSender<Msg>,
) -> Result<(), String> {
    let rt = AsyncRuntime::new().map_err(|e| e.to_string())?;
    let ctx = AsyncContext::full(&rt).await.map_err(|e| e.to_string())?;

    let api = Arc::new(core::HidApiHandle::default());
    let registry = Arc::new(core::HidRegistry::default());
    let sink: core::ReportSink = Arc::new(move |payload| {
        let _ = sink_tx.unbounded_send(Msg::InputReport(payload));
    });

    ctx.with(|ctx| {
        install_host(&ctx, api, registry, sink)?;
        // OPENMOUSE_MOCK=slx,x3 adds fake devices (no hardware) for looking at their UI.
        if let Ok(wanted) = std::env::var("OPENMOUSE_MOCK") {
            let wanted: String = wanted.chars().filter(|c| c.is_ascii_alphanumeric() || *c == ',').collect();
            ctx.eval::<(), _>(format!("globalThis.__mock = '{wanted}';"))?;
        }
        ctx.eval::<(), _>(BUNDLE)
    })
        .await
        .map_err(|e| e.to_string())?;

    loop {
        // Drive timers and promise continuations until there is nothing left,
        // but wake for a message as soon as one arrives. Once idle, block on
        // the channel alone so a quiet app costs nothing.
        let msg = futures::select_biased! {
            msg = rx.next().fuse() => msg,
            () = rt.idle().fuse() => rx.next().await,
        };
        let Some(msg) = msg else { return Ok(()) };
        ctx.with(|ctx| handle(&ctx, msg)).await;
    }
}

fn handle(ctx: &Ctx<'_>, msg: Msg) {
    match msg {
        Msg::InputReport(payload) => {
            let Ok(json) = serde_json::to_string(&payload) else { return };
            let result: rquickjs::Result<()> = (|| {
                let dispatch: Function = ctx.globals().get("__dispatch")?;
                dispatch.call::<_, ()>(("hid-input-report", json))
            })();
            if let Err(error) = result {
                eprintln!("[hid] input report dispatch failed: {error}");
            }
        }
        Msg::Call { function, arg, reply } => {
            let started: rquickjs::Result<Promise> = (|| {
                let api: Object = ctx.globals().get("__api")?;
                let function: Function = api.get(function)?;
                match arg {
                    Some(arg) => function.call((arg,)),
                    None => function.call(()),
                }
            })();
            match started {
                Ok(promise) => {
                    let ctx = ctx.clone();
                    ctx.clone().spawn(async move {
                        let result = promise.into_future::<String>().await;
                        // A rejection leaves the JS error on the context; the
                        // Rust-side error alone is just "Exception generated".
                        let _ = reply.send(result.map_err(|error| describe(&ctx, error)));
                    })
                }
                Err(error) => {
                    let _ = reply.send(Err(error.to_string()));
                }
            }
        }
    }
}

fn describe(ctx: &Ctx<'_>, error: rquickjs::Error) -> String {
    if !matches!(error, rquickjs::Error::Exception) {
        return error.to_string();
    }
    let thrown = ctx.catch();
    if let Some(exception) = thrown.clone().into_exception() {
        if let Some(message) = exception.message() {
            return message;
        }
    }
    thrown
        .get::<String>()
        .unwrap_or_else(|_| "unknown JS error".to_string())
}

fn install_host<'js>(
    ctx: &Ctx<'js>,
    api: Arc<core::HidApiHandle>,
    registry: Arc<core::HidRegistry>,
    sink: core::ReportSink,
) -> rquickjs::Result<()> {
    // Everything that touches `HidApi` (enumerate, open) must stay on ONE
    // long-lived thread: on macOS the IOHID manager is bound to the run loop
    // of the thread that created it, and refreshing from a different (or
    // since-exited) thread aborts inside IOHIDDeviceScheduleWithRunLoop.
    // Reads and writes on an already-open handle are thread-safe and keep
    // using a thread per call.
    type Job = Box<dyn FnOnce() + Send>;
    let (api_jobs, api_rx) = std::sync::mpsc::channel::<Job>();
    std::thread::Builder::new()
        .name("openmouse-hid-api".into())
        .spawn(move || {
            while let Ok(job) = api_rx.recv() {
                job();
            }
        })
        .map_err(|_| rquickjs::Error::Unknown)?;

    let host = Object::new(ctx.clone())?;
    host.set(
        "platform",
        if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "macos") {
            "macos"
        } else {
            "linux"
        },
    )?;
    host.set(
        "log",
        Function::new(ctx.clone(), |line: String| eprintln!("{line}"))?,
    )?;
    host.set(
        "sleep",
        Function::new(
            ctx.clone(),
            Async(|ms: f64| async move {
                futures_timer::Delay::new(Duration::from_millis(ms.max(0.0) as u64)).await;
                rquickjs::Result::Ok(())
            }),
        )?,
    )?;
    host.set(
        "invoke",
        Function::new(
            ctx.clone(),
            Async(move |ctx: Ctx<'js>, command: String, args: String| {
                let (api, registry, sink) = (api.clone(), registry.clone(), sink.clone());
                let api_jobs = api_jobs.clone();
                async move {
                    // HID calls block (and some wait on the device), so they run
                    // off the runtime thread; the promise resolves when done.
                    let (done, wait) = oneshot::channel();
                    let pinned = matches!(command.as_str(), "hid_list_interfaces" | "hid_open");
                    let job = move || {
                        let _ = done.send(dispatch(&api, &registry, &sink, &command, &args));
                    };
                    if pinned {
                        let _ = api_jobs.send(Box::new(job));
                    } else {
                        std::thread::spawn(job);
                    }
                    match wait.await {
                        Ok(Ok(json)) => Ok(json),
                        Ok(Err(message)) => Err(Exception::throw_message(&ctx, &message)),
                        Err(_) => Err(Exception::throw_message(&ctx, "device call dropped")),
                    }
                }
            }),
        )?,
    )?;
    ctx.globals().set("__host", host)
}

/// The Tauri `hid_*` command surface, by name, with Tauri's camelCase
/// argument keys — what `src/native-hid/tauri-hid-device.ts` sends.
fn dispatch(
    api: &core::HidApiHandle,
    registry: &core::HidRegistry,
    sink: &core::ReportSink,
    command: &str,
    args: &str,
) -> Result<String, String> {
    let args: Value = serde_json::from_str(args).map_err(|e| e.to_string())?;
    let num = |key: &str| -> Result<u64, String> {
        args.get(key)
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("{command}: missing `{key}`"))
    };
    let opt = |key: &str| args.get(key).and_then(Value::as_u64).map(|v| v as u16);
    let bytes = || -> Result<Vec<u8>, String> {
        args.get("data")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_u64).map(|b| b as u8).collect())
            .ok_or_else(|| format!("{command}: missing `data`"))
    };
    let ids = || -> Result<(u16, u16), String> { Ok((num("vendorId")? as u16, num("productId")? as u16)) };
    let json = |v: &dyn erased::Ser| v.to_json();

    match command {
        "hid_list_interfaces" => {
            let vendor_ids: Vec<u16> = args
                .get("vendorIds")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_u64).map(|v| v as u16).collect())
                .unwrap_or_default();
            json(&core::hid_list_interfaces(api, vendor_ids)?)
        }
        "hid_open" => {
            let (v, p) = ids()?;
            core::hid_open(
                sink.clone(),
                api,
                registry,
                v,
                p,
                opt("preferredUsagePage"),
                opt("preferredUsage"),
            )?;
            Ok(String::new())
        }
        "hid_close" => {
            let (v, p) = ids()?;
            core::hid_close(registry, v, p)?;
            Ok(String::new())
        }
        "hid_send_report" => {
            let (v, p) = ids()?;
            core::hid_send_report(registry, v, p, num("reportId")? as u8, bytes()?)?;
            Ok(String::new())
        }
        "hid_send_feature_report" => {
            let (v, p) = ids()?;
            core::hid_send_feature_report(registry, v, p, num("reportId")? as u8, bytes()?)?;
            Ok(String::new())
        }
        "hid_get_feature_report" => {
            let (v, p) = ids()?;
            json(&core::hid_get_feature_report(
                registry, v, p, num("reportId")? as u8, num("length")? as usize,
            )?)
        }
        "hid_get_input_report" => {
            let (v, p) = ids()?;
            json(&core::hid_get_input_report(
                registry, v, p, num("reportId")? as u8, num("length")? as usize,
            )?)
        }
        other => Err(format!("unknown command `{other}`")),
    }
}

mod erased {
    pub trait Ser {
        fn to_json(&self) -> Result<String, String>;
    }
    impl<T: serde::Serialize> Ser for T {
        fn to_json(&self) -> Result<String, String> {
            serde_json::to_string(self).map_err(|e| e.to_string())
        }
    }
}
