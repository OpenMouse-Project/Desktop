//! Tauri command wrappers around the shared `openmouse-presence` crate
//! (`crates/openmouse-presence`), which holds the actual Rich Presence logic.

use openmouse_presence::Presence;
use tauri::State;

pub use openmouse_presence::Presence as DiscordRpcState;

/// Routes the shared crate's diagnostics into the in-app log buffer.
pub fn init_logging() {
    let _ = openmouse_presence::LOG_SINK.set(crate::applog::log);
}

#[tauri::command]
pub fn enable(state: State<Presence>) -> Result<(), String> {
    state.enable()
}

#[tauri::command]
pub fn update_activity(
    state: State<Presence>,
    details: String,
    state_text: String,
) -> Result<(), String> {
    state.update_activity(details, state_text)
}

#[tauri::command]
pub fn disable(state: State<Presence>) -> Result<(), String> {
    state.disable()
}

// Upstream renamed these (bare `enable`/`disable` collided with the autostart
// plugin's commands, so the toggle never reached the backend). Both spellings
// are registered so a frontend works either side of that rename.

#[tauri::command]
pub fn discord_enable(state: State<Presence>) -> Result<(), String> {
    state.enable()
}

#[tauri::command]
pub fn discord_update_activity(
    state: State<Presence>,
    details: String,
    state_text: String,
) -> Result<(), String> {
    state.update_activity(details, state_text)
}

#[tauri::command]
pub fn discord_status(state: State<Presence>) -> bool {
    state.is_connected()
}

#[tauri::command]
pub fn discord_disable(state: State<Presence>) -> Result<(), String> {
    state.disable()
}
