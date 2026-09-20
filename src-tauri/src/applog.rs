//! In-memory ring buffer backing Settings' "Download Logs" button.
//!
//! Every diagnostic line hid.rs already printed via `eprintln!` (device
//! open/close, HID++ request/reply traffic, decoded errors) goes through
//! `applog::log()`/the `applog!` macro instead now, so those lines are
//! captured somewhere the user can actually get to — before this, they only
//! ever existed in `npm run tauri dev`'s own terminal, gone the moment the
//! window was launched any other way (a packaged build, `open` from
//! Finder). The line is still written to stderr as well, so the
//! dev-terminal experience is unchanged — but best-effort, never fatally:
//! see `log()`.

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::{LazyLock, OnceLock};

use parking_lot::Mutex;

/// Generous but bounded — a session's worth of HID++ traffic at this app's
/// actual chattiness (see hid.rs's own dedup/gating comments) comfortably
/// fits well under this before the oldest lines start rolling off.
const CAPACITY: usize = 20_000;

static BUFFER: LazyLock<Mutex<VecDeque<String>>> =
    LazyLock::new(|| Mutex::new(VecDeque::with_capacity(1024)));

/// One always-on log file, opened once at startup. `None` (and staying
/// `None`) means the file could not be opened — logging is best-effort here
/// exactly as it is on stderr, never fatal.
static LOG_FILE: OnceLock<Mutex<Option<File>>> = OnceLock::new();

/// Caps the on-disk log at 2 MiB, then moves it aside to `.1` and starts a
/// fresh one. `log()` runs for every HID++/Razer round trip, so a long session
/// writes a lot, and a log file nobody can open is no better than none.
const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;

/// Points the always-on log at `<dir>/openmouse-desktop.log`, creating the
/// directory and rotating an oversized file first. Called once from `run()`'s
/// setup with Tauri's per-app log directory.
///
/// This is what makes a reported failure diagnosable after the fact: stderr
/// belongs to whatever terminal launched the app (nothing at all for a
/// packaged build), and the ring buffer below dies with the process — so a
/// "clicked Apply and it errored" report used to leave nothing to read.
pub fn init_file_logging(dir: &Path) {
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let path = dir.join("openmouse-desktop.log");
    if std::fs::metadata(&path).map(|meta| meta.len() >= MAX_LOG_BYTES).unwrap_or(false) {
        let _ = std::fs::rename(&path, dir.join("openmouse-desktop.log.1"));
    }
    let file = OpenOptions::new().create(true).append(true).open(&path).ok();
    let _ = LOG_FILE.set(Mutex::new(file));
}

/// `YYYY-MM-DDTHH:MM:SSZ` (UTC) for the log prefix. This crate has no date
/// library and one is not worth a dependency for a prefix; convert to local
/// time by hand when reading it against a wall clock.
fn utc_timestamp() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0);
    let days = (seconds / 86_400) as i64;
    let rest = seconds % 86_400;
    // Days since the epoch → civil date, Howard Hinnant's algorithm.
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 { month_prime + 3 } else { month_prime - 9 };
    let year = if month <= 2 { year + 1 } else { year };
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3_600,
        (rest % 3_600) / 60,
        rest % 60
    )
}

/// Records one already-formatted line: writes it to stderr, appends it to the
/// always-on log file and to the ring buffer, trimming the oldest line once
/// `CAPACITY` is exceeded.
pub fn log(line: String) {
    // `eprintln!` PANICS when the write fails, and a panic here aborts the
    // whole app (the panic handler's own attempt to print the message fails
    // the same way, so it never gets to unwind into anything recoverable).
    // CONFIRMED as a real crash, not a theory: the report from 2026-09-19
    // 19:33 has the panic unwinding out of `applog::log` →
    // `std::io::stdio::__eprint` on the main thread *inside `hid_open`* —
    // the dev server's terminal had gone away, so stderr was a closed pipe
    // and every device connect killed the app. Diagnostics must never be
    // able to do that: both writes below drop their result, and the ring
    // buffer is what "Download Logs" reads either way.
    let line = format!("{} {line}", utc_timestamp());
    let _ = writeln!(std::io::stderr(), "{line}");
    if let Some(file) = LOG_FILE.get() {
        // A plain file write: nothing in here can run a run loop or call back
        // into the app, so unlike hid.rs's `HidApi` handle this lock cannot be
        // re-entered by the nested invokes a pumped main-thread run loop
        // dispatches (see `with_hid_api` there).
        let mut guard = file.lock();
        if let Some(handle) = guard.as_mut() {
            let _ = writeln!(handle, "{line}");
        }
    }
    let mut buf = BUFFER.lock();
    buf.push_back(line);
    if buf.len() > CAPACITY {
        buf.pop_front();
    }
}

/// Every line currently held, oldest first.
pub fn snapshot() -> Vec<String> {
    BUFFER.lock().iter().cloned().collect()
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

/// Lets the frontend record a line into the same buffer as the Rust side's
/// own `applog!` call sites — for diagnostics that only make sense from JS
/// (e.g. which URL a `fetch()` actually resolved through, or why it fell
/// back), so "Download Logs" captures the full picture rather than only the
/// backend half of it.
#[tauri::command]
pub fn log_line(line: String) {
    log(line);
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
