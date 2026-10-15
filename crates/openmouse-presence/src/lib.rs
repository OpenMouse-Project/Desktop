//! Discord Rich Presence for OpenMouse Desktop, shared by the Tauri app
//! (`src-tauri/src/discord_rpc.rs` wraps this as commands) and the GPUI app.
//!
//! Behaviour follows upstream's `discord_rpc.rs` (80588f6): the display name
//! comes from the Discord application registration, so no custom `name` is sent
//! (Discord ACKs a mismatched one but never shows the status); the payload is
//! details, state, a start timestamp, the logo image and a website button; and
//! every send waits for Discord's reply so a rejection is an error, not a
//! silent no-show.

use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use parking_lot::Mutex;

/// The registered Discord application. It owns the `logo` art asset below.
const CLIENT_ID: &str = "1555824483389149245";
/// Where the status's button sends people.
const WEBSITE: &str = "https://openmouse.app";
/// Key of the large image in the application's Rich Presence art assets.
const LOGO_ASSET: &str = "logo";

/// Diagnostics hook; the host points it at its own log. Unset, lines go to stderr.
pub static LOG_SINK: std::sync::OnceLock<fn(String)> = std::sync::OnceLock::new();

fn log(line: String) {
    match LOG_SINK.get() {
        Some(sink) => sink(line),
        None => eprintln!("{line}"),
    }
}

macro_rules! applog {
    ($($arg:tt)*) => {
        log(format!($($arg)*))
    };
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Details + state + the session's start time, the logo and the website button.
/// `started` is fixed per connection: restarting it on every update would reset
/// the "elapsed" counter each time the status changes.
fn build_activity(details: String, state_text: String, started: i64) -> activity::Activity<'static> {
    // Discord rejects blank strings, so an empty line is left out entirely.
    let mut activity = activity::Activity::new().details(details);
    if !state_text.is_empty() {
        activity = activity.state(state_text);
    }
    activity
        .timestamps(activity::Timestamps::new().start(started))
        .assets(activity::Assets::new().large_image(LOGO_ASSET).large_text("OpenMouse"))
        .buttons(vec![activity::Button::new("OpenMouse Website", WEBSITE)])
}

fn initial_activity(started: i64) -> activity::Activity<'static> {
    // The title above these lines is already the app's name, so they don't repeat it.
    if cfg!(debug_assertions) {
        build_activity(
            concat!("Dev Mode: Build ", env!("CARGO_PKG_VERSION")).to_string(),
            "Managing mouse settings".to_string(),
            started,
        )
    } else {
        build_activity("Managing mouse settings".to_string(), String::new(), started)
    }
}

/// Sends the activity and waits for Discord's reply: it answers every
/// SET_ACTIVITY, and `evt == "ERROR"` means it refused the payload and the
/// status will never display.
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
                let detail = response.get("data").map(|d| d.to_string()).unwrap_or_else(|| "unknown error".to_string());
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

struct Connection {
    client: DiscordIpcClient,
    /// When this presence session began (shown as elapsed time).
    started: i64,
}

#[derive(Default)]
pub struct Presence {
    connection: Mutex<Option<Connection>>,
}

impl Presence {
    /// Connects to the running Discord client and shows the initial activity.
    /// Blocking; fails if Discord is not running. If a client already exists its
    /// socket may be dead (Discord was quit), so it is reconnected rather than
    /// trusted — otherwise the switch would look on while nothing is connected.
    pub fn enable(&self) -> Result<(), String> {
        applog!("[discord] enable requested");
        let mut guard = self.connection.lock();

        if let Some(connection) = guard.as_mut() {
            applog!("[discord] client exists, reconnecting to refresh");
            if let Err(error) = connection.client.reconnect() {
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
            *guard = Some(Connection { client, started: now_millis() });
        }

        let connection = guard.as_mut().expect("connection just established");
        let started = connection.started;
        set_activity_and_confirm(&mut connection.client, initial_activity(started))
    }

    pub fn update_activity(&self, details: String, state_text: String) -> Result<(), String> {
        applog!("[discord] activity update requested: details={details:?}, state={state_text:?}");
        let mut guard = self.connection.lock();
        let Some(connection) = guard.as_mut() else {
            applog!("[discord] activity update skipped: RPC is not enabled");
            return Err("Discord RPC is not enabled".to_string());
        };
        let started = connection.started;
        set_activity_and_confirm(&mut connection.client, build_activity(details, state_text, started))
    }

    /// Whether a client is held (not whether its socket is still alive).
    pub fn is_connected(&self) -> bool {
        self.connection.lock().is_some()
    }

    pub fn disable(&self) -> Result<(), String> {
        applog!("[discord] disable requested");
        let mut guard = self.connection.lock();
        if let Some(mut connection) = guard.take() {
            if let Err(error) = connection.client.close() {
                applog!("[discord] disconnect failed: {error}");
                return Err(error.to_string());
            }
            applog!("[discord] disconnected");
        } else {
            applog!("[discord] already disconnected");
        }
        Ok(())
    }
}
