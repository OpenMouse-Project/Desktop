use std::sync::Mutex;

use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use tauri::State;

const CLIENT_ID: &str = "1541591980281299056";

fn build_activity(name: String, details: String, state_text: String) -> activity::Activity<'static> {
    activity::Activity::new()
        .name(name)
        .details(details)
        .state(state_text)
        .buttons(vec![activity::Button::new(
            "OpenMouse",
            "https://openmouse.dev",
        )])
}

pub struct DiscordRpcState {
    client: Mutex<Option<DiscordIpcClient>>,
}

impl Default for DiscordRpcState {
    fn default() -> Self {
        Self {
            client: Mutex::new(None),
        }
    }
}

#[tauri::command]
pub fn enable(state: State<DiscordRpcState>) -> Result<(), String> {
    applog!("[discord] enable requested");
    let mut rpc = state.client.lock().map_err(|error| error.to_string())?;
    if rpc.is_some() {
        applog!("[discord] already connected");
        return Ok(());
    }

    applog!("[discord] opening IPC connection for client {CLIENT_ID}");
    let mut client = DiscordIpcClient::new(CLIENT_ID);
    if let Err(error) = client.connect() {
        applog!("[discord] connection failed: {error}");
        return Err(error.to_string());
    }
    applog!("[discord] IPC connection established");

    let (name, details) = if cfg!(debug_assertions) {
        ("OpenMouse Dev", "Dev Mode")
    } else {
        ("OpenMouse", "OpenMouse Desktop")
    };
    let activity = build_activity(
        name.to_string(),
        details.to_string(),
        "Managing mouse settings".to_string(),
    );
    applog!("[discord] setting initial activity");
    if let Err(error) = client.set_activity(activity) {
        applog!("[discord] initial activity failed: {error}");
        return Err(error.to_string());
    }
    applog!("[discord] initial activity set");

    *rpc = Some(client);
    applog!("[discord] connected");
    Ok(())
}

#[tauri::command]
pub fn update_activity(
    state: State<DiscordRpcState>,
    details: String,
    state_text: String,
) -> Result<(), String> {
    applog!("[discord] activity update requested: details={details:?}, state={state_text:?}");
    let mut rpc = state.client.lock().map_err(|error| error.to_string())?;
    let Some(client) = rpc.as_mut() else {
        applog!("[discord] activity update skipped: RPC is not enabled");
        return Err("Discord RPC is not enabled".to_string());
    };

    if let Err(error) = client.set_activity(build_activity(
        "OpenMouse".to_string(),
        details,
        state_text,
    )) {
        applog!("[discord] activity update failed: {error}");
        return Err(error.to_string());
    }
    applog!("[discord] activity updated");
    Ok(())
}

#[tauri::command]
pub fn disable(state: State<DiscordRpcState>) -> Result<(), String> {
    applog!("[discord] disable requested");
    let mut rpc = state.client.lock().map_err(|error| error.to_string())?;
    if let Some(mut client) = rpc.take() {
        if let Err(error) = client.close() {
            applog!("[discord] disconnect failed: {error}");
            return Err(error.to_string());
        }
        applog!("[discord] disconnected");
    } else {
        applog!("[discord] already disconnected");
    }
    Ok(())
}
