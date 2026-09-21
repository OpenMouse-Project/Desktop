//! Reads the current desktop wallpaper and picks a single dominant, vibrant
//! color from it — the one piece of the "Dynamic" theme (see the frontend's
//! `lib/themes.ts`) that can't be a fixed preset, since it's different for
//! every user and every wallpaper. Only macOS and Windows are implemented;
//! Linux has no single place a wallpaper path lives (it varies by desktop
//! environment), so the command there returns an error the frontend falls
//! back on.

use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::process::Command as Cmd;

#[cfg(target_os = "macos")]
fn wallpaper_path() -> Result<PathBuf> {
    // No public API for this short of AppleScript; System Events' "picture
    // of current desktop" is the same source macOS's own wallpaper picker
    // reads from, and returns the image's file path directly.
    let output = Cmd::new("osascript")
        .args(["-e", "tell application \"System Events\" to get picture of current desktop"])
        .output()
        .context("could not run osascript")?;
    if !output.status.success() {
        bail!(
            "osascript exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        bail!("osascript returned an empty wallpaper path");
    }
    Ok(PathBuf::from(path))
}

#[cfg(target_os = "windows")]
fn wallpaper_path() -> Result<PathBuf> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let desktop = hkcu
        .open_subkey("Control Panel\\Desktop")
        .context("could not open Control Panel\\Desktop")?;
    let path: String = desktop
        .get_value("WallPaper")
        .context("could not read the WallPaper registry value")?;
    if path.is_empty() {
        bail!("WallPaper registry value is empty (solid color, not an image)");
    }
    Ok(PathBuf::from(path))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn wallpaper_path() -> Result<PathBuf> {
    bail!("wallpaper detection isn't implemented on this platform yet");
}

/// The app's own default green accent — returned when a wallpaper genuinely
/// has no usable color (solid black/white, fully desaturated), not as a
/// silent failure mode but because "no strong color" is a legitimate answer.
const FALLBACK_ACCENT: (u8, u8, u8) = (93, 222, 137);

/// Downsamples the wallpaper to a coarse grid and buckets pixels by
/// quantized color, each weighted toward saturated, mid-brightness pixels —
/// the same idea Android's Material You uses so the result reads as "an
/// accent color a person picked" rather than an average that blends two big
/// regions of a wallpaper into a third color neither region actually has.
/// Picks the highest-weighted bucket rather than the highest-*count* one, so
/// a large but dull region (a gray sky) doesn't beat a smaller vivid one (a
/// red logo) just by covering more pixels.
fn dominant_color(img: &image::DynamicImage) -> (u8, u8, u8) {
    let small = img
        .resize(64, 64, image::imageops::FilterType::Triangle)
        .to_rgb8();
    let mut buckets: std::collections::HashMap<(u8, u8, u8), f64> = std::collections::HashMap::new();
    for pixel in small.pixels() {
        let [r, g, b] = pixel.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        // The saturation ratio `(max-min)/max` blows up toward 1.0 for
        // near-black pixels with even one unit of channel difference (a
        // shadow at (0, 3, 0) reads as "fully saturated green"), which
        // otherwise buried every wallpaper's actual accent color under its
        // dark, desaturated-looking shadow regions (CONFIRMED against a
        // real photo: this picked near-black (0, 32, 0) over the photo's
        // visibly green foliage). Requiring real brightness before scoring
        // a pixel at all — not just down-weighting it — fixes that; a
        // near-white pixel is excluded for the mirror reason (no color left
        // to read once every channel is pinned near 255).
        // The floor is higher than a plain "not black" cutoff (40) would be:
        // this doubles as "bright enough to read as a UI accent against this
        // app's dark shell (--surface-bg #08090a)," not just "not literally
        // a shadow." Verified against a real photo: a 40 floor still let a
        // shadowed-foliage dark olive (#104000) outscore the same photo's
        // actual mid-tone greens by sheer pixel count.
        if max < 90 || max > 250 {
            continue;
        }
        // Quantize to 16 steps per channel so near-identical colors merge
        // into the same bucket instead of splitting their weight.
        let key = (r & 0xF0, g & 0xF0, b & 0xF0);
        let saturation = (max - min) as f64 / max as f64;
        let lightness = (max as f64 + min as f64) / 2.0 / 255.0;
        // Targets ~0.6, not 0.5: this app's own default accent (#5dde89)
        // sits at lightness 0.62, and an accent needs to read clearly
        // against a dark shell — 0.5 let a legible-on-paper but visually
        // muddy mid-gray-green outrank a same-photo tone that actually pops.
        let lightness_weight = (1.0 - (lightness - 0.6).abs() * 1.6).max(0.05);
        let weight = saturation.max(0.05) * lightness_weight;
        *buckets.entry(key).or_insert(0.0) += weight;
    }
    buckets
        .into_iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|((r, g, b), _)| (r, g, b))
        .unwrap_or(FALLBACK_ACCENT)
}

/// Cheap "has the wallpaper changed" check — the path (osascript/registry
/// read) plus the file's modified time, with no image decode at all.
///
/// The frontend polls on a timer and on every window focus (themes.ts's
/// startDynamicAccentWatcher) to catch a wallpaper change without a manual
/// refresh; calling `wallpaper_accent_color` for that would mean spawning
/// osascript AND fully decoding + resizing the wallpaper file on every
/// single focus event, wallpaper unchanged or not. CONFIRMED as a real,
/// user-visible freeze on refocusing the app — a decode+resample of a large
/// (multi-megapixel, common for a desktop background) image is genuinely
/// slow enough to notice, and it was paying that cost every time regardless
/// of whether anything had actually changed. This lets the frontend skip
/// straight past that cost the overwhelming majority of the time, only
/// calling the expensive command when this signature actually differs from
/// the last one it saw.
#[tauri::command]
pub fn wallpaper_signature() -> Result<String, String> {
    let path = wallpaper_path().map_err(|e| e.to_string())?;
    let modified = std::fs::metadata(&path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    Ok(format!("{}|{modified}", path.display()))
}

#[tauri::command]
pub fn wallpaper_accent_color() -> Result<String, String> {
    let path = wallpaper_path().map_err(|e| e.to_string())?;
    let img = image::open(&path).map_err(|e| format!("could not open wallpaper image at {}: {e}", path.display()))?;
    let (r, g, b) = dominant_color(&img);
    Ok(format!("#{r:02x}{g:02x}{b:02x}"))
}
