//! App theme: System / Light / Dark / Dynamic.
//!
//! Mirrors `src/lib/themes.ts`'s contract in native form:
//! - `ThemeChoice::System` follows the OS appearance via
//!   [`Theme::sync_system_appearance`](gpui_kit::component::Theme::sync_system_appearance).
//! - Light/Dark apply the Matt palette (warm orange accent on dark, flipped
//!   shell on light) from `App.css`'s `:root` / `[data-theme="light"]` tokens.
//! - Dynamic builds the same ramp from the current wallpaper's dominant hue,
//!   like Android's Material You (`wallpaper.rs` + `accentCssFromHex`).
//!
//! Colors are painted onto the live [`Theme`] via [`Theme::update`](gpui_kit::component::Theme::update),
//! which reconciles tokens and refreshes every window — so the vibrancy
//! slider can call [`apply_choice`] per-tick with no JSON regen (mirrors
//! `applyDynamicVibrancy`'s no-transition live preview).

use gpui_kit::component::{ActiveTheme as _, Theme, ThemeColor, ThemeMode};
use gpui_kit::{App, Hsla, Window};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// The theme choices the app offers: system follow + Matt dark (default) +
/// Matt light + wallpaper Dynamic.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ThemeChoice {
    /// Follow the OS light/dark appearance.
    System,
    /// Matt dark shell (the app's default look).
    #[default]
    Dark,
    /// Matt dark shell with true-black surfaces, for OLED panels.
    Oled,
    /// Matt light shell.
    Light,
    /// Wallpaper-derived accent ramp over the dark shell.
    Dynamic,
}

impl ThemeChoice {
    pub fn label(self) -> &'static str {
        match self {
            ThemeChoice::System => "System",
            ThemeChoice::Dark => "Dark",
            ThemeChoice::Oled => "OLED",
            ThemeChoice::Light => "Light",
            ThemeChoice::Dynamic => "Dynamic",
        }
    }
}

/// Default accent `#5dde89` as HSL: the same green Dynamic falls back to.
const MATT_ACCENT: (f32, f32, f32) = (140.5, 0.66, 0.62);
/// Default accent on the light shell, a deeper green (`#2f9e5a`) that holds contrast on white.
const MATT_LIGHT_ACCENT: (f32, f32, f32) = (143.0, 0.54, 0.40);
/// Dynamic fallback green `#5dde89` (same as `DYNAMIC_FALLBACK_HEX`).
const DYNAMIC_FALLBACK: (u8, u8, u8) = (93, 222, 137);

fn hsla(h: f32, s: f32, l: f32) -> Hsla {
    Hsla {
        h: h / 360.0,
        s,
        l,
        a: 1.0,
    }
}

/// Sample the current desktop wallpaper's dominant accent color, porting
/// `wallpaper.rs` (`wallpaper_accent_color` + `dominant_color`): downsample
/// to 64×64, bucket quantized colors weighted toward saturated
/// mid-brightness pixels (Material You idea), highest weight wins.
///
/// macOS reads the path via System Events, Windows via the registry; other
/// platforms return an error and Dynamic falls back to green (same contract
/// as the Tauri command). Blocking call — run on a background executor, not
/// in render.
/// Opens an image by extension, else by sniffing its bytes: Windows keeps a
/// slideshow's current frame as `TranscodedWallpaper`, JPEG data with no extension.
fn open_image(path: &std::path::Path) -> image::ImageResult<image::DynamicImage> {
    image::open(path).or_else(|first| {
        image::ImageReader::open(path)
            .and_then(|reader| reader.with_guessed_format())
            .map_err(image::ImageError::IoError)
            .and_then(|reader| reader.decode())
            .map_err(|_| first)
    })
}

pub fn sample_wallpaper_accent() -> anyhow::Result<(u8, u8, u8)> {
    let path = wallpaper_path()?;
    match open_image(&path) {
        Ok(img) => Ok(dominant_color(&img)),
        Err(first) => {
            // macOS wallpapers are HEIC by default (Sonoma+ dynamic pairs),
            // which the `image` crate cannot decode. Convert through `sips`
            // (ships with macOS, reads every wallpaper format) into a temp
            // PNG and decode that instead.
            #[cfg(target_os = "macos")]
            {
                let converted = std::env::temp_dir().join("openmouse-wallpaper.png");
                let path_str = path.to_string_lossy().into_owned();
                let out_str = converted.to_string_lossy().into_owned();
                // Same timeout discipline as osascript: never let a
                // stalled converter block the caller forever, and kill it so
                // the failed attempt does not leave a process behind.
                let mut command = std::process::Command::new("sips");
                command
                    .args(["-s", "format", "png", "-Z", "512"])
                    .arg(&path_str)
                    .arg("--out")
                    .arg(&out_str);
                let output = run_with_timeout(command, OSASCRIPT_TIMEOUT)
                    .map_err(|e| anyhow::anyhow!("sips failed: {e}"))?;
                if !output.status.success() {
                    anyhow::bail!(
                        "could not decode wallpaper at {} ({first}); sips exited {}",
                        path.display(),
                        output.status,
                    );
                }
                let img = image::open(&converted).map_err(|e| {
                    anyhow::anyhow!("could not decode sips output: {e} (first error: {first})")
                })?;
                Ok(dominant_color(&img))
            }
            #[cfg(not(target_os = "macos"))]
            {
                Err(anyhow::anyhow!(
                    "could not decode wallpaper at {}: {first}",
                    path.display()
                ))
            }
        }
    }
}

/// Run `osascript` with a timeout. A backgrounded process (nohup'd dev
/// builds, launchd without the right session) can stall AppleEvents to
/// System Events FOREVER — hanging the main thread at startup with no window
/// and no log. Never let a wallpaper read block longer than this.
#[cfg(target_os = "macos")]
const OSASCRIPT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Run a helper with a hard deadline, killing it on expiry.
///
/// A timed-out child used to keep running: the reader thread stayed blocked in
/// `child.wait()` forever, so a stalled `osascript` leaked one process and one
/// thread on every 2 s poll. Killing the child makes `wait` return, which ends
/// the reader thread with it.
#[cfg(any(target_os = "macos", test))]
fn run_with_timeout(
    mut command: std::process::Command,
    timeout: std::time::Duration,
) -> anyhow::Result<std::process::Output> {
    use std::io::Read as _;
    let mut child = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    // Drain the pipes on a helper thread: a full pipe buffer would block the
    // child and keep `try_wait` from ever reporting an exit.
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut out = Vec::new();
        let mut err = Vec::new();
        if let Some(mut o) = stdout {
            let _ = o.read_to_end(&mut out);
        }
        if let Some(mut e) = stderr {
            let _ = e.read_to_end(&mut err);
        }
        let _ = tx.send((out, err));
    });
    let deadline = std::time::Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("timed out after {timeout:?}; helper killed");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    // The child is gone, so the reader thread is at EOF or about to be. Cap
    // the wait anyway: a grandchild inheriting the pipes must not stall us.
    let (stdout, stderr) = rx
        .recv_timeout(std::time::Duration::from_millis(500))
        .unwrap_or_default();
    Ok(std::process::Output {
        status,
        stdout,
        stderr,
    })
}

#[cfg(target_os = "macos")]
fn run_osascript(args: &[&str]) -> anyhow::Result<std::process::Output> {
    let mut command = std::process::Command::new("osascript");
    command.args(args);
    run_with_timeout(command, OSASCRIPT_TIMEOUT)
        .map_err(|e| anyhow::anyhow!("osascript failed: {e}"))
}

#[cfg(target_os = "macos")]
fn wallpaper_path() -> anyhow::Result<std::path::PathBuf> {
    // No public API for this short of AppleScript; System Events' "picture
    // of current desktop" is the same source the wallpaper picker reads.
    let output = run_osascript(&[
        "-e",
        "tell application \"System Events\" to get picture of current desktop",
    ])?;
    if !output.status.success() {
        anyhow::bail!(
            "osascript exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        anyhow::bail!("osascript returned an empty wallpaper path");
    }
    Ok(path.into())
}

#[cfg(target_os = "windows")]
fn wallpaper_path() -> anyhow::Result<std::path::PathBuf> {
    crate::wallpaper_win::path()
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn wallpaper_path() -> anyhow::Result<std::path::PathBuf> {
    anyhow::bail!("wallpaper detection isn't implemented on this platform yet")
}

/// Cheap "has the wallpaper changed" check, porting
/// `wallpaper_signature`: path + mtime, no image decode. Callers poll this
/// often and only pay for `sample_wallpaper_accent` when it differs.
pub fn wallpaper_signature() -> anyhow::Result<String> {
    let path = wallpaper_path()?;
    let modified = std::fs::metadata(&path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    Ok(format!("{}|{modified}", path.display()))
}

/// Cached Dynamic sample: last-seen wallpaper signature + its accent hex.
/// Mirrors `DYNAMIC_SIGNATURE_KEY`/`DYNAMIC_HEX_KEY` in `themes.ts` — the web
/// app skips the expensive resample when the cheap signature is unchanged,
/// and so do we: switching back to Dynamic with the same wallpaper repaints
/// instantly from cache instead of respawning osascript + decoding.
/// Last-seen wallpaper signature + its sampled accent.
type DynamicCache = std::sync::Mutex<(Option<String>, Option<(u8, u8, u8)>)>;
static DYNAMIC_CACHE: DynamicCache = std::sync::Mutex::new((None, None));

/// Fresh accent for Dynamic: returns the cached hex when `wallpaper_signature`
/// matches the last sample, otherwise resamples (background-safe: caller
/// decides the executor) and caches. `None` only when sampling truly fails.
pub fn dynamic_accent() -> Option<(u8, u8, u8)> {
    let signature = wallpaper_signature().ok();
    let cache = DYNAMIC_CACHE.lock().unwrap();
    if signature.is_some() && signature == cache.0 {
        return cache.1;
    }
    drop(cache);
    let accent = sample_wallpaper_accent().ok()?;
    *DYNAMIC_CACHE.lock().unwrap() = (signature, Some(accent));
    Some(accent)
}

/// Store an externally sampled accent (watcher path) under its signature.
pub fn store_dynamic_sample(signature: Option<String>, accent: (u8, u8, u8)) {
    *DYNAMIC_CACHE.lock().unwrap() = (signature, Some(accent));
}

/// Seed the cache from persisted settings at startup (mirrors the web app
/// reading `DYNAMIC_HEX_KEY` before the first sample lands).
pub fn seed_dynamic_cache(signature: Option<String>, hex: Option<(u8, u8, u8)>) {
    *DYNAMIC_CACHE.lock().unwrap() = (signature, hex);
}

/// Last cached Dynamic hex, if any (for instant repaint without any I/O).
pub fn cached_dynamic_accent() -> Option<(u8, u8, u8)> {
    DYNAMIC_CACHE.lock().unwrap().1
}

/// Signature the cached sample came from; `None` before the first successful
/// sample. The watcher treats this as its "last seen" marker so the startup
/// sample (which fills it) is not decoded a second time on the first tick.
pub fn cached_dynamic_signature() -> Option<String> {
    DYNAMIC_CACHE.lock().unwrap().0.clone()
}

/// Records a signature whose sample failed, keeping the last good accent.
/// Without it an unreadable wallpaper would be re-decoded every poll.
fn mark_dynamic_signature(signature: Option<String>) {
    DYNAMIC_CACHE.lock().unwrap().0 = signature;
}

/// Poll loop keeping Dynamic in sync while the app runs (mirrors
/// `startDynamicAccentWatcher`): 2s signature ticks while Dynamic is the
/// theme and the window is active, slower otherwise, resampling + repainting
/// only when the signature moves. The blocking osascript spawn + image decode
/// run on the background executor (mirrors `spawn_blocking`); the UI thread
/// only pays for the repaint. Detached task — it ends when its view is dropped.
pub fn spawn_wallpaper_watcher(
    cx: &mut App,
    view: &gpui_kit::Entity<crate::app::AppView>,
    window: &gpui_kit::AnyWindowHandle,
) {
    let view = view.downgrade();
    let window = *window;
    cx.spawn(async move |cx| {
        // Only Dynamic follows the wallpaper, and every check costs an
        // `osascript` spawn on macOS, so any other theme just waits it out.
        // Switching to Dynamic resamples on its own (`AppView::set_theme`),
        // so the long wait never delays that. An inactive window also waits
        // longer, and coming back to it checks straight away.
        let mut dynamic = true;
        loop {
            let (active_for, inactive_for) = if dynamic {
                (Duration::from_secs(2), Duration::from_secs(30))
            } else {
                (Duration::from_secs(30), Duration::from_secs(30))
            };
            crate::activity::ACTIVITY
                .sleep(cx.background_executor(), active_for, inactive_for)
                .await;
            match view.update(cx, |view, _| view.theme() == ThemeChoice::Dynamic) {
                Ok(now_dynamic) => dynamic = now_dynamic,
                Err(_) => break,
            }
            if !dynamic {
                continue;
            }
            // Cheap check first (path|mtime, no decode)...
            let signature = cx
                .background_executor()
                .spawn(async { wallpaper_signature().ok() })
                .await;
            // The cache carries the signature of the last sample, including
            // the startup one, so this does not re-decode the same wallpaper
            // once per launch.
            if signature.is_none() || signature == cached_dynamic_signature() {
                continue;
            }
            // ...then the real sample on the background executor.
            let Some(accent) = cx
                .background_executor()
                .spawn(async { sample_wallpaper_accent().ok() })
                .await
            else {
                // Remember the failure: retrying an unreadable wallpaper every
                // 2 s would spawn the sampler forever.
                mark_dynamic_signature(signature);
                continue;
            };
            store_dynamic_sample(signature, accent);
            // Back on the foreground executor: repaint if Dynamic is live.
            if view
                .update(cx, |view, cx| {
                    view.note_wallpaper_sample(accent, window, cx);
                })
                .is_err()
            {
                break;
            }
        }
    })
    .detach();
}

fn dominant_color(img: &image::DynamicImage) -> (u8, u8, u8) {
    let small = img
        .resize(64, 64, image::imageops::FilterType::Triangle)
        .to_rgb8();
    let mut buckets: std::collections::HashMap<(u8, u8, u8), f64> =
        std::collections::HashMap::new();
    for pixel in small.pixels() {
        let [r, g, b] = pixel.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        // Brightness floor first: near-black pixels score saturation ~1.0
        // from trivial channel differences and would bury the real accent
        // under shadow regions (verified against a real photo upstream).
        if !(90..=250).contains(&max) {
            continue;
        }
        let key = (r & 0xF0, g & 0xF0, b & 0xF0);
        let saturation = (max - min) as f64 / max as f64;
        let lightness = (max as f64 + min as f64) / 2.0 / 255.0;
        let lightness_weight = (1.0 - (lightness - 0.6).abs() * 1.6).max(0.05);
        *buckets.entry(key).or_insert(0.0) += saturation.max(0.05) * lightness_weight;
    }
    buckets
        .into_iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|((r, g, b), _)| (r, g, b))
        .unwrap_or(DYNAMIC_FALLBACK)
}

fn hex_to_hsl(hex: (u8, u8, u8)) -> (f32, f32, f32) {
    let (r, g, b) = (
        hex.0 as f32 / 255.0,
        hex.1 as f32 / 255.0,
        hex.2 as f32 / 255.0,
    );
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < f32::EPSILON {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if (max - r).abs() < f32::EPSILON {
        60.0 * (((g - b) / d) % 6.0)
    } else if (max - g).abs() < f32::EPSILON {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (((h + 360.0) % 360.0), s.clamp(0.0, 1.0), l.clamp(0.0, 1.0))
}

/// Exact `App.css` hex token → [`Hsla`], avoiding hand-tuned HSL drift.
fn rgb_to_hsla(rgb: (u8, u8, u8)) -> Hsla {
    let (h, s, l) = hex_to_hsl(rgb);
    hsla(h, s, l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s < f32::EPSILON {
        return (l, l, l);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let hk = (((h % 360.0) + 360.0) % 360.0) / 360.0;
    let channel = |t: f32| {
        let t = ((t % 1.0) + 1.0) % 1.0;
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    (
        channel(hk + 1.0 / 3.0),
        channel(hk),
        channel(hk - 1.0 / 3.0),
    )
}

/// Surface ramp calibrated from the Violet preset's HSL values (see the
/// `DARK_RAMP` doc in `themes.ts`): each token holds the accent hue at a
/// fixed saturation/lightness, reproducing the hand-tuned dark look for any
/// wallpaper hue. Returns `(background, sidebar, card, border, muted)`.
fn dark_ramp(hue: f32, vibrancy: f32) -> (Hsla, Hsla, Hsla, Hsla, Hsla) {
    let scale = (vibrancy / 100.0).clamp(0.0, crate::settings::MAX_VIBRANCY / 100.0);
    let sat = |s: f32| (s * scale).clamp(0.0, 1.0);
    (
        hsla(hue, sat(0.31), 0.05), // background / sidebar
        hsla(hue, sat(0.31), 0.05), // sidebar
        hsla(hue, sat(0.28), 0.07), // card
        hsla(hue, sat(0.25), 0.20), // border (--line)
        hsla(hue, sat(0.16), 0.61), // muted text
    )
}

/// Apply a theme choice through the live [`Theme`]: set the mode, then paint
/// the Matt/Dynamic palette onto the public color fields.
///
/// `wallpaper_hex` is the sampled dominant color for Dynamic; `None` uses
/// the fallback green. `vibrancy` is 0–200, 100 = as designed. `picked` is
/// the user's accent override: it wins over every theme's own accent.
pub fn apply_choice(
    choice: ThemeChoice,
    wallpaper_hex: Option<(u8, u8, u8)>,
    vibrancy: f32,
    opacity: f32,
    picked: Option<(u8, u8, u8)>,
    window: Option<&mut Window>,
    cx: &mut App,
) {
    if choice == ThemeChoice::System {
        // Resolve the OS appearance, then paint the Matt palette for it.
        // `sync_system_appearance` alone only flips the mode (Kit default
        // colors) and leaves whatever the previous choice painted — a stale
        // Dynamic tint surviving into System.
        Theme::sync_system_appearance(window, cx);
        if cx.theme().mode.is_dark() {
            paint_matt(false, MATT_ACCENT, vibrancy, picked, wallpaper_hex, cx);
        } else {
            paint_matt(true, MATT_LIGHT_ACCENT, vibrancy, picked, wallpaper_hex, cx);
        }
        set_surface_opacity(opacity, cx);
        return;
    }

    let mode = match choice {
        ThemeChoice::Light => ThemeMode::Light,
        _ => ThemeMode::Dark,
    };
    Theme::change(mode, window, cx);

    match choice {
        ThemeChoice::Dynamic => {
            let (hue, sat, light) = hex_to_hsl(wallpaper_hex.unwrap_or(DYNAMIC_FALLBACK));
            paint_dynamic(hue, sat, light, vibrancy, picked, cx);
        }
        ThemeChoice::Dark => paint_matt(false, MATT_ACCENT, vibrancy, picked, wallpaper_hex, cx),
        ThemeChoice::Oled => paint_oled(vibrancy, picked, cx),
        ThemeChoice::Light => paint_matt(true, MATT_LIGHT_ACCENT, vibrancy, picked, wallpaper_hex, cx),
        ThemeChoice::System => unreachable!(),
    }
    set_surface_opacity(opacity, cx);
}

/// Tint the scene background without fading foreground text or controls.
/// Kit's Root owns the background paint; app shell layers remain clear.
pub fn set_surface_opacity(opacity: f32, cx: &mut App) {
    Theme::update(cx, |theme| {
        theme.colors.background.a = opacity.clamp(0.0, 1.0);
    });
}

/// The accent and the ink that sits on it, at this vibrancy. Ink is dark on
/// light accents and near-white on dark ones — same luminance rule as
/// `accentCssFromHex`.
fn accent_and_ink(accent_hsl: (f32, f32, f32), vibrancy: f32) -> (Hsla, Hsla) {
    let (hue, sat, light) = accent_hsl;
    let scale = (vibrancy / 100.0).clamp(0.0, crate::settings::MAX_VIBRANCY / 100.0);
    let sat_scaled = |s: f32| (s * scale).clamp(0.0, 1.0);
    let accent = hsla(hue, sat_scaled(sat), light);
    let (r, g, b) = hsl_to_rgb(hue, sat_scaled(sat), light);
    let ink = if 0.299 * r + 0.587 * g + 0.114 * b > 0.55 {
        hsla(hue, sat_scaled(0.30), 0.08)
    } else {
        hsla(hue, sat_scaled(0.15), 0.94)
    };
    (accent, ink)
}

/// Saturation/lightness a picked accent is generated at: enough to read as a
/// color on a dark shell without turning pastel or muddy.
const PICKED_SAT: f32 = 0.80;
const PICKED_LIGHT: f32 = 0.62;

/// Accent stored when the user picks `hue360` on the ring.
pub fn picked_accent(hue360: f32) -> (u8, u8, u8) {
    let (r, g, b) = hsl_to_rgb(hue360, PICKED_SAT, PICKED_LIGHT);
    let to_u8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    (to_u8(r), to_u8(g), to_u8(b))
}

/// The same accent as a color, for painting the ring's dots. One function
/// pair, so the swatch the user clicks is exactly the color they get.
pub fn picked_accent_hsla(hue360: f32) -> Hsla {
    let (r, g, b) = picked_accent(hue360);
    rgb_to_hsla((r, g, b))
}

/// Ink that reads on an arbitrary accent: near-black or near-white, whichever
/// contrasts more. Used for user-picked accents, which carry no theme hue.
fn readable_ink(accent: Hsla) -> Hsla {
    let dark = rgb_to_hsla((16, 16, 18));
    let light = rgb_to_hsla((250, 250, 250));
    if contrast(dark, accent) >= contrast(light, accent) {
        dark
    } else {
        light
    }
}

/// The accent a palette paints: the user's picked color when there is one
/// (verbatim — a picked color must not be re-saturated by the vibrancy
/// slider), otherwise the theme's own accent at the current vibrancy.
fn resolved_accent(
    default_hsl: (f32, f32, f32),
    vibrancy: f32,
    picked: Option<(u8, u8, u8)>,
) -> (Hsla, Hsla) {
    match picked {
        Some(rgb) => {
            let accent = rgb_to_hsla(rgb);
            (accent, readable_ink(accent))
        }
        None => accent_and_ink(default_hsl, vibrancy),
    }
}

/// Matt palette: exact `App.css` tokens — near-neutral surfaces, ONLY the
/// accent carrying hue. `is_light` selects the flipped light shell.
fn paint_matt(
    is_light: bool,
    accent_hsl: (f32, f32, f32),
    vibrancy: f32,
    picked: Option<(u8, u8, u8)>,
    _wallpaper_hex: Option<(u8, u8, u8)>,
    cx: &mut App,
) {
    let (accent, accent_ink) = resolved_accent(accent_hsl, vibrancy, picked);

    // Exact `App.css` tokens per shell; the two shells never share a
    // side of this branch (sharing is what split Light in half before).
    let (bg, sidebar, border, muted, muted_fg, fg) = if is_light {
        (
            rgb_to_hsla((247, 247, 244)), // --ink (light shell base)
            rgb_to_hsla((239, 239, 234)), // --sidebar
            rgb_to_hsla((220, 220, 214)), // --line
            rgb_to_hsla((115, 115, 112)), // --muted
            rgb_to_hsla((136, 136, 133)), // --faint
            rgb_to_hsla((20, 20, 20)),    // --text
        )
    } else {
        (
            rgb_to_hsla((13, 13, 15)),    // --sidebar #0d0d0f (near-neutral)
            rgb_to_hsla((13, 13, 15)),    // --sidebar
            rgb_to_hsla((41, 41, 45)),    // --line #29292d
            rgb_to_hsla((133, 133, 138)), // --muted #85858a
            rgb_to_hsla((119, 119, 124)), // --faint #77777c
            rgb_to_hsla((247, 247, 247)), // --text #f7f7f7
        )
    };
    paint_shared(
        bg, sidebar, border, muted, muted_fg, fg, accent, accent_ink, cx,
    );
}

/// OLED shell surfaces: `(background, sidebar, border, muted fill,
/// muted foreground, text)`. Pure black where the dark shell uses
/// near-black, so an OLED panel lights only the pixels carrying information;
/// everything above black stays one step up so structure is still visible.
fn oled_shell() -> (Hsla, Hsla, Hsla, Hsla, Hsla, Hsla) {
    (
        rgb_to_hsla((0, 0, 0)),       // background: true black
        rgb_to_hsla((0, 0, 0)),       // sidebar
        rgb_to_hsla((32, 32, 34)),    // border — the only separation on black
        rgb_to_hsla((24, 24, 26)),    // muted fill (badges, hover chips)
        rgb_to_hsla((150, 150, 156)), // muted foreground
        rgb_to_hsla((242, 242, 242)), // text
    )
}

/// OLED palette: the Matt accent on true-black surfaces.
fn paint_oled(vibrancy: f32, picked: Option<(u8, u8, u8)>, cx: &mut App) {
    let (accent, accent_ink) = resolved_accent(MATT_ACCENT, vibrancy, picked);
    let (bg, sidebar, border, muted, muted_fg, fg) = oled_shell();
    paint_shared(
        bg, sidebar, border, muted, muted_fg, fg, accent, accent_ink, cx,
    );
}

/// WCAG relative luminance of a colour.
fn luminance(c: Hsla) -> f32 {
    let (r, g, b) = hsl_to_rgb(c.h * 360.0, c.s, c.l);
    let lin = |v: f32| {
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

/// WCAG contrast ratio, 1.0 (identical) to 21.0 (black on white).
fn contrast(a: Hsla, b: Hsla) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Every colour Dynamic paints, computed from the sampled wallpaper colour.
struct DynamicPalette {
    bg: Hsla,
    sidebar: Hsla,
    /// Card / muted-surface fill.
    surface: Hsla,
    border: Hsla,
    muted_fg: Hsla,
    fg: Hsla,
    accent: Hsla,
    accent_ink: Hsla,
}

/// Minimum contrast of the accent against the background: it is used for text
/// and icons as well as fills, so it gets the text threshold, not the 3:1 one.
const ACCENT_MIN_CONTRAST: f32 = 4.5;
/// Borders are decoration, not text, so they only need to be seen.
const BORDER_MIN_CONTRAST: f32 = 1.4;

/// Dynamic palette: wallpaper-hue ramp over the dark shell, with the accent
/// pulled into a readable range. A wallpaper's dominant colour can be nearly
/// black or washed out, and using it raw gave an accent that vanished into the
/// background (or ink that could not be read on it).
fn dynamic_palette(hue: f32, sat: f32, light: f32, vibrancy: f32) -> DynamicPalette {
    let scale = (vibrancy / 100.0).clamp(0.0, crate::settings::MAX_VIBRANCY / 100.0);
    let sat_scaled = |s: f32| (s * scale).clamp(0.0, 1.0);
    // `dark_ramp` is (background, sidebar, card, border, muted text).
    let (bg, sidebar, surface, mut border, muted_fg) = dark_ramp(hue, vibrancy);
    let fg = hsla(hue, sat_scaled(0.40), 0.97);
    // Blue has so little luminance that the ramp's border can sink into the
    // background at high vibrancy; lift it until it reads as a line.
    while contrast(border, bg) < BORDER_MIN_CONTRAST && border.l < 0.5 {
        border.l += 0.01;
    }

    // Ink on the accent: whichever of a near-black and a near-white tone reads
    // better.
    let dark_ink = hsla(hue, sat_scaled(0.30), 0.03);
    let light_ink = hsla(hue, sat_scaled(0.10), 0.99);
    let best_ink = |accent: Hsla| {
        if contrast(dark_ink, accent) >= contrast(light_ink, accent) {
            dark_ink
        } else {
            light_ink
        }
    };

    // Keep the wallpaper's hue and (scaled) saturation, but not its lightness:
    // start in a mid-light band, then lift until the accent reads on the
    // background AND carries readable ink.
    let mut l = light.clamp(0.54, 0.74);
    let mut accent = hsla(hue, sat_scaled(sat), l);
    while (contrast(accent, bg) < ACCENT_MIN_CONTRAST
        || contrast(best_ink(accent), accent) < ACCENT_MIN_CONTRAST)
        && l < 0.9
    {
        l += 0.01;
        accent = hsla(hue, sat_scaled(sat), l);
    }
    let accent_ink = best_ink(accent);

    DynamicPalette { bg, sidebar, surface, border, muted_fg, fg, accent, accent_ink }
}

fn paint_dynamic(
    hue: f32,
    sat: f32,
    light: f32,
    vibrancy: f32,
    picked: Option<(u8, u8, u8)>,
    cx: &mut App,
) {
    let mut p = dynamic_palette(hue, sat, light, vibrancy);
    // A picked accent replaces the wallpaper-derived one; its ink follows the
    // same readability rule so badges stay legible on any color.
    if let Some(rgb) = picked {
        p.accent = rgb_to_hsla(rgb);
        p.accent_ink = readable_ink(p.accent);
    }
    // Every surface carries the wallpaper hue (mirrors the Tauri Dynamic
    // ramp, where ink/sidebar/card all hold the accent hue) — pinning `bg`
    // to neutral is what left the content area flat black next to a tinted
    // sidebar.
    paint_shared(
        p.bg, p.sidebar, p.border, p.surface, p.muted_fg, p.fg, p.accent, p.accent_ink, cx,
    );
}

/// One `Theme::update` for every palette: all surface fields always move
/// together, so no choice can ever split light/dark halves again.
#[allow(clippy::too_many_arguments)]
fn paint_shared(
    bg: Hsla,
    sidebar: Hsla,
    border: Hsla,
    muted: Hsla,
    muted_fg: Hsla,
    fg: Hsla,
    accent: Hsla,
    accent_ink: Hsla,
    cx: &mut App,
) {
    Theme::update(cx, |theme| {
        let c = &mut theme.colors;
        c.background = bg;
        c.foreground = fg;
        c.muted = muted;
        c.muted_foreground = muted_fg;
        c.sidebar = sidebar;
        c.border = border;
        c.accent = accent;
        c.accent_foreground = accent_ink;
        c.primary = accent;
        c.primary_foreground = accent_ink;
        c.title_bar = sidebar;
        c.title_bar_border = border;
        // Scrollbar track stays invisible so no grey band splits the shell
        // (the Tauri app has no track gutter); the thumb follows the theme.
        c.scrollbar = Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.0,
            a: 0.0,
        };
        c.scrollbar_thumb = border;
        c.scrollbar_thumb_hover = muted;
    });
}

/// Crossfade length for a theme switch — the same 380ms the web app puts on
/// its theme tokens (`App.css`), so a native switch reads like the web one.
pub const THEME_FADE_MS: u64 = 380;

/// Frame step of that crossfade (~60fps).
const THEME_FADE_STEP_MS: u64 = 16;

/// Only the newest crossfade may write colors. A switch landing mid-fade
/// bumps this and the older animator stops instead of fighting it for the
/// palette.
static THEME_FADE_SEQ: AtomicU64 = AtomicU64::new(0);

/// Shortest-arc hue interpolation: accents cross the color wheel the short
/// way, so orange → Dynamic-green never sweeps through the blues between.
fn lerp_hsla(from: Hsla, to: Hsla, t: f32) -> Hsla {
    let mut hue_delta = to.h - from.h;
    if hue_delta > 0.5 {
        hue_delta -= 1.0;
    } else if hue_delta < -0.5 {
        hue_delta += 1.0;
    }
    Hsla {
        h: (from.h + hue_delta * t).rem_euclid(1.0),
        s: from.s + (to.s - from.s) * t,
        l: from.l + (to.l - from.l) * t,
        a: from.a + (to.a - from.a) * t,
    }
}

/// Every palette token interpolated. The field list mirrors `ThemeColor`
/// exactly: a token added upstream that is missing here would sit at the old
/// color through the fade and snap at the end.
fn lerp_colors(from: ThemeColor, to: ThemeColor, t: f32) -> ThemeColor {
    ThemeColor {
        accent: lerp_hsla(from.accent, to.accent, t),
        accent_foreground: lerp_hsla(from.accent_foreground, to.accent_foreground, t),
        accordion: lerp_hsla(from.accordion, to.accordion, t),
        background: lerp_hsla(from.background, to.background, t),
        border: lerp_hsla(from.border, to.border, t),
        button: lerp_hsla(from.button, to.button, t),
        button_active: lerp_hsla(from.button_active, to.button_active, t),
        button_foreground: lerp_hsla(from.button_foreground, to.button_foreground, t),
        button_hover: lerp_hsla(from.button_hover, to.button_hover, t),
        button_danger: lerp_hsla(from.button_danger, to.button_danger, t),
        button_danger_active: lerp_hsla(from.button_danger_active, to.button_danger_active, t),
        button_danger_foreground: lerp_hsla(from.button_danger_foreground, to.button_danger_foreground, t),
        button_danger_hover: lerp_hsla(from.button_danger_hover, to.button_danger_hover, t),
        button_info: lerp_hsla(from.button_info, to.button_info, t),
        button_info_active: lerp_hsla(from.button_info_active, to.button_info_active, t),
        button_info_foreground: lerp_hsla(from.button_info_foreground, to.button_info_foreground, t),
        button_info_hover: lerp_hsla(from.button_info_hover, to.button_info_hover, t),
        button_primary: lerp_hsla(from.button_primary, to.button_primary, t),
        button_primary_active: lerp_hsla(from.button_primary_active, to.button_primary_active, t),
        button_primary_foreground: lerp_hsla(from.button_primary_foreground, to.button_primary_foreground, t),
        button_primary_hover: lerp_hsla(from.button_primary_hover, to.button_primary_hover, t),
        button_secondary: lerp_hsla(from.button_secondary, to.button_secondary, t),
        button_secondary_active: lerp_hsla(from.button_secondary_active, to.button_secondary_active, t),
        button_secondary_foreground: lerp_hsla(from.button_secondary_foreground, to.button_secondary_foreground, t),
        button_secondary_hover: lerp_hsla(from.button_secondary_hover, to.button_secondary_hover, t),
        button_success: lerp_hsla(from.button_success, to.button_success, t),
        button_success_active: lerp_hsla(from.button_success_active, to.button_success_active, t),
        button_success_foreground: lerp_hsla(from.button_success_foreground, to.button_success_foreground, t),
        button_success_hover: lerp_hsla(from.button_success_hover, to.button_success_hover, t),
        button_warning: lerp_hsla(from.button_warning, to.button_warning, t),
        button_warning_active: lerp_hsla(from.button_warning_active, to.button_warning_active, t),
        button_warning_foreground: lerp_hsla(from.button_warning_foreground, to.button_warning_foreground, t),
        button_warning_hover: lerp_hsla(from.button_warning_hover, to.button_warning_hover, t),
        group_box: lerp_hsla(from.group_box, to.group_box, t),
        group_box_foreground: lerp_hsla(from.group_box_foreground, to.group_box_foreground, t),
        caret: lerp_hsla(from.caret, to.caret, t),
        chart_1: lerp_hsla(from.chart_1, to.chart_1, t),
        chart_2: lerp_hsla(from.chart_2, to.chart_2, t),
        chart_3: lerp_hsla(from.chart_3, to.chart_3, t),
        chart_4: lerp_hsla(from.chart_4, to.chart_4, t),
        chart_5: lerp_hsla(from.chart_5, to.chart_5, t),
        chart_bullish: lerp_hsla(from.chart_bullish, to.chart_bullish, t),
        chart_bearish: lerp_hsla(from.chart_bearish, to.chart_bearish, t),
        chart_grid: lerp_hsla(from.chart_grid, to.chart_grid, t),
        danger: lerp_hsla(from.danger, to.danger, t),
        danger_active: lerp_hsla(from.danger_active, to.danger_active, t),
        danger_foreground: lerp_hsla(from.danger_foreground, to.danger_foreground, t),
        danger_hover: lerp_hsla(from.danger_hover, to.danger_hover, t),
        description_list_label: lerp_hsla(from.description_list_label, to.description_list_label, t),
        description_list_label_foreground: lerp_hsla(from.description_list_label_foreground, to.description_list_label_foreground, t),
        drag_border: lerp_hsla(from.drag_border, to.drag_border, t),
        drop_target: lerp_hsla(from.drop_target, to.drop_target, t),
        foreground: lerp_hsla(from.foreground, to.foreground, t),
        info: lerp_hsla(from.info, to.info, t),
        info_active: lerp_hsla(from.info_active, to.info_active, t),
        info_foreground: lerp_hsla(from.info_foreground, to.info_foreground, t),
        info_hover: lerp_hsla(from.info_hover, to.info_hover, t),
        input: lerp_hsla(from.input, to.input, t),
        link: lerp_hsla(from.link, to.link, t),
        link_active: lerp_hsla(from.link_active, to.link_active, t),
        link_hover: lerp_hsla(from.link_hover, to.link_hover, t),
        list: lerp_hsla(from.list, to.list, t),
        list_active: lerp_hsla(from.list_active, to.list_active, t),
        list_active_border: lerp_hsla(from.list_active_border, to.list_active_border, t),
        list_even: lerp_hsla(from.list_even, to.list_even, t),
        list_head: lerp_hsla(from.list_head, to.list_head, t),
        list_hover: lerp_hsla(from.list_hover, to.list_hover, t),
        muted: lerp_hsla(from.muted, to.muted, t),
        muted_foreground: lerp_hsla(from.muted_foreground, to.muted_foreground, t),
        popover: lerp_hsla(from.popover, to.popover, t),
        popover_foreground: lerp_hsla(from.popover_foreground, to.popover_foreground, t),
        primary: lerp_hsla(from.primary, to.primary, t),
        primary_active: lerp_hsla(from.primary_active, to.primary_active, t),
        primary_foreground: lerp_hsla(from.primary_foreground, to.primary_foreground, t),
        primary_hover: lerp_hsla(from.primary_hover, to.primary_hover, t),
        progress_bar: lerp_hsla(from.progress_bar, to.progress_bar, t),
        ring: lerp_hsla(from.ring, to.ring, t),
        scrollbar: lerp_hsla(from.scrollbar, to.scrollbar, t),
        scrollbar_thumb: lerp_hsla(from.scrollbar_thumb, to.scrollbar_thumb, t),
        scrollbar_thumb_hover: lerp_hsla(from.scrollbar_thumb_hover, to.scrollbar_thumb_hover, t),
        secondary: lerp_hsla(from.secondary, to.secondary, t),
        secondary_active: lerp_hsla(from.secondary_active, to.secondary_active, t),
        secondary_foreground: lerp_hsla(from.secondary_foreground, to.secondary_foreground, t),
        secondary_hover: lerp_hsla(from.secondary_hover, to.secondary_hover, t),
        selection: lerp_hsla(from.selection, to.selection, t),
        sidebar: lerp_hsla(from.sidebar, to.sidebar, t),
        sidebar_accent: lerp_hsla(from.sidebar_accent, to.sidebar_accent, t),
        sidebar_accent_foreground: lerp_hsla(from.sidebar_accent_foreground, to.sidebar_accent_foreground, t),
        sidebar_border: lerp_hsla(from.sidebar_border, to.sidebar_border, t),
        sidebar_foreground: lerp_hsla(from.sidebar_foreground, to.sidebar_foreground, t),
        sidebar_primary: lerp_hsla(from.sidebar_primary, to.sidebar_primary, t),
        sidebar_primary_foreground: lerp_hsla(from.sidebar_primary_foreground, to.sidebar_primary_foreground, t),
        skeleton: lerp_hsla(from.skeleton, to.skeleton, t),
        slider_bar: lerp_hsla(from.slider_bar, to.slider_bar, t),
        slider_thumb: lerp_hsla(from.slider_thumb, to.slider_thumb, t),
        success: lerp_hsla(from.success, to.success, t),
        success_foreground: lerp_hsla(from.success_foreground, to.success_foreground, t),
        success_hover: lerp_hsla(from.success_hover, to.success_hover, t),
        success_active: lerp_hsla(from.success_active, to.success_active, t),
        switch: lerp_hsla(from.switch, to.switch, t),
        switch_thumb: lerp_hsla(from.switch_thumb, to.switch_thumb, t),
        tab: lerp_hsla(from.tab, to.tab, t),
        tab_active: lerp_hsla(from.tab_active, to.tab_active, t),
        tab_active_foreground: lerp_hsla(from.tab_active_foreground, to.tab_active_foreground, t),
        tab_bar: lerp_hsla(from.tab_bar, to.tab_bar, t),
        tab_bar_segmented: lerp_hsla(from.tab_bar_segmented, to.tab_bar_segmented, t),
        tab_foreground: lerp_hsla(from.tab_foreground, to.tab_foreground, t),
        table: lerp_hsla(from.table, to.table, t),
        table_active: lerp_hsla(from.table_active, to.table_active, t),
        table_active_border: lerp_hsla(from.table_active_border, to.table_active_border, t),
        table_even: lerp_hsla(from.table_even, to.table_even, t),
        table_head: lerp_hsla(from.table_head, to.table_head, t),
        table_head_foreground: lerp_hsla(from.table_head_foreground, to.table_head_foreground, t),
        table_foot: lerp_hsla(from.table_foot, to.table_foot, t),
        table_foot_foreground: lerp_hsla(from.table_foot_foreground, to.table_foot_foreground, t),
        table_hover: lerp_hsla(from.table_hover, to.table_hover, t),
        table_row_border: lerp_hsla(from.table_row_border, to.table_row_border, t),
        title_bar: lerp_hsla(from.title_bar, to.title_bar, t),
        title_bar_border: lerp_hsla(from.title_bar_border, to.title_bar_border, t),
        status_bar: lerp_hsla(from.status_bar, to.status_bar, t),
        status_bar_border: lerp_hsla(from.status_bar_border, to.status_bar_border, t),
        warning: lerp_hsla(from.warning, to.warning, t),
        warning_active: lerp_hsla(from.warning_active, to.warning_active, t),
        warning_hover: lerp_hsla(from.warning_hover, to.warning_hover, t),
        warning_foreground: lerp_hsla(from.warning_foreground, to.warning_foreground, t),
        overlay: lerp_hsla(from.overlay, to.overlay, t),
        window_border: lerp_hsla(from.window_border, to.window_border, t),
        red: lerp_hsla(from.red, to.red, t),
        red_light: lerp_hsla(from.red_light, to.red_light, t),
        green: lerp_hsla(from.green, to.green, t),
        green_light: lerp_hsla(from.green_light, to.green_light, t),
        blue: lerp_hsla(from.blue, to.blue, t),
        blue_light: lerp_hsla(from.blue_light, to.blue_light, t),
        yellow: lerp_hsla(from.yellow, to.yellow, t),
        yellow_light: lerp_hsla(from.yellow_light, to.yellow_light, t),
        magenta: lerp_hsla(from.magenta, to.magenta, t),
        magenta_light: lerp_hsla(from.magenta_light, to.magenta_light, t),
        cyan: lerp_hsla(from.cyan, to.cyan, t),
        cyan_light: lerp_hsla(from.cyan_light, to.cyan_light, t),
    }
}

/// Fade the palette from `from` to `to` over [`THEME_FADE_MS`], one
/// `Theme::update` per frame, so every window repaints with the interpolated
/// tokens. The caller applies the new theme first (this only rewrites colors),
/// which keeps mode switches, font resolution, and token reconciliation on the
/// existing path.
///
/// Skipped when the palettes already match or the OS asks for reduced motion —
/// in both cases the caller's instant switch is the whole story.
pub fn crossfade_colors(from: ThemeColor, to: ThemeColor, cx: &mut App) {
    let seq = THEME_FADE_SEQ.fetch_add(1, Ordering::Relaxed) + 1;
    if from == to || cx.reduce_motion() {
        return;
    }
    // Put the starting palette back before returning: the caller has already
    // applied the target, and letting one frame paint with it first shows up
    // as a flash of the new theme before the fade begins.
    Theme::update(cx, |theme| theme.colors = from);
    let duration = Duration::from_millis(THEME_FADE_MS).as_secs_f32();
    cx.spawn(async move |cx| {
        let start = Instant::now();
        loop {
            if THEME_FADE_SEQ.load(Ordering::Relaxed) != seq {
                return;
            }
            let t = (start.elapsed().as_secs_f32() / duration).clamp(0.0, 1.0);
            // Ease-out cubic: most of the color movement lands early, like the
            // web transition, instead of crawling through the middle.
            let eased = 1.0 - (1.0 - t).powi(3);
            let colors = lerp_colors(from, to, eased);
            cx.update(move |cx| Theme::update(cx, |theme| theme.colors = colors));
            if t >= 1.0 {
                return;
            }
            cx.background_executor()
                .timer(Duration::from_millis(THEME_FADE_STEP_MS))
                .await;
        }
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A helper that never exits must be killed at the deadline: the old
    /// thread + `recv_timeout` shape leaked the child (and its waiter thread)
    /// on every stalled poll.
    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn timed_out_helpers_are_killed() {
        #[cfg(windows)]
        let mut slow = {
            let mut command = std::process::Command::new("cmd");
            command.args(["/c", "ping", "-n", "30", "127.0.0.1"]);
            command
        };
        #[cfg(target_os = "macos")]
        let mut slow = {
            let mut command = std::process::Command::new("sleep");
            command.arg("30");
            command
        };
        let started = std::time::Instant::now();
        let error = run_with_timeout(slow, std::time::Duration::from_millis(200))
            .expect_err("a 30 s helper must time out");
        assert!(error.to_string().contains("timed out"), "{error}");
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
    }

    /// The kill path must not break the normal path: output still arrives.
    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn helper_output_is_captured() {
        #[cfg(windows)]
        let command = {
            let mut command = std::process::Command::new("cmd");
            command.args(["/c", "echo", "openmouse"]);
            command
        };
        #[cfg(target_os = "macos")]
        let command = {
            let mut command = std::process::Command::new("echo");
            command.arg("openmouse");
            command
        };
        let output = run_with_timeout(command, std::time::Duration::from_secs(5)).unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("openmouse"));
    }

    #[test]
    fn default_accent_hue_is_green() {
        let (h, _, _) = MATT_ACCENT;
        assert!((130.0..150.0).contains(&h));
    }

    #[test]
    fn fallback_green_converts() {
        let (h, s, l) = hex_to_hsl(DYNAMIC_FALLBACK);
        assert!((130.0..150.0).contains(&h), "green hue, got {h}");
        assert!(s > 0.5);
        assert!((0.5..0.8).contains(&l));
    }

    #[test]
    fn vibrancy_zero_mutes_to_gray() {
        let (_, _, _, _, muted) = dark_ramp(250.0, 0.0);
        assert_eq!(muted.s, 0.0);
    }

    #[test]
    fn vibrancy_above_150_strengthens_colors_until_200() {
        let (at_150, ..) = dark_ramp(250.0, 150.0);
        let (at_200, ..) = dark_ramp(250.0, 200.0);
        let (above_max, ..) = dark_ramp(250.0, 250.0);
        assert!(at_200.s > at_150.s);
        assert!((at_200.s - 0.62).abs() < f32::EPSILON);
        assert_eq!(above_max.s, at_200.s);
    }

    #[test]
    fn hsl_round_trip_is_stable() {
        let (h, s, l) = hex_to_hsl((245, 151, 92));
        let (r, g, b) = hsl_to_rgb(h, s, l);
        assert!(((r * 255.0).round() - 245.0).abs() <= 2.0);
        assert!(((g * 255.0).round() - 151.0).abs() <= 2.0);
        assert!(((b * 255.0).round() - 92.0).abs() <= 2.0);
    }

    /// Wallpaper colours that used to break Dynamic: near-black, near-white,
    /// gray, saturated blue (low luminance), yellow, and the green fallback.
    const AWKWARD: [(u8, u8, u8); 8] = [
        (8, 10, 24),
        (12, 12, 12),
        (250, 250, 250),
        (128, 128, 128),
        (20, 30, 220),
        (250, 220, 20),
        (93, 222, 137),
        (120, 20, 40),
    ];

    #[test]
    fn dynamic_palette_stays_readable_for_any_wallpaper() {
        for rgb in AWKWARD {
            for vibrancy in [0.0, 100.0, 200.0] {
                let (h, s, l) = hex_to_hsl(rgb);
                let p = dynamic_palette(h, s, l, vibrancy);
                let at = format!("wallpaper {rgb:?} vibrancy {vibrancy}");
                assert!(contrast(p.fg, p.bg) >= 12.0, "text on background, {at}");
                assert!(contrast(p.muted_fg, p.bg) >= 4.5, "muted text on background, {at}");
                assert!(contrast(p.muted_fg, p.surface) >= 4.5, "muted text on card, {at}");
                assert!(contrast(p.accent, p.bg) >= 4.5, "accent on background, {at}");
                assert!(contrast(p.accent_ink, p.accent) >= 4.5, "ink on accent, {at}");
                assert!(contrast(p.border, p.bg) >= 1.4, "border visible, {at}");
            }
        }
    }

    #[test]
    fn dynamic_border_is_not_the_card_color() {
        let (h, s, l) = hex_to_hsl(DYNAMIC_FALLBACK);
        let p = dynamic_palette(h, s, l, 100.0);
        assert!(p.border.l > p.surface.l + 0.05);
    }

    /// Blends `top` at `alpha` over `bottom`, in sRGB like the compositor does.
    fn over(top: Hsla, alpha: f32, bottom: Hsla) -> Hsla {
        let (tr, tg, tb) = hsl_to_rgb(top.h * 360.0, top.s, top.l);
        let (br, bg, bb) = hsl_to_rgb(bottom.h * 360.0, bottom.s, bottom.l);
        let mix = |t: f32, b: f32| t * alpha + b * (1.0 - alpha);
        let (r, g, b) = (mix(tr, br), mix(tg, bg), mix(tb, bb));
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;
        if (max - min).abs() < f32::EPSILON {
            return Hsla { h: 0.0, s: 0.0, l, a: 1.0 };
        }
        let d = max - min;
        let s = d / (1.0 - (2.0 * l - 1.0).abs());
        let h = if (max - r).abs() < f32::EPSILON {
            60.0 * (((g - b) / d) % 6.0)
        } else if (max - g).abs() < f32::EPSILON {
            60.0 * ((b - r) / d + 2.0)
        } else {
            60.0 * ((r - g) / d + 4.0)
        };
        Hsla { h: ((h + 360.0) % 360.0) / 360.0, s: s.clamp(0.0, 1.0), l, a: 1.0 }
    }

    /// A palette fade must land exactly on both ends and cross the hue wheel
    /// the short way — a long-way lerp reads as a flash of unrelated color
    /// halfway through the switch.
    #[test]
    fn theme_crossfade_hits_both_ends_and_takes_the_short_hue_way() {
        let from = ThemeColor {
            accent: Hsla { h: 10.0 / 360.0, s: 0.8, l: 0.5, a: 1.0 },
            ..ThemeColor::default()
        };
        let to = ThemeColor {
            accent: Hsla { h: 350.0 / 360.0, s: 0.4, l: 0.3, a: 1.0 },
            ..ThemeColor::default()
        };
        let start = lerp_colors(from, to, 0.0).accent;
        assert!((start.h - from.accent.h).abs() < 1e-6 && (start.l - from.accent.l).abs() < 1e-6);
        let end = lerp_colors(from, to, 1.0).accent;
        assert!((end.h - to.accent.h).abs() < 1e-6, "end hue {} wanted {}", end.h, to.accent.h);
        assert!((end.s - to.accent.s).abs() < 1e-6 && (end.l - to.accent.l).abs() < 1e-6);
        // 10° → 350° is 20° across zero, not 340° the long way round.
        let mid = lerp_colors(from, to, 0.5).accent.h;
        assert!(mid.min(1.0 - mid) < 0.01, "mid hue {mid} should sit at zero");
        // Tokens the switch does not touch hold their color through the fade.
        assert_eq!(lerp_colors(from, to, 0.5).background, from.background);
    }

    /// OLED only earns its name if the background is *actually* black: any
    /// near-black tint lights the whole panel. Text still has to be readable
    /// on it, and the surfaces above black must stay distinguishable.
    #[test]
    fn oled_shell_is_true_black_and_still_readable() {
        let (bg, _sidebar, border, _muted, muted_fg, fg) = oled_shell();
        assert_eq!(hsl_to_rgb(0.0, bg.s, bg.l), (0.0, 0.0, 0.0), "background must be #000000");
        assert!(contrast(fg, bg) >= 15.0, "text on black: {:.2}", contrast(fg, bg));
        assert!(contrast(muted_fg, bg) >= 4.5, "muted text on black: {:.2}", contrast(muted_fg, bg));
        assert!(luminance(border) > luminance(bg), "black needs a visible border");
    }

    /// The accent badge (app.rs `accent_badge`): the theme's text colour on
    /// accent at 20% over the page and over a selected row's 10% tint.
    #[test]
    fn accent_badge_text_is_readable_on_its_tint() {
        for rgb in AWKWARD {
            let (h, s, l) = hex_to_hsl(rgb);
            let p = dynamic_palette(h, s, l, 100.0);
            for (label, surface) in [("page", p.bg), ("selected row", over(p.accent, 0.10, p.bg))] {
                let badge_fill = over(p.accent, 0.20, surface);
                let ratio = contrast(p.fg, badge_fill);
                assert!(ratio >= 7.0, "badge text over {label}, wallpaper {rgb:?}: {ratio:.2}");
            }
        }
    }
}
