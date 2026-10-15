//! Launch on login, written natively per platform (the Tauri app uses its
//! autostart plugin, which has no equivalent here):
//!
//! - macOS: a LaunchAgent plist in `~/Library/LaunchAgents`
//! - Windows: a value under `HKCU\...\CurrentVersion\Run`
//! - Linux: an XDG autostart entry in `~/.config/autostart`
//!
//! Each starts the current executable with [`ARG`], so the app can tell an
//! OS-started launch from a user-started one. The truth lives in the OS entry,
//! not in settings: [`is_enabled`] reads it back, as the Tauri app's
//! `isEnabled()` does.

/// Passed on the command line by the OS entry (same flag as the Tauri app).
pub const ARG: &str = "--autostart";

const NAME: &str = "OpenMouse";

fn exe() -> Result<std::path::PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("could not locate the app: {e}"))
}

#[cfg(target_os = "macos")]
mod imp {
    use super::*;

    const LABEL: &str = "app.openmouse.desktop.gpui";

    fn plist_path() -> Result<std::path::PathBuf, String> {
        dirs::home_dir()
            .map(|h| h.join("Library/LaunchAgents").join(format!("{LABEL}.plist")))
            .ok_or_else(|| "no home directory".to_string())
    }

    fn xml_escape(text: &str) -> String {
        text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
    }

    pub fn is_enabled() -> bool {
        plist_path().is_ok_and(|p| p.is_file())
    }

    pub fn enable() -> Result<(), String> {
        let path = plist_path()?;
        let exe = exe()?;
        let body = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\">\n<dict>\n\
             \t<key>Label</key>\n\t<string>{LABEL}</string>\n\
             \t<key>ProgramArguments</key>\n\t<array>\n\t\t<string>{}</string>\n\t\t<string>{ARG}</string>\n\t</array>\n\
             \t<key>RunAtLoad</key>\n\t<true/>\n\
             </dict>\n</plist>\n",
            xml_escape(&exe.to_string_lossy()),
        );
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, body).map_err(|e| e.to_string())
    }

    pub fn disable() -> Result<(), String> {
        match std::fs::remove_file(plist_path()?) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::*;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

    pub fn is_enabled() -> bool {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(RUN_KEY, KEY_READ)
            .and_then(|key| key.get_value::<String, _>(NAME))
            .is_ok()
    }

    pub fn enable() -> Result<(), String> {
        let exe = exe()?;
        let key = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(RUN_KEY, KEY_WRITE)
            .map_err(|e| e.to_string())?;
        key.set_value(NAME, &format!("\"{}\" {ARG}", exe.display()))
            .map_err(|e| e.to_string())
    }

    pub fn disable() -> Result<(), String> {
        let key = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(RUN_KEY, KEY_WRITE)
            .map_err(|e| e.to_string())?;
        match key.delete_value(NAME) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod imp {
    use super::*;

    fn entry_path() -> Result<std::path::PathBuf, String> {
        dirs::config_dir()
            .map(|c| c.join("autostart").join("openmouse.desktop"))
            .ok_or_else(|| "no config directory".to_string())
    }

    pub fn is_enabled() -> bool {
        entry_path().is_ok_and(|p| p.is_file())
    }

    pub fn enable() -> Result<(), String> {
        let path = entry_path()?;
        let exe = exe()?;
        let body = format!(
            "[Desktop Entry]\nType=Application\nName={NAME}\nExec=\"{}\" {ARG}\nX-GNOME-Autostart-enabled=true\n",
            exe.display()
        );
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, body).map_err(|e| e.to_string())
    }

    pub fn disable() -> Result<(), String> {
        match std::fs::remove_file(entry_path()?) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

pub use imp::{disable, enable, is_enabled};

/// Whether this process was started by the OS entry rather than by the user.
pub fn launched_by_os() -> bool {
    std::env::args().any(|arg| arg == ARG)
}
