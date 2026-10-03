use parking_lot::Mutex;

use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use tauri::State;

const CLIENT_ID: &str = "1555824483389149245";
fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

/// Details + state + start timestamp, plus a button to the website. The
/// display name comes from the Discord portal registration — sending a custom
/// `name` ("OpenMouse Dev" in dev builds) mismatches the registered app and
/// Discord ACKs the payload without ever rendering it, so it stays out.
fn build_activity(details: String, state_text: String) -> activity::Activity<'static> {
    activity::Activity::new()
        .details(details)
        .state(state_text)
        .timestamps(activity::Timestamps::new().start(now_millis()))
        .assets(activity::Assets::new().large_image("logo").large_text("OpenMouse"))
        .buttons(vec![activity::Button::new(
            "OpenMouse Website",
            "https://openmouse.app",
        )])
}

fn initial_activity() -> activity::Activity<'static> {
    let details = if cfg!(debug_assertions) {
        concat!("Dev Mode: Build ", env!("CARGO_PKG_VERSION"))
    } else {
        "OpenMouse Desktop"
    };
    build_activity(details.to_string(), "Managing mouse settings".to_string())
}

/// Sends the activity and waits for Discord's reply, so a rejection surfaces
/// as an error instead of a silent no-show. Discord answers every SET_ACTIVITY
/// with the same nonce; `evt == "ERROR"` means it refused the payload and the
/// presence never displays.
fn set_activity_and_confirm(
    client: &mut DiscordIpcClient,
    activity: activity::Activity<'static>,
) -> Result<(), String> {
    if let Err(error) = client.set_activity(activity) {
        applog!("[discord] activity send failed: {error}");
        return Err(error.to_string());
    }
    match client.recv() {
        Ok((_opcode, response)) => {
            applog!("[discord] activity response: {response}");
            if response.get("evt").and_then(|evt| evt.as_str()) == Some("ERROR") {
                let detail = response
                    .get("data")
                    .map(|data| data.to_string())
                    .unwrap_or_else(|| "unknown error".to_string());
                applog!("[discord] activity rejected: {detail}");
                return Err(format!("Discord rejected the activity: {detail}"));
            }
            Ok(())
        }
        Err(error) => {
            applog!("[discord] activity confirm read failed: {error}");
            Err(error.to_string())
        }
    }
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
pub fn discord_enable(state: State<DiscordRpcState>) -> Result<(), String> {
    applog!("[discord] enable requested");
    let mut rpc = state.client.lock();

    // A previous toggle already holds a client: its socket may be dead
    // (Discord was quit, or the handshake never completed), and the old code
    // returned Ok here without touching it — the toggle looked on while
    // nothing was connected. Reconnect so the state is real.
    if let Some(client) = rpc.as_mut() {
        applog!("[discord] client exists, reconnecting to refresh");
        if let Err(error) = client.reconnect() {
            applog!("[discord] reconnect failed: {error}");
            return Err(error.to_string());
        }
        applog!("[discord] reconnected");
    } else {
        applog!("[discord] opening IPC connection for client {CLIENT_ID}");
        let mut client = DiscordIpcClient::new(CLIENT_ID);
        if let Err(error) = client.connect() {
            applog!("[discord] connection failed: {error}");
            return Err(error.to_string());
        }
        applog!("[discord] IPC connection established");
        *rpc = Some(client);
    }

    set_activity_and_confirm(
        rpc.as_mut().expect("client just connected"),
        initial_activity(),
    )
}

#[tauri::command]
pub fn discord_update_activity(
    state: State<DiscordRpcState>,
    details: String,
    state_text: String,
) -> Result<(), String> {
    applog!("[discord] activity update requested: details={details:?}, state={state_text:?}");
    let mut rpc = state.client.lock();
    let Some(client) = rpc.as_mut() else {
        applog!("[discord] activity update skipped: RPC is not enabled");
        return Err("Discord RPC is not enabled".to_string());
    };

    set_activity_and_confirm(client, build_activity(details, state_text))
}
#[tauri::command]
pub fn discord_status(state: State<DiscordRpcState>) -> bool {
    let connected = state.client.lock().is_some();
    applog!("[discord] status queried: connected={connected}");
    connected
}

#[tauri::command]
pub fn discord_disable(state: State<DiscordRpcState>) -> Result<(), String> {
    applog!("[discord] disable requested");
    let mut rpc = state.client.lock();
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
