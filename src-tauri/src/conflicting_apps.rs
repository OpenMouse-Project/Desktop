//! Detects proprietary mouse/keyboard software that conflicts with
//! OpenMouse's HID access. These apps hold exclusive handles on the
//! same HID devices, so OpenMouse either can't open them or gets
//! garbled responses when they're running.

use sysinfo::{ProcessesToUpdate, System};

/// Known conflicting process name substrings (lowercased).
/// Matched via `contains` rather than exact equality because vendor
/// executables vary across versions.
const CONFLICTING_APPS: &[(&str, &str)] = &[
    // Razer
    ("razerappengine", "Razer Synapse"),
    ("rzenginemon", "Razer Synapse"),
    ("razer_elevation_service", "Razer Synapse"),
    ("rzsynapse", "Razer Synapse"),
    ("razer central", "Razer Central"),
    ("rzcentral", "Razer Central"),
    // Logitech
    ("lghub", "Logitech G Hub"),
    ("logitech g hub", "Logitech G Hub"),
    ("logiofirmware", "Logitech Firmware"),
    ("logitechpcsdk", "Logitech SDK"),
    ("lcore", "Logitech Gaming Software"),
    // Corsair
    ("icue", "Corsair iCUE"),
    // SteelSeries
    ("steelseries", "SteelSeries GG"),
    // HyperX
    ("ngenuity", "HyperX NGENUITY"),
    // Glorious
    ("glorious", "Glorious Core"),
    // Pulsar
    ("pulsar", "Pulsar"),
    // Endgame Gear
    ("endgamegear", "Endgame Gear"),
];

/// Process name substrings (lowercased) that would otherwise false-positive
/// against CONFLICTING_APPS above but are known NOT to hold the HID handle
/// — checked before the real match, so a process matching one of these is
/// skipped entirely regardless of what it also matches.
///
/// CONFIRMED: LGHUB Updater (`lghub_updater.exe`) is a background updater
/// service that ships and runs alongside G Hub — it matches "lghub" but
/// doesn't touch the mouse's HID interface, unlike the main G Hub app/agent
/// (`lghub.exe`, `lghub_agent.exe`). Without this exclusion, OpenMouse
/// reports G Hub as "running" and blocks connecting even when the actual
/// conflicting app is closed, as long as its updater service is still up
/// (which is most of the time — it's not tied to G Hub's own lifecycle).
const SAFE_PROCESS_SUBSTRINGS: &[&str] = &["lghub_updater"];

#[derive(serde::Serialize)]
pub struct ConflictingApp {
    pub process: String,
    pub label: String,
}

/// Returns every known conflicting app that is currently running.
#[tauri::command]
pub fn detect_conflicting_apps() -> Vec<ConflictingApp> {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);

    let running: Vec<String> = system
        .processes()
        .values()
        .map(|p| p.name().to_string_lossy().to_lowercase())
        .filter(|name| !SAFE_PROCESS_SUBSTRINGS.iter().any(|safe| name.contains(safe)))
        .collect();

    applog!("[conflicting] scanned {} processes", running.len());

    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();

    for (substr, label) in CONFLICTING_APPS {
        if seen.contains(*label) {
            continue;
        }
        if running.iter().any(|r| r.contains(*substr)) {
            seen.insert(*label);
            applog!("[conflicting] detected: {} (matched '{}')", label, substr);
            result.push(ConflictingApp {
                process: label.to_string(),
                label: label.to_string(),
            });
        }
    }

    applog!("[conflicting] found {} conflicting apps", result.len());
    result
}
