//! Backs the Games page's "is this game currently running" indicator.
//!
//! `public/games.json` lists each game's known executable names (Windows
//! and non-Windows variants side by side, e.g. `["cs2.exe", "cs2"]`) — the
//! frontend already owns that file and the matching logic against it; this
//! module's only job is answering "what's actually running right now",
//! platform by platform, so the frontend has something to match against.
//!
//! Uses `sysinfo` (already a dependency — see resource_monitor.rs) instead
//! of shelling out to `ps`/`tasklist`: this poll runs every few seconds for
//! as long as the Games page is open, and spawning a whole subprocess that
//! often to answer "list processes" is real, avoidable overhead — sysinfo
//! reads the same information via a direct syscall/sysctl, no fork/exec.

use std::sync::Mutex;

use sysinfo::{ProcessesToUpdate, System};

pub struct ProcessListState(Mutex<System>);

impl Default for ProcessListState {
    fn default() -> Self {
        Self(Mutex::new(System::new()))
    }
}

/// Every currently-running process's executable base name (e.g. `"cs2"` or
/// `"cs2.exe"`, whatever the OS reports), lowercased so the frontend can
/// match case-insensitively without needing to know this platform's own
/// casing conventions.
#[tauri::command]
pub fn running_process_names(state: tauri::State<'_, ProcessListState>) -> Vec<String> {
    let mut system = state.0.lock().unwrap();
    system.refresh_processes(ProcessesToUpdate::All, true);
    system
        .processes()
        .values()
        .map(|process| process.name().to_string_lossy().to_lowercase())
        .collect()
}
