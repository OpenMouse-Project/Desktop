//! Local HTTP server for the "OBS overlay" — a browser-source page a
//! streamer adds in OBS that shows their currently-connected mouse's name,
//! DPI and polling rate, live.
//!
//! This is deliberately *not* a native OBS plugin: those are per-OS C++
//! binaries built against OBS's own SDK version, need separate signing and
//! an install step inside OBS itself, and would roughly triple this
//! feature's maintenance surface for something OBS already has a door for.
//! OBS's Browser Source can point at any URL, so instead this module runs a
//! tiny HTTP server on `127.0.0.1` and serves the overlay as a normal page,
//! same idea as a stream-deck-style widget. `tiny_http` (sync, one thread
//! per connection) rather than pulling in an async runtime — this is a
//! handful of long-lived local connections at most.
//!
//! Data flow mirrors tray.rs: the frontend owns the live `MouseStatus` (the
//! protocol drivers run in the webview) and pushes it down here through the
//! `stream_overlay_set_device_status` command whenever it changes. This
//! module just holds the latest snapshot and fans it out over
//! Server-Sent Events to whatever's currently loaded in OBS.

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tiny_http::{Header, Method, Response, Server, StatusCode};

/// What the frontend knows about the connected device that's worth showing
/// on stream. Kept as its own type rather than shared with
/// `tray::TrayDeviceStatus` — that one's about charge (what matters
/// glancing at a tray menu), this one's about the settings a viewer watching
/// a stream cares about (DPI/polling rate), so the two are free to diverge.
/// No battery field here on purpose — deliberately dropped (not everyone's
/// mouse even has one, and it's not why anyone wants this overlay).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayDeviceStatus {
    pub name: String,
    pub dpi: Option<u32>,
    pub polling_rate_hz: Option<u32>,
}

struct Inner {
    /// Shared with the listener thread so a freshly-opened `/events`
    /// connection can paint the current device immediately instead of
    /// waiting for the next `stream_overlay_set_device_status` call.
    status: Arc<Mutex<Option<OverlayDeviceStatus>>>,
    /// One sender per currently-open `/events` connection; each SSE handler
    /// thread owns the matching receiver and forwards whatever arrives here
    /// straight to its client. Cleared lazily — a dead client is dropped
    /// the next time a send to it fails. Shared (not just owned by `Inner`)
    /// because the listener thread spawned in `stream_overlay_start` needs
    /// the exact same list to register new connections into as
    /// `stream_overlay_set_device_status` broadcasts through.
    clients: Arc<Mutex<Vec<Sender<String>>>>,
    port: Option<u16>,
    running: Arc<AtomicBool>,
    /// The listener thread, kept so `stream_overlay_stop` can wait for it to
    /// actually drop its `Server` — see that function.
    listener: Option<thread::JoinHandle<()>>,
}

impl Default for Inner {
    fn default() -> Self {
        Inner {
            status: Arc::new(Mutex::new(None)),
            clients: Arc::new(Mutex::new(Vec::new())),
            port: None,
            running: Arc::new(AtomicBool::new(false)),
            listener: None,
        }
    }
}

#[derive(Default)]
pub struct StreamOverlayState(Mutex<Inner>);

const PREFERRED_PORT: u16 = 39217;

fn status_json(status: &Option<OverlayDeviceStatus>) -> String {
    serde_json::to_string(status).unwrap_or_else(|_| "null".to_string())
}

/// Starts the server if it isn't already running and returns the URL to
/// paste into OBS's Browser Source. Idempotent — calling this again while
/// already running just returns the existing URL.
#[tauri::command]
pub fn stream_overlay_start(state: tauri::State<StreamOverlayState>) -> Result<String, String> {
    let mut guard = state.0.lock();
    // `running`, not `port`, is what says the server is actually up: the
    // listener thread clears it if it ever exits on its own (see `serve`), so
    // a dead server is restarted here instead of being reported as running
    // from a stale port.
    if guard.running.load(Ordering::SeqCst) {
        if let Some(port) = guard.port {
            return Ok(format!("http://127.0.0.1:{port}/"));
        }
    }

    // Prefer a fixed port so a previously-saved OBS Browser Source URL keeps
    // working across restarts; fall back to whatever the OS hands out if
    // something else already has it.
    let server = Server::http(("127.0.0.1", PREFERRED_PORT))
        .or_else(|_| Server::http(("127.0.0.1", 0)))
        .map_err(|e| format!("couldn't start overlay server: {e}"))?;
    let port = server.server_addr().to_ip().map(|a| a.port()).ok_or("overlay server has no port")?;

    let running = Arc::new(AtomicBool::new(true));
    guard.running = running.clone();
    guard.port = Some(port);
    let clients = guard.clients.clone();
    let status = guard.status.clone();

    guard.listener = Some(thread::spawn(move || serve(server, running, clients, status)));

    Ok(format!("http://127.0.0.1:{port}/"))
}

/// Stops the server and waits for the listener thread to finish, so the
/// socket is actually released before this returns. It used to just flip the
/// flag and return: the thread only notices on its next poll (up to
/// `POLL_INTERVAL` later) and the `Server` — and with it the fixed port —
/// lives until it does, so turning the overlay off and straight back on
/// failed to rebind `PREFERRED_PORT` and silently fell back to a random port,
/// changing the URL the user had already saved in OBS. That is the one thing
/// the fixed port exists to prevent.
#[tauri::command]
pub fn stream_overlay_stop(state: tauri::State<StreamOverlayState>) -> Result<(), String> {
    let mut guard = state.0.lock();
    guard.running.store(false, Ordering::SeqCst);
    // Dropping every sender is what ends each `/events` handler thread's
    // `recv_timeout` loop, so the connections actually close rather than
    // sitting open until the next write fails.
    guard.clients.lock().clear();
    if let Some(listener) = guard.listener.take() {
        let _ = listener.join();
    }
    guard.port = None;
    Ok(())
}

/// Mirrors the cached device status into the overlay, same trigger as
/// `tray::tray_set_device_status`. A no-op (cheap) when the server isn't
/// running — the frontend doesn't need to know whether it's on.
#[tauri::command]
pub fn stream_overlay_set_device_status(
    state: tauri::State<StreamOverlayState>,
    status: Option<OverlayDeviceStatus>,
) -> Result<(), String> {
    let guard = state.0.lock();
    *guard.status.lock() = status.clone();
    let payload = format!("data: {}\n\n", status_json(&status));
    guard.clients.lock().retain(|tx| tx.send(payload.clone()).is_ok());
    Ok(())
}

/// Returns the current overlay URL, or `None` if the server isn't running.
/// Lets Settings show the URL after a re-render without restarting the
/// server (`stream_overlay_start` is also safe to call again, but this
/// avoids the log noise of a redundant "already running" round trip).
#[tauri::command]
pub fn stream_overlay_status(state: tauri::State<StreamOverlayState>) -> Result<Option<String>, String> {
    let guard = state.0.lock();
    // The listener thread died on its own (see `serve`'s error arm) — report
    // the overlay as not running instead of handing back a port nothing is
    // listening on, so the Settings toggle and a re-`start` both see the
    // truth.
    if !guard.running.load(Ordering::SeqCst) {
        return Ok(None);
    }
    Ok(guard.port.map(|port| format!("http://127.0.0.1:{port}/")))
}

const POLL_INTERVAL: Duration = Duration::from_millis(500);
/// How often an idle SSE connection gets a comment line, so a proxy or OBS
/// itself doesn't decide the connection died and needs poking.
const HEARTBEAT: Duration = Duration::from_secs(15);

fn serve(
    server: Server,
    running: Arc<AtomicBool>,
    clients: Arc<Mutex<Vec<Sender<String>>>>,
    status: Arc<Mutex<Option<OverlayDeviceStatus>>>,
) {
    while running.load(Ordering::SeqCst) {
        let request = match server.recv_timeout(POLL_INTERVAL) {
            Ok(Some(request)) => request,
            Ok(None) => continue,
            // tiny_http funnels accept-side failures into this same call, and
            // there is no way back once its internal accept thread is gone —
            // so stop for good, but say so and clear the flag: leaving
            // `running` set had `stream_overlay_status` and the Settings
            // toggle report the overlay as live while nothing was being
            // served, with no log line anywhere to explain why OBS lost it
            // mid-session.
            Err(error) => {
                applog!("[overlay] listener stopped: {error}");
                running.store(false, Ordering::SeqCst);
                break;
            }
        };

        // `request.url()` is the raw request target, query string and all
        // (e.g. `/?name=1&dpi=1`) — the field toggles live there (read by
        // OVERLAY_HTML's own JS via `location.search`), so routing has to
        // match on the path alone or every preview/OBS URL with params
        // 404s here instead of ever reaching the JS that would use them.
        let path = request.url().split('?').next().unwrap_or("").to_string();
        match (request.method(), path.as_str()) {
            (Method::Get, "/" | "/index.html") => {
                let header = Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap();
                let _ = request.respond(Response::from_string(OVERLAY_HTML).with_header(header));
            }
            (Method::Get, "/events") => {
                let clients = clients.clone();
                let running = running.clone();
                let status = status.clone();
                thread::spawn(move || handle_sse(request, running, clients, status));
            }
            _ => {
                let _ = request.respond(Response::from_string("not found").with_status_code(StatusCode(404)));
            }
        }
    }
}

fn handle_sse(
    request: tiny_http::Request,
    running: Arc<AtomicBool>,
    clients: Arc<Mutex<Vec<Sender<String>>>>,
    status: Arc<Mutex<Option<OverlayDeviceStatus>>>,
) {
    let (tx, rx) = channel::<String>();
    clients.lock().push(tx);

    // `into_writer` hands back the raw connection with none of tiny_http's
    // usual header handling, since an SSE response needs to stay open and
    // keep writing after the "headers" are sent rather than close once a
    // single `Response` body is done — so the status line and headers are
    // written by hand here, once, before the event loop below.
    let mut writer = request.into_writer();
    let preamble = b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\n\r\n";
    if writer.write_all(preamble).and_then(|_| writer.flush()).is_err() {
        return;
    }

    // Paint whatever's already cached before waiting on the channel, so a
    // browser source that (re)loads mid-session doesn't sit blank until the
    // next status change.
    let initial = status.lock().clone();
    if writer.write_all(format!("data: {}\n\n", status_json(&initial)).as_bytes()).and_then(|_| writer.flush()).is_err() {
        return;
    }

    while running.load(Ordering::SeqCst) {
        match rx.recv_timeout(HEARTBEAT) {
            Ok(payload) => {
                if writer.write_all(payload.as_bytes()).and_then(|_| writer.flush()).is_err() {
                    break;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if writer.write_all(b": keep-alive\n\n").and_then(|_| writer.flush()).is_err() {
                    break;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

/// Single-file overlay page: fetches nothing but `/events` (SSE), no build
/// step, no external assets — OBS's Browser Source loads this directly.
/// Background is transparent so it composites straight onto the scene.
const OVERLAY_HTML: &str = r#"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<title>OpenMouse Overlay</title>
<style>
  html, body { margin: 0; padding: 0; background: transparent; }
  body {
    font-family: -apple-system, "Segoe UI", Roboto, sans-serif;
    color: #fff;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 16px;
    width: fit-content;
    background: rgba(20, 20, 24, 0.72);
    border-radius: 10px;
    box-shadow: 0 2px 10px rgba(0,0,0,0.35);
  }
  #dot { width: 10px; height: 10px; border-radius: 50%; background: #555; flex-shrink: 0; }
  #dot.connected { background: #3ddc84; }
  #name { font-size: 18px; font-weight: 600; white-space: nowrap; }
  #stats { display: flex; gap: 10px; font-size: 14px; opacity: 0.85; white-space: nowrap; }
  #stats span:empty { display: none; }
  #empty { font-size: 16px; opacity: 0.7; }
</style>
</head>
<body>
  <div id="dot"></div>
  <div id="name"></div>
  <div id="stats">
    <span id="dpi"></span>
    <span id="polling"></span>
  </div>
  <div id="empty">No mouse connected</div>
<script>
// Which fields to draw is chosen in Settings (SettingsPage.tsx's live
// preview) and baked into the Browser Source URL as query params, e.g.
// `?name=1&dpi=1&polling=0` — so the one HTML page here serves both the
// in-app preview iframe and the real OBS source, always in agreement,
// without needing a second round trip back into the app to ask.
const params = new URLSearchParams(location.search);
const show = (key) => params.get(key) !== '0';
function render(status) {
  const dot = document.getElementById('dot');
  const name = document.getElementById('name');
  const stats = document.getElementById('stats');
  const dpi = document.getElementById('dpi');
  const polling = document.getElementById('polling');
  const empty = document.getElementById('empty');
  if (!status) {
    dot.className = '';
    name.textContent = '';
    stats.style.display = 'none';
    dpi.textContent = '';
    polling.textContent = '';
    empty.style.display = '';
    return;
  }
  empty.style.display = 'none';
  stats.style.display = '';
  dot.className = 'connected';
  name.style.display = show('name') ? '' : 'none';
  name.textContent = status.name || 'Connected device';
  dpi.textContent = show('dpi') && typeof status.dpi === 'number' ? `${status.dpi} DPI` : '';
  polling.textContent = show('polling') && typeof status.pollingRateHz === 'number' ? `${status.pollingRateHz} Hz` : '';
}
render(null);
function connect() {
  const es = new EventSource('/events');
  es.onmessage = (e) => {
    try { render(JSON.parse(e.data)); } catch (err) { /* ignore malformed frame */ }
  };
  es.onerror = () => {
    es.close();
    setTimeout(connect, 1000);
  };
}
connect();
</script>
</body>
</html>"#;
