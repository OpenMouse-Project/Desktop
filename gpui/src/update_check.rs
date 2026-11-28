//! Update availability check against the `latest.json` that each GitHub Release
//! publishes (see `.github/workflows/release.yml`). Check-only for now: it says
//! whether a newer version is out. The file also carries the installer URL and
//! its SHA-256, which is what a future in-app download and install will use.

use std::time::Duration;

use gpui_kit::{App, Entity};
use serde::Deserialize;

use crate::app::AppView;

const LATEST_JSON_URL: &str =
    "https://github.com/OpenMouse-Project/Desktop/releases/latest/download/latest.json";
/// Background cadence. There's no rush — a flaky network shouldn't nag, and
/// the startup check plus the manual "Check for updates" button cover the
/// cases where someone actually wants a fresh answer right now.
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct UpdateInfo {
    pub version: String,
    #[serde(default)]
    pub notes: Option<String>,
}

/// Fetches `latest.json` via the system `curl` — the same dependency-free
/// convention `games::prefetch_art` uses for network access, rather than
/// pulling in an HTTP client crate for one small GET. Blocking; callers must
/// run this off the UI thread. The per-platform `url`/`signature` fields in
/// the updater JSON are ignored entirely at this stage.
pub fn fetch_latest() -> Option<UpdateInfo> {
    let latest = request_latest();
    match &latest {
        Ok(info) => crate::logging::log(&format!("[update] latest release is {}", info.version)),
        Err(error) => crate::logging::log(&format!("[update] check failed: {error}")),
    }
    latest.ok()
}

fn request_latest() -> Result<UpdateInfo, String> {
    let output = crate::proc::quiet(
        std::process::Command::new("curl")
            .args(["-sSfL", "-m", "10"])
            .arg(LATEST_JSON_URL)
            .stdin(std::process::Stdio::null())
    )
    .output()
    .map_err(|e| format!("could not run curl: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    serde_json::from_slice(&output.stdout).map_err(|e| format!("unreadable latest.json: {e}"))
}

/// True when `latest` is a newer version than `current`. Both are bare
/// `x.y.z` strings (no prerelease tag, see `APP_VERSION`'s usage in
/// `app.rs`); a missing/non-numeric component compares as `0`, so a
/// malformed string never panics — it just sorts low.
pub fn is_newer(current: &str, latest: &str) -> bool {
    fn parts(v: &str) -> Vec<u32> {
        v.split('.').map(|p| p.parse().unwrap_or(0)).collect()
    }
    parts(latest) > parts(current)
}

/// Startup check plus a 6-hour background poll, mirroring
/// `theme::spawn_wallpaper_watcher`'s shape: the fetch runs on the
/// background executor, the result lands on `AppView` via the foreground
/// executor. Detached — ends on its own once the view is dropped.
pub fn spawn_update_watcher(cx: &mut App, view: &Entity<AppView>) {
    let view = view.downgrade();
    cx.spawn(async move |cx| {
        loop {
            let info = cx.background_executor().spawn(async { fetch_latest() }).await;
            if let Some(info) = info {
                if view.update(cx, |view, cx| view.note_update_available(info, cx)).is_err() {
                    break;
                }
            } else if view.upgrade().is_none() {
                break;
            }
            cx.background_executor().timer(CHECK_INTERVAL).await;
        }
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn compares_numeric_components_not_strings() {
        assert!(is_newer("0.0.29", "0.0.30"));
        assert!(is_newer("0.0.9", "0.0.10"));
        assert!(is_newer("0.0.29", "0.1.0"));
        assert!(!is_newer("0.0.29", "0.0.29"));
        assert!(!is_newer("0.1.0", "0.0.99"));
    }

    #[test]
    fn malformed_versions_do_not_panic() {
        assert!(!is_newer("not-a-version", ""));
        assert!(is_newer("", "0.0.1"));
    }
}
