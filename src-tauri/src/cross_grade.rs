//! Bridge <-> Desktop cross-grade: switching between the two separately
//! downloadable builds (see tauri.conf.json vs tauri.bridge.conf.json —
//! distinct `identifier`s, so the OS treats them as different apps that can
//! coexist on disk).
//!
//! Two paths, both reachable from the frontend's "switch mode" action when
//! the mode being switched to isn't the one this binary was built as:
//!
//!   1. **Already installed** (the common case once someone's downloaded
//!      both once): every build self-registers its own exe path into a
//!      manifest shared by identifier — see `register_self` — the instant
//!      it starts up. Switching just launches the sibling exe and exits
//!      this process. No network, no re-download.
//!   2. **Not installed yet**: fetch the latest GitHub release, find the
//!      asset for the wanted variant + this OS, download it, and hand it to
//!      the OS's own installer/opener (`open::that` — ShellExecute on
//!      Windows, xdg-open on Linux) instead of running it ourselves. The
//!      installer's own UI (UAC prompt, package-manager confirmation,
//!      whatever the OS provides) is what actually applies it — this code
//!      never bypasses that.
//!
//! The manifest lives under the OS's base app-data dir (NOT the
//! identifier-scoped one Tauri normally hands out — that would put each
//! variant's manifest file in a different place, defeating the point),
//! keyed by variant name, so either build can read what the other one
//! wrote.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const MANIFEST_DIR: &str = "OpenMouse";
const MANIFEST_FILE: &str = "installed-variants.json";
const RELEASES_API: &str = "https://api.github.com/repos/OpenMouse-Project/Desktop/releases/latest";

#[derive(Clone, Serialize, Deserialize)]
pub struct InstalledVariant {
    pub variant: String,
    pub exe_path: String,
    pub version: String,
}

#[derive(Default, Serialize, Deserialize)]
struct Manifest {
    variants: Vec<InstalledVariant>,
}

#[derive(Deserialize)]
struct GithubReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Deserialize)]
struct GithubRelease {
    assets: Vec<GithubReleaseAsset>,
}

fn manifest_path(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .data_dir()
        .map_err(|e| format!("couldn't resolve the OS data directory: {e}"))?;
    Ok(base.join(MANIFEST_DIR).join(MANIFEST_FILE))
}

fn read_manifest(path: &PathBuf) -> Manifest {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok())
        .unwrap_or_default()
}

/// Called once from the frontend's startup effect (App.tsx / App.bridge.tsx
/// — each hardcodes its own variant name, since that's a build-time fact
/// only the frontend entry point actually knows; the Rust side isn't
/// feature-gated per variant, so it has no other way to tell). Records
/// this executable's path and version in the shared manifest so the
/// *other* variant, if and when it's ever run, can find it without a
/// download. Best-effort: a write failure here shouldn't block startup,
/// just means cross-grade falls back to the download path next time
/// someone tries it.
#[tauri::command]
pub fn register_variant(app: AppHandle, variant: String) {
    let path = match manifest_path(&app) {
        Ok(p) => p,
        Err(e) => {
            applog!("[cross-grade] couldn't resolve manifest path: {e}");
            return;
        }
    };
    let Ok(exe_path) = std::env::current_exe() else {
        applog!("[cross-grade] couldn't resolve current_exe(), skipping self-registration");
        return;
    };
    let mut manifest = read_manifest(&path);
    manifest.variants.retain(|v| v.variant != variant);
    manifest.variants.push(InstalledVariant {
        variant: variant.to_string(),
        exe_path: exe_path.to_string_lossy().into_owned(),
        version: app.package_info().version.to_string(),
    });

    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            applog!("[cross-grade] couldn't create manifest dir: {e}");
            return;
        }
    }
    match serde_json::to_string_pretty(&manifest) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                applog!("[cross-grade] couldn't write manifest: {e}");
            }
        }
        Err(e) => applog!("[cross-grade] couldn't serialize manifest: {e}"),
    }
}

/// What's already installed, per the shared manifest — a stale entry
/// (uninstalled since, or the exe moved) is possible since nothing prunes
/// this on uninstall; the frontend should treat "listed" as "probably
/// there" and let `switch_to_installed_variant`'s spawn failure be the
/// real check.
#[tauri::command]
pub fn get_installed_variants(app: AppHandle) -> Result<Vec<InstalledVariant>, String> {
    Ok(read_manifest(&manifest_path(&app)?).variants)
}

/// Launches the already-installed sibling build and exits this process.
/// This is the whole "auto-apply" path when nothing needs downloading.
#[tauri::command]
pub fn switch_to_installed_variant(app: AppHandle, exe_path: String) -> Result<(), String> {
    std::process::Command::new(&exe_path)
        .spawn()
        .map_err(|e| format!("couldn't launch {exe_path}: {e}"))?;
    app.exit(0);
    Ok(())
}

/// Fetches the latest GitHub release and picks out the installer asset for
/// `variant` ("bridge" or "full-desktop") on this OS, by filename — Bridge
/// and Desktop builds get distinct productNames ("OpenMouse Bridge" /
/// "OpenMouse Desktop"), which is what shows up in tauri-action's installer
/// filenames, so matching on that word plus this OS's usual installer
/// extension is enough to tell the two apart without hardcoding exact
/// version-numbered filenames.
#[tauri::command]
pub async fn find_installer_asset(variant: String) -> Result<Option<(String, String)>, String> {
    let name_marker = match variant.as_str() {
        "bridge" => "Bridge",
        "full-desktop" | "desktop" => "Desktop",
        other => return Err(format!("unknown variant '{other}'")),
    };
    let ext: &[&str] = if cfg!(target_os = "windows") {
        &[".exe", ".msi"]
    } else if cfg!(target_os = "linux") {
        &[".AppImage", ".deb", ".rpm"]
    } else {
        &[".dmg", ".app.tar.gz"]
    };

    let client = reqwest::Client::new();
    let response = client
        .get(RELEASES_API)
        // GitHub's API rejects requests with no User-Agent.
        .header("User-Agent", "openmouse-desktop-cross-grade")
        .send()
        .await
        .map_err(|e| format!("couldn't reach GitHub releases: {e}"))?
        .error_for_status()
        .map_err(|e| format!("GitHub releases returned an error: {e}"))?;
    let release: GithubRelease = response
        .json()
        .await
        .map_err(|e| format!("couldn't parse the release response: {e}"))?;

    let found = release.assets.into_iter().find(|asset| {
        asset.name.contains(name_marker) && ext.iter().any(|e| asset.name.ends_with(e))
    });
    Ok(found.map(|a| (a.name, a.browser_download_url)))
}

/// Downloads `url` to a temp file named `file_name`, then hands it to the
/// OS's own opener (ShellExecute on Windows, xdg-open on Linux/macOS) —
/// NOT something this code runs directly. For an installer that means the
/// normal install UI (UAC prompt on Windows, a package-manager or archive
/// handler on Linux); this app never elevates or executes the downloaded
/// file itself. Exits this process right after handing off, matching
/// switch_to_installed_variant's behavior, so the installer isn't fighting
/// a running instance of the app it's about to replace or sit alongside.
#[tauri::command]
pub async fn download_and_launch_installer(
    app: AppHandle,
    url: String,
    file_name: String,
) -> Result<(), String> {
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("User-Agent", "openmouse-desktop-cross-grade")
        .send()
        .await
        .map_err(|e| format!("download failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("download failed: {e}"))?;
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("download failed while reading the response: {e}"))?;

    let dest = std::env::temp_dir().join(file_name);
    tokio::fs::write(&dest, &bytes)
        .await
        .map_err(|e| format!("couldn't save the installer to {}: {e}", dest.display()))?;

    open::that(&dest).map_err(|e| format!("couldn't open the downloaded installer: {e}"))?;
    app.exit(0);
    Ok(())
}
