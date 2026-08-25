//! In-memory ring buffer backing Settings' "Download Logs" button.
//!
//! Every diagnostic line hid.rs already printed via `eprintln!` (device
//! open/close, HID++ request/reply traffic, decoded errors) goes through
//! `applog::log()`/the `applog!` macro instead now, so those lines are
//! captured somewhere the user can actually get to — before this, they only
//! ever existed in `npm run tauri dev`'s own terminal, gone the moment the
//! window was launched any other way (a packaged build, `open` from
//! Finder). `eprintln!` still happens here too, so the dev-terminal
//! experience is unchanged.

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

/// Generous but bounded — a session's worth of HID++ traffic at this app's
/// actual chattiness (see hid.rs's own dedup/gating comments) comfortably
/// fits well under this before the oldest lines start rolling off.
const CAPACITY: usize = 20_000;

static BUFFER: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();

fn buffer() -> &'static Mutex<VecDeque<String>> {
    BUFFER.get_or_init(|| Mutex::new(VecDeque::with_capacity(1024)))
}

/// Records one already-formatted line: prints it (same as the `eprintln!`
/// it replaced) and appends it to the ring buffer, trimming the oldest line
/// once `CAPACITY` is exceeded.
pub fn log(line: String) {
    eprintln!("{line}");
    let mut buf = buffer().lock().unwrap();
    buf.push_back(line);
    if buf.len() > CAPACITY {
        buf.pop_front();
    }
}

/// Every line currently held, oldest first.
pub fn snapshot() -> Vec<String> {
    buffer().lock().unwrap().iter().cloned().collect()
}

/// `eprintln!`'s own call shape (`applog!("[hid] ...{x}", x = 1)`) — a
/// drop-in replacement so converting an existing `eprintln!` call site is a
/// single word swap, not a rewrite.
#[macro_export]
macro_rules! applog {
    ($($arg:tt)*) => {
        $crate::applog::log(format!($($arg)*))
    };
}

#[tauri::command]
pub fn get_logs() -> Vec<String> {
    snapshot()
}

/// Writes the current buffer to a timestamped file under the app's log
/// directory and returns its absolute path — the frontend hands that back
/// to the user (and can reveal it via the already-installed
/// `tauri-plugin-opener`, no extra plugin needed just for this).
#[tauri::command]
pub fn export_logs(app: tauri::AppHandle) -> Result<String, String> {
    use tauri::Manager;

    let dir = app.path().app_log_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = dir.join(format!("openmouse-{timestamp}.log"));

    let contents = snapshot().join("\n");
    std::fs::write(&path, contents).map_err(|e| e.to_string())?;

    Ok(path.to_string_lossy().into_owned())
}
