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

use gpui_kit::component::{ActiveTheme as _, Theme, ThemeMode};
use gpui_kit::{App, Hsla, Window};

/// The theme choices the app offers: system follow + Matt dark (default) +
/// Matt light + wallpaper Dynamic.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ThemeChoice {
    /// Follow the OS light/dark appearance.
    System,
    /// Matt dark shell (the app's default look).
    #[default]
    Dark,
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
            ThemeChoice::Light => "Light",
            ThemeChoice::Dynamic => "Dynamic",
        }
    }
}

/// Matt accent `#f5975c` as HSL (matches `App.css :root --accent`).
const MATT_ACCENT: (f32, f32, f32) = (27.0, 0.89, 0.66);
/// Matt light-shell accent `#d46a32`.
const MATT_LIGHT_ACCENT: (f32, f32, f32) = (19.0, 0.65, 0.51);
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
                // stalled converter block the caller forever.
                let mut child = std::process::Command::new("sips")
                    .args(["-s", "format", "png", "-Z", "512"])
                    .arg(&path_str)
                    .arg("--out")
                    .arg(&out_str)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .map_err(|e| anyhow::anyhow!("could not run sips: {e}"))?;
                let status = {
                    let (tx, rx) = std::sync::mpsc::channel();
                    std::thread::spawn(move || {
                        let _ = tx.send(child.wait());
                    });
                    match rx.recv_timeout(OSASCRIPT_TIMEOUT) {
                        Ok(r) => r.map_err(|e| anyhow::anyhow!("sips failed: {e}"))?,
                        Err(_) => anyhow::bail!("sips timed out after 5s"),
                    }
                };
                if !status.success() {
                    anyhow::bail!(
                        "could not decode wallpaper at {} ({first}); sips exited {status}",
                        path.display(),
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

#[cfg(target_os = "macos")]
fn run_osascript(args: &[&str]) -> anyhow::Result<std::process::Output> {
    use std::io::Read as _;
    let mut child = std::process::Command::new("osascript")
        .args(args)
        // Detach stdin: a backgrounded launch has no tty, and an inherited
        // closed/confused stdin has hung script hosts before.
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("could not run osascript: {e}"))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let status = child.wait();
        let mut out = Vec::new();
        let mut err = Vec::new();
        if let Some(mut o) = stdout {
            let _ = o.read_to_end(&mut out);
        }
        if let Some(mut e) = stderr {
            let _ = e.read_to_end(&mut err);
        }
        let _ = tx.send((status, out, err));
    });
    match rx.recv_timeout(OSASCRIPT_TIMEOUT) {
        Ok((status, out, err)) => {
            let status = status.map_err(|e| anyhow::anyhow!("osascript wait failed: {e}"))?;
            Ok(std::process::Output {
                status,
                stdout: out,
                stderr: err,
            })
        }
        Err(_) => anyhow::bail!("osascript timed out after 5s (backgrounded process?)"),
    }
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
    use std::time::Duration;
    let view = view.downgrade();
    let window = *window;
    cx.spawn(async move |cx| {
        let mut last: Option<String> = None;
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
            if signature == last {
                continue;
            }
            // ...then the real sample on the background executor.
            let Some(accent) = cx
                .background_executor()
                .spawn(async { sample_wallpaper_accent().ok() })
                .await
            else {
                continue;
            };
            last = signature.clone();
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
/// the fallback green. `vibrancy` is 0–200, 100 = as designed.
pub fn apply_choice(
    choice: ThemeChoice,
    wallpaper_hex: Option<(u8, u8, u8)>,
    vibrancy: f32,
    opacity: f32,
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
            paint_matt(false, MATT_ACCENT, vibrancy, wallpaper_hex, cx);
        } else {
            paint_matt(true, MATT_LIGHT_ACCENT, vibrancy, wallpaper_hex, cx);
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
            paint_dynamic(hue, sat, light, vibrancy, cx);
        }
        ThemeChoice::Dark => paint_matt(false, MATT_ACCENT, vibrancy, wallpaper_hex, cx),
        ThemeChoice::Light => paint_matt(true, MATT_LIGHT_ACCENT, vibrancy, wallpaper_hex, cx),
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

/// Matt palette: exact `App.css` tokens — near-neutral surfaces, ONLY the
/// accent carrying hue. `is_light` selects the flipped light shell.
fn paint_matt(
    is_light: bool,
    accent_hsl: (f32, f32, f32),
    vibrancy: f32,
    _wallpaper_hex: Option<(u8, u8, u8)>,
    cx: &mut App,
) {
    let (hue, sat, light) = accent_hsl;
    let scale = (vibrancy / 100.0).clamp(0.0, crate::settings::MAX_VIBRANCY / 100.0);
    let sat_scaled = |s: f32| (s * scale).clamp(0.0, 1.0);
    let accent = hsla(hue, sat_scaled(sat), light);
    // Accent ink: dark text on light accents, near-white on dark ones —
    // same luminance rule as `accentCssFromHex`.
    let (r, g, b) = hsl_to_rgb(hue, sat_scaled(sat), light);
    let accent_ink = if 0.299 * r + 0.587 * g + 0.114 * b > 0.55 {
        hsla(hue, sat_scaled(0.30), 0.08)
    } else {
        hsla(hue, sat_scaled(0.15), 0.94)
    };

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

fn paint_dynamic(hue: f32, sat: f32, light: f32, vibrancy: f32, cx: &mut App) {
    let p = dynamic_palette(hue, sat, light, vibrancy);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matt_orange_hue_is_recognizable() {
        let (h, _, _) = MATT_ACCENT;
        assert!((20.0..35.0).contains(&h));
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
