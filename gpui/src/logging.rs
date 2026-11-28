//! Log files and crash reports. Both stay on this computer: nothing here sends
//! anything anywhere.
//!
//! `<local data dir>/openmouse/logs` holds `openmouse.log`, the previous one
//! (`openmouse.old.log`) and `crash-*.txt` reports. A report the user has not
//! been told about yet is announced on the next launch (see `app/crash.rs`);
//! once shown or dismissed it is renamed to `crash-*.seen.txt`.
//!
//! Every line goes through [`redact`] before it is written.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Size at which the log rolls over; with the old file, at most twice this.
const MAX_LOG_BYTES: u64 = 1024 * 1024;
const MAX_CRASH_REPORTS: usize = 10;
/// Log lines copied into a crash report.
const REPORT_TAIL_LINES: usize = 100;

struct Log {
    dir: PathBuf,
    /// The last line written and how many times it has repeated since, so a
    /// failure on every poll does not push everything else out of the file.
    last: String,
    repeats: u32,
}

static LOG: OnceLock<Mutex<Log>> = OnceLock::new();

/// Where the logs and crash reports are.
pub fn dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join("openmouse").join("logs"))
}

/// Starts logging to disk, takes over the shared crates' diagnostics and
/// installs the crash handler. Until this runs (tests, examples) lines only go
/// to stderr.
pub fn init() {
    let Some(dir) = dir() else { return };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    if LOG.set(Mutex::new(Log { dir: dir.clone(), last: String::new(), repeats: 0 })).is_err() {
        return;
    }
    let _ = openmouse_hid::LOG_SINK.set(device_line);
    let _ = openmouse_presence::LOG_SINK.set(|line| log(&line));
    log(&format!("[app] OpenMouse Desktop {} on {}", env!("CARGO_PKG_VERSION"), system()));

    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".to_string());
        let location = info.location().map(ToString::to_string).unwrap_or_default();
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        let thread = thread.name().unwrap_or("unnamed");
        log(&format!("[crash] thread `{thread}` panicked at {location}: {message}"));
        write_report(&dir, thread, &location, &message, &backtrace);
        previous(info);
    }));
}

/// Appends one line to the log.
pub fn log(line: &str) {
    let line = redact(line);
    if cfg!(debug_assertions) || LOG.get().is_none() {
        eprintln!("{line}");
    }
    let Some(log) = LOG.get() else { return };
    // The crash handler logs too, and must never wait on a lock its own thread
    // may hold; everyone else waits their turn.
    let mut log = if std::thread::panicking() {
        match log.try_lock() {
            Ok(log) => log,
            Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => return,
        }
    } else {
        log.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    };
    if line == log.last {
        log.repeats += 1;
        return;
    }
    let mut text = String::new();
    if log.repeats > 0 {
        text = format!("{} (previous line repeated {} more times)\n", timestamp(), log.repeats);
    }
    text.push_str(&format!("{} {line}\n", timestamp()));
    log.repeats = 0;
    log.last = line;
    append(&log.dir, &text);
}

/// The HID transport narrates every report it moves, bytes included. Only what
/// says something about the connection or a failure is worth keeping.
fn device_line(line: String) {
    if is_device_event(&line) {
        log(&line);
    }
}

fn is_device_event(line: &str) -> bool {
    const KEEP: [&str; 7] =
        ["failed", "Err(", "unreadable", "unparsable", "stale", "dropped the handles", "successfully opened"];
    KEEP.iter().any(|word| line.contains(word))
}

fn append(dir: &Path, text: &str) {
    let path = dir.join("openmouse.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() + text.len() as u64 > MAX_LOG_BYTES) {
        let _ = std::fs::rename(&path, dir.join("openmouse.old.log"));
    }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(text.as_bytes());
    }
}

fn write_report(dir: &Path, thread: &str, location: &str, message: &str, backtrace: &str) {
    let tail = std::fs::read_to_string(dir.join("openmouse.log")).unwrap_or_default();
    let lines: Vec<&str> = tail.lines().collect();
    let tail = lines[lines.len().saturating_sub(REPORT_TAIL_LINES)..].join("\n");
    let report = format!(
        "OpenMouse Desktop crash report\n\
         This file is only on your computer. Nothing has been sent.\n\n\
         Version:  {}\n\
         System:   {}\n\
         Time:     {}\n\
         Thread:   {thread}\n\
         Location: {}\n\
         Message:  {}\n\n\
         Backtrace:\n{}\n\n\
         Recent log:\n{tail}\n",
        env!("CARGO_PKG_VERSION"),
        system(),
        timestamp(),
        redact(location),
        redact(message),
        backtrace.lines().map(redact).collect::<Vec<_>>().join("\n"),
    );
    let millis = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let _ = std::fs::write(dir.join(format!("crash-{millis}.txt")), report);
    // Names sort by time, so the oldest come first.
    let reports = crash_reports(dir);
    for old in &reports[..reports.len().saturating_sub(MAX_CRASH_REPORTS)] {
        let _ = std::fs::remove_file(old);
    }
}

fn crash_reports(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut reports: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("crash-") && name.ends_with(".txt"))
        })
        .collect();
    reports.sort();
    reports
}

/// The newest crash report the user has not been told about.
pub fn unseen_report() -> Option<PathBuf> {
    newest_unseen(&dir()?)
}

fn newest_unseen(dir: &Path) -> Option<PathBuf> {
    crash_reports(dir).into_iter().rev().find(|path| !path.to_string_lossy().ends_with(".seen.txt"))
}

/// Marks every report as seen and returns where `report` is now.
pub fn mark_seen(report: &Path) -> PathBuf {
    let seen = |path: &Path| path.with_extension("seen.txt");
    let mut now = report.to_path_buf();
    if let Some(dir) = report.parent() {
        for path in crash_reports(dir) {
            if !path.to_string_lossy().ends_with(".seen.txt")
                && std::fs::rename(&path, seen(&path)).is_ok()
                && path == report
            {
                now = seen(&path);
            }
        }
    }
    now
}

fn system() -> String {
    format!(
        "{} ({})",
        sysinfo::System::long_os_version().unwrap_or_else(|| std::env::consts::OS.to_string()),
        std::env::consts::ARCH
    )
}

/// UTC, as `2026-01-31 13:05:09Z`.
fn timestamp() -> String {
    format_utc(SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0))
}

fn format_utc(seconds: u64) -> String {
    let (days, rest) = ((seconds / 86_400) as i64, seconds % 86_400);
    // Howard Hinnant's days-to-civil-date algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}Z", rest / 3600, rest / 60 % 60, rest % 60)
}

/// Takes out what could identify a person or one particular device: the home
/// folder and user name in paths, device paths (Windows ones carry instance ids
/// and Bluetooth addresses), serial numbers and raw report bytes.
pub fn redact(line: &str) -> String {
    let mut out = line.to_string();
    if let Some(home) = dirs::home_dir().and_then(|h| h.to_str().map(str::to_string)) {
        if home.len() > 3 {
            for form in [home.replace('\\', "\\\\"), home.replace('\\', "/"), home] {
                out = out.replace(&form, "~");
            }
        }
    }
    // Someone else's home folder, or ours spelled differently.
    for marker in ["\\Users\\", "\\\\Users\\\\", "/Users/", "/home/"] {
        out = replace_after(&out, marker, |c| !matches!(c, '/' | '\\' | '"' | '\''), "<user>");
    }
    // Device paths, plain or escaped as `{:?}` prints them.
    for marker in ["\\\\\\\\?\\\\", "\\\\?\\", "IOService:"] {
        out = replace_after(&out, marker, |c| !c.is_whitespace() && c != '"', "<device>");
    }
    redact_bytes(&redact_serials(&out))
}

/// Replaces the run of `part` characters following each `marker`.
fn replace_after(text: &str, marker: &str, part: impl Fn(char) -> bool, with: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(marker) {
        let start = at + marker.len();
        let length = rest[start..].find(|c| !part(c)).unwrap_or(rest.len() - start);
        out.push_str(&rest[..start]);
        if length > 0 {
            out.push_str(with);
        }
        rest = &rest[start + length..];
    }
    out.push_str(rest);
    out
}

/// `serial: X`, `serialNumber="X"`, `serial_number: Some("X")` and the like.
fn redact_serials(text: &str) -> String {
    // ASCII lowercasing keeps byte positions the same as in `text`.
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut done = 0;
    let mut from = 0;
    while let Some(found) = lower[from..].find("serial") {
        let after = from + found + "serial".len();
        from = after;
        let key = text[after..]
            .find(|c: char| !(c.is_ascii_alphabetic() || matches!(c, '_' | ' ' | '"' | '\'')))
            .map(|n| after + n);
        let Some(separator) = key.filter(|&i| text[i..].starts_with([':', '='])) else { continue };
        let value = separator + 1;
        let end = text[value..].find([',', ';', '}', '\n']).map_or(text.len(), |n| value + n);
        out.push_str(&text[done..value]);
        out.push_str(" <redacted>");
        done = end;
        from = end;
    }
    out.push_str(&text[done..]);
    out
}

/// `[1, 2, 3]` and `[0a, ff]`.
fn redact_bytes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('[') {
        out.push_str(&rest[..=at]);
        rest = &rest[at + 1..];
        let Some(end) = rest.find(']') else { break };
        let inside = &rest[..end];
        if inside.contains(',') && inside.chars().all(|c| c.is_ascii_hexdigit() || matches!(c, 'x' | ',' | ' ')) {
            out.push_str("bytes");
            rest = &rest[end..];
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_lose_the_user_name() {
        assert_eq!(redact(r"open C:\Users\Jane Doe\file failed"), r"open C:\Users\<user>\file failed");
        assert_eq!(redact("at /home/jane/src/main.rs:3"), "at /home/<user>/src/main.rs:3");
        if let Some(home) = dirs::home_dir() {
            let line = format!("missing {}", home.join("settings.json").display());
            assert!(redact(&line).starts_with("missing ~"), "{}", redact(&line));
        }
    }

    #[test]
    fn device_paths_serials_and_bytes_are_removed() {
        let path = r"descriptor for \\?\HID#VID_046D&PID_C547#7&2f6a&0&0000#{4d1e}: open failed";
        assert_eq!(redact(path), r"descriptor for \\?\<device> open failed");
        let escaped = format!("{:?}", r"\\?\HID#VID_046D#7&2f6a");
        assert!(!redact(&escaped).contains("2f6a"), "{}", redact(&escaped));
        assert_eq!(redact("serialNumber: AB12CD, ok"), "serialNumber: <redacted>, ok");
        assert_eq!(redact(r#"{"serial":"AB12"}"#), r#"{"serial": <redacted>}"#);
        assert_eq!(redact("the serial port is busy"), "the serial port is busy");
        assert_eq!(redact("[hid] done: Ok([1, 2, 255])"), "[hid] done: Ok([bytes])");
        assert_eq!(redact("[js error] data [0a, ff]"), "[js error] data [bytes]");
    }

    #[test]
    fn only_connection_events_and_failures_are_kept_from_the_transport() {
        assert!(is_device_event("[hid] hid_open: failed to open x: denied"));
        assert!(is_device_event("[hid] hid_send_report done: Err(\"gone\")"));
        assert!(!is_device_event("[hid] hid_send_report done: Ok(())"));
        assert!(!is_device_event("[hid] reader k: got 8 bytes, reportId=0x01 data=[00]"));
    }

    #[test]
    fn dates_are_formatted_in_utc() {
        assert_eq!(format_utc(0), "1970-01-01 00:00:00Z");
        assert_eq!(format_utc(951_827_696), "2000-02-29 12:34:56Z");
    }

    #[test]
    fn log_rolls_over_and_reports_are_announced_once() {
        let dir = std::env::temp_dir().join(format!("openmouse-log-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let big = "x".repeat(MAX_LOG_BYTES as usize);
        append(&dir, &big);
        append(&dir, "next\n");
        assert_eq!(std::fs::read_to_string(dir.join("openmouse.log")).unwrap(), "next\n");
        assert!(dir.join("openmouse.old.log").is_file());

        write_report(&dir, "main", "src/a.rs:1:1", "boom serial=AB12", "0: frame");
        let report = newest_unseen(&dir).expect("a new report");
        let text = std::fs::read_to_string(&report).unwrap();
        assert!(text.contains("boom serial= <redacted>") && text.contains("next"), "{text}");
        let seen = mark_seen(&report);
        assert!(seen.is_file() && seen.to_string_lossy().ends_with(".seen.txt"));
        assert_eq!(newest_unseen(&dir), None);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
