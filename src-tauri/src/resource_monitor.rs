//! Backs Settings' resource-monitor section — CPU% and RAM for this app's
//! own process, sampled on request by the frontend.
//!
//! `sysinfo::System::cpu_usage()` needs at least two refreshes separated by
//! real wall-clock time to report a meaningful percentage (a single
//! refresh has no prior sample to diff against and reads 0). So the
//! `System` has to survive between calls, not get rebuilt fresh each
//! time — it's Tauri-managed state (`ResourceMonitorState`), one refresh
//! per `sample_resource_usage()` call, and the frontend's own poll
//! interval (see GamesPage-style polling elsewhere in this app) is what
//! provides that time gap. History, min/max, and the graph itself all live
//! on the frontend — this only ever answers "right now."

use std::sync::Mutex;

use serde::Serialize;
use sysinfo::{Pid, ProcessesToUpdate, System};

pub struct ResourceMonitorState(Mutex<System>);

impl Default for ResourceMonitorState {
    fn default() -> Self {
        Self(Mutex::new(System::new()))
    }
}

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSample {
    /// Percent of a single CPU core (0-100 per core, so a busy multi-core
    /// app can read above 100) — sysinfo's own convention, matches `ps`.
    cpu_percent: f32,
    memory_bytes: u64,
    /// Milliseconds since the Unix epoch, for the frontend's own graph.
    timestamp_ms: u64,
}

#[tauri::command]
pub fn sample_resource_usage(state: tauri::State<'_, ResourceMonitorState>) -> Option<ResourceSample> {
    let pid = Pid::from_u32(std::process::id());
    let mut system = state.0.lock().unwrap();
    system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    let process = system.process(pid)?;

    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    Some(ResourceSample {
        cpu_percent: process.cpu_usage(),
        memory_bytes: process.memory(),
        timestamp_ms,
    })
}
