//! Where Windows keeps the current wallpaper (ported from the Tauri app's
//! `wallpaper.rs`, plus the slideshow/Spotlight case).

use std::path::PathBuf;

use anyhow::{bail, Context};
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

/// The wallpaper image file. `Control Panel\Desktop\WallPaper` holds the path for
/// a still picture; it is empty when a slideshow or Windows Spotlight is in use,
/// where Windows keeps the current frame in `TranscodedWallpaper` (JPEG data with
/// no file extension).
pub fn path() -> anyhow::Result<PathBuf> {
    let desktop = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Control Panel\\Desktop")
        .context("could not open Control Panel\\Desktop")?;
    let configured: String = desktop
        .get_value("WallPaper")
        .context("could not read the WallPaper registry value")?;
    if !configured.is_empty() {
        let path = PathBuf::from(&configured);
        if path.is_file() {
            return Ok(path);
        }
    }
    let transcoded = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .map(|dir| dir.join(r"Microsoft\Windows\Themes\TranscodedWallpaper"));
    match transcoded {
        Some(path) if path.is_file() => Ok(path),
        _ if configured.is_empty() => bail!("no wallpaper image (solid colour background)"),
        _ => bail!("the wallpaper file {configured} does not exist"),
    }
}
