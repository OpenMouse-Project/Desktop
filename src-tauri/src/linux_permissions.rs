//! Linux-only: install the bundled udev rule (`70-openmouse.rules`, shipped
//! as an app resource) so hidapi can actually open mice.
//!
//! `.deb`/`.rpm` users never need this — the package's maintainer script
//! already drops the rule into `/etc/udev/rules.d/` at install time. This
//! exists for the portable `.AppImage`, which has no install step at all:
//! the first time it runs, the app proactively installs the rule via
//! `pkexec` (the standard PolicyKit GUI auth dialog on desktop distros) so
//! the user isn't left staring at "connecting…" forever. Idempotent — if the
//! rule is already in place (deb/rpm, previous AppImage run, manual install)
//! it does nothing, so a deb/rpm user never sees a prompt either.

#[cfg(target_os = "linux")]
const RULE_NAME: &str = "70-openmouse.rules";
#[cfg(target_os = "linux")]
const RULE_DEST: &str = "/etc/udev/rules.d/70-openmouse.rules";

/// Installs the bundled udev rule on Linux; no-ops elsewhere so the frontend
/// can fire this command unconditionally at startup.
///
/// Returns one of: `"installed"` (rule newly put in place — caller should
/// prompt to reconnect), `"already-installed"` (no change), or `"not-linux"`.
/// Errors when it existed but couldn't be installed (pkexec failed/cancelled).
#[cfg(target_os = "linux")]
#[tauri::command]
pub fn install_udev_rules(app: tauri::AppHandle) -> Result<String, String> {
    use tauri::Manager;

    let source = app
        .path()
        .resource_dir()
        .map_err(|e| e.to_string())?
        .join(RULE_NAME);

    if !source.exists() {
        return Err(format!("{RULE_NAME} not found in app resources"));
    }

    // Already present with identical content (deb/rpm maintainer script, a
    // previous AppImage run, or a manual install) — nothing to do.
    if let (Ok(existing), Ok(ours)) =
        (std::fs::read_to_string(RULE_DEST), std::fs::read_to_string(&source))
    {
        if existing.trim() == ours.trim() {
            return Ok("already-installed".to_string());
        }
    }

    // Install with root via pkexec — the standard, GUI, distro-agnostic way
    // to elevate on a desktop session. The rule file ships read-only so a
    // dubious drop doesn't add executable bits to /etc.
    use std::os::unix::process::ExitStatusExt;
    use std::process::Command;

    let install = Command::new("pkexec")
        .args(["install", "-m", "0644", source.to_str().unwrap(), RULE_DEST])
        .status();
    match install {
        Ok(status) if status.success() => {}
        Ok(status) => {
            let code = status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| format!("signal {}", status.signal().unwrap_or(0)));
            return Err(format!("udev rule install failed (pkexec exit {code})"));
        }
        Err(error) => return Err(format!("could not run pkexec: {error}")),
    }

    // Reload + retrigger existing hidraw devices so the rule applies to a
    // mouse already plugged in, not just ones connected after install.
    // Best-effort — udev reflects new rules on next boot regardless.
    let _ = Command::new("pkexec")
        .args(["udevadm", "control", "--reload-rules"])
        .status();
    let _ = Command::new("pkexec")
        .args(["udevadm", "trigger", "--subsystem-match=hidraw"])
        .status();

    Ok("installed".to_string())
}

#[cfg(not(target_os = "linux"))]
#[tauri::command]
pub fn install_udev_rules(_app: tauri::AppHandle) -> Result<String, String> {
    Ok("not-linux".to_string())
}