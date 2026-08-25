//! Native HID access for Full Desktop Mode, exposed to the frontend as Tauri
//! commands + an `"hid-input-report"` event.
//!
//! This exists so the frontend can drive `@openmouse/protocol`'s driver
//! classes — written against the browser WebHID API — from inside Tauri's
//! webview, which does not implement WebHID. The frontend's
//! `TauriHidDevice` (see `src/native-hid/tauri-hid-device.ts`) wraps these
//! commands into the same `HIDDevice`-shaped interface the driver classes
//! expect, mirroring OpenMouse-Bridge's `native-hid/src/hid-device-adapter.mjs`
//! (which does the same thing for a Node host instead of a webview).
//!
//! One WebHID `HIDDevice` == one USB/BT *interface*, which can carry several
//! top-level HID collections (e.g. a short and a long report variant on the
//! same vendor interface). hidapi enumerates one entry *per collection*, not
//! per interface, so every collection sharing a (vendor id, product id) pair
//! is grouped and opened together as one logical device.
//!
//! This used to group by (vendor id, product id, interface number) instead —
//! mirroring the Node adapter's `candidateDevices()` — but `interface_number`
//! turned out to not be stable across separate `device_list()` calls on
//! macOS for several real devices (a Logitech receiver, a Wooting keyboard),
//! producing a different number per enumeration for what was the same
//! physical collection. That surfaced as duplicate rows in the device list
//! (the same physical device appearing under several different keys) and, on
//! connect, an "exclusive access, device already open" hidapi error (list
//! time and open time disagreeing on which path a given key meant). Vendor +
//! product id alone is coarser — two genuinely different physical devices
//! that happen to share both ids would incorrectly merge into one entry —
//! but that's a rare edge case, and it's stable, which interface_number is
//! not on this platform.
//!
//! ## The actual cause of the input-freeze bug (found via the hidapi crate's
//! own doc comment on `HidApi::set_open_exclusive`, after several wrong
//! guesses — see git history on this file for the discarded theories):
//! **on macOS, hidapi opens every device in *exclusive* mode by default.**
//! That seizes the device from the OS's own HID client, which is exactly
//! why any real mouse/keyboard went unresponsive the moment this code
//! opened it, regardless of which collection, how long it stayed open, or
//! whether the `HidApi` instance was shared or freshly created each call —
//! none of those things actually mattered; every one of them still opened
//! in exclusive mode. `with_hid_api()` below calls
//! `api.set_open_exclusive(false)` once, immediately after creating the
//! shared instance, so every subsequent `open_path()` opens non-exclusively
//! and coexists with the OS's own handling of the device.
//!
//! The usage-page exclusion just below (never touching a mouse's pointer
//! collection or a keyboard's keystroke collection) stays in place as
//! defense in depth even though it wasn't the actual fix — there is still
//! no reason to open those collections, and Razer's driver classes are the
//! only ones that need to (see `is_live_input_collection`).
//!
//! **Known gap, confirmed on real hardware, not yet solved:** at least one
//! device (Endgame Gear's OP1-8K) answers `GetFeatureReport` with an I/O
//! timeout when opened non-exclusively — its control-transfer protocol
//! appears to require exclusive HID access to work at all. Exclusive access
//! was tried again with the shortest possible open/one-read/close window
//! and still froze the device's own input immediately, so duration is not
//! a mitigating factor. Non-exclusive stays the default because freezing a
//! user's real mouse/keyboard is a worse failure than a device's config
//! protocol not answering — a device that needs exclusive access to
//! function is presently unreachable through this bridge, same category of
//! gap as Razer (see `brands.ts`), not a bug to "fix" by bringing exclusive
//! mode back.
//!
//! A single `HidApi` instance is kept alive for the app's whole lifetime
//! (`HidApiHandle` below) and refreshed rather than recreated on every
//! call — simpler and cheaper than a fresh instance per call, and (now that
//! exclusive mode is off) has no bearing on the freeze either way.
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use hidapi::HidApi;
use serde::Serialize;
use tauri::Emitter;

/// How long each blocking read waits before checking the stop flag again.
/// Short enough that `hid_close` feels immediate, long enough not to spin.
const READ_POLL_TIMEOUT_MS: i32 = 200;

#[derive(Clone, Serialize)]
pub struct HidInterface {
    pub key: String,
    #[serde(rename = "vendorId")]
    pub vendor_id: u16,
    #[serde(rename = "productId")]
    pub product_id: u16,
    #[serde(rename = "productString")]
    pub product_string: String,
    #[serde(rename = "manufacturerString")]
    pub manufacturer_string: String,
}

#[derive(Clone, Serialize)]
struct HidInputReportPayload {
    key: String,
    #[serde(rename = "reportId")]
    report_id: u8,
    data: Vec<u8>,
}

struct OpenSplit {
    device: Mutex<hidapi::HidDevice>,
    stop: Arc<AtomicBool>,
}

struct OpenGroup {
    splits: Vec<Arc<OpenSplit>>,
    readers: Vec<JoinHandle<()>>,
    /// Which split answered a given report id last time. WebHID never has to
    /// guess this — the browser already knows which collection declares a
    /// report id from `device.collections` and routes straight to it.
    /// hidapi's flat device list gives us no such map, so `try_each` used to
    /// re-probe from split 0 on every single call — for a device where the
    /// answering split isn't first, that's a guaranteed-fail write before
    /// the one that actually works, paid again on every one of the dozens of
    /// requests a full `readStatus()` walk makes. Once a split answers for a
    /// report id, remember it and try that one first next time.
    routes: Mutex<HashMap<u8, usize>>,
}

/// One `HidApi` for the app's lifetime — see the module docs above for why
/// this matters, not just for efficiency.
pub struct HidApiHandle(Mutex<Option<HidApi>>);

impl Default for HidApiHandle {
    fn default() -> Self {
        HidApiHandle(Mutex::new(None))
    }
}

#[derive(Default)]
pub struct HidRegistry(Mutex<HashMap<(u16, u16), OpenGroup>>);

fn interface_key(vendor_id: u16, product_id: u16) -> String {
    format!("{vendor_id:04x}:{product_id:04x}")
}

/// Generic Desktop (page 0x01) Mouse (usage 0x02) or Keyboard (usage 0x06) —
/// the collection an OS actively reads for real cursor motion or keystrokes.
/// See the module docs above for why this is never opened, unconditionally.
fn is_live_input_collection(usage_page: u16, usage: u16) -> bool {
    usage_page == 0x01 && (usage == 0x02 || usage == 0x06)
}

/// Runs `f` against the shared `HidApi`, initializing it on first use and
/// refreshing its device list (cheap — no manager teardown/recreation) on
/// every later call. Non-exclusive open is set once, right after creation —
/// see the module docs above for why this is the actual freeze fix.
fn with_hid_api<T>(
    handle: &HidApiHandle,
    f: impl FnOnce(&mut HidApi) -> Result<T, String>,
) -> Result<T, String> {
    let mut guard = handle.0.lock().unwrap();
    match guard.as_mut() {
        Some(api) => {
            api.refresh_devices().map_err(|error| error.to_string())?;
        }
        None => {
            let api = HidApi::new().map_err(|error| error.to_string())?;
            // CONFIRMED on real hardware: exclusive open freezes the
            // device's own input (mouse cursor/keyboard keys) the moment
            // it's opened — reproduced even with the shortest possible
            // open/one-read/close window, so duration was never the
            // mitigating factor. Non-exclusive avoids that entirely. The
            // real cost: at least one device's (Endgame Gear OP1-8K)
            // GetFeatureReport-based protocol needs exclusive access to
            // answer at all and fails outright non-exclusively — a genuine
            // conflict between "don't freeze the user's hardware" and
            // "every protocol variant works." Freezing real input is the
            // worse failure mode, so non-exclusive wins here. A device
            // whose protocol needs exclusive access is a documented gap
            // (see brands.ts), not something to solve by reintroducing the
            // freeze.
            api.set_open_exclusive(false);
            *guard = Some(api);
        }
    }
    f(guard.as_mut().expect("just initialized above"))
}

// hid_list_interfaces and hid_open stay synchronous (blocking the main
// thread) deliberately, unlike every other command below: they're the two
// that touch `HidApi` itself — `HidApi::new()`/`refresh_devices()`/
// `open_path()`, the calls that actually open a device handle through
// macOS's IOHIDManager, as opposed to reading/writing an already-open one.
// Making them `async fn` (moving that off the main thread, onto Tokio's
// runtime) reproduced a real hang requiring a force-quit — every prior
// successful connect this session happened with these on the main thread,
// and there is no equivalent evidence they're safe off it the way the
// reader threads' plain read()/write() on an open handle demonstrably are
// (those have run on a spawned thread since the very start of this file's
// history without incident). Left blocking here; the high-frequency calls
// during a `readStatus()` walk — the ones that actually caused the original
// UI freeze — are the ones below that stayed async.
#[tauri::command]
pub fn hid_list_interfaces(
    api_handle: tauri::State<HidApiHandle>,
    vendor_ids: Vec<u16>,
) -> Result<Vec<HidInterface>, String> {
    with_hid_api(&api_handle, |api| {
        let mut groups: HashMap<(u16, u16), Vec<&hidapi::DeviceInfo>> = HashMap::new();
        for info in api.device_list() {
            if !vendor_ids.contains(&info.vendor_id()) {
                continue;
            }
            if is_live_input_collection(info.usage_page(), info.usage()) {
                continue;
            }
            groups
                .entry((info.vendor_id(), info.product_id()))
                .or_default()
                .push(info);
        }

        // A device whose only matching collection(s) were the live input
        // one just excluded above (e.g. Razer) legitimately has nothing
        // left here — it simply won't appear in the list, which is correct:
        // there is nothing safe to open for it.
        let mut result: Vec<HidInterface> = groups
            .into_iter()
            .map(|((vendor_id, product_id), infos)| {
                let product_string = infos
                    .iter()
                    .find_map(|info| info.product_string())
                    .unwrap_or("")
                    .to_string();
                let manufacturer_string = infos
                    .iter()
                    .find_map(|info| info.manufacturer_string())
                    .unwrap_or("")
                    .to_string();
                HidInterface {
                    key: interface_key(vendor_id, product_id),
                    vendor_id,
                    product_id,
                    product_string,
                    manufacturer_string,
                }
            })
            .collect();
        result.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(result)
    })
}

// See the comment on hid_list_interfaces above — kept synchronous for the
// same reason.
#[tauri::command]
pub fn hid_open(
    app: tauri::AppHandle,
    api_handle: tauri::State<HidApiHandle>,
    registry: tauri::State<HidRegistry>,
    vendor_id: u16,
    product_id: u16,
) -> Result<(), String> {
    let group_key = (vendor_id, product_id);
    applog!("[hid] hid_open start {vendor_id:04x}:{product_id:04x}");
    if registry.0.lock().unwrap().contains_key(&group_key) {
        applog!("[hid] hid_open: already open, returning");
        return Ok(());
    }

    let devices = with_hid_api(&api_handle, |api| {
        let paths: Vec<_> = api
            .device_list()
            .filter(|info| {
                info.vendor_id() == vendor_id
                    && info.product_id() == product_id
                    && !is_live_input_collection(info.usage_page(), info.usage())
            })
            .map(|info| info.path().to_owned())
            .collect();
        if paths.is_empty() {
            return Err("No matching HID interface is currently connected.".into());
        }
        // Best-effort per path: a device with several top-level collections
        // can have some open fine and others rejected for reasons that have
        // nothing to do with the collection this brand's driver actually
        // needs (a boot mouse/keyboard collection gated by macOS's Input
        // Monitoring permission, for instance, sitting alongside the vendor
        // collection a driver here actually talks to). Failing the whole
        // group over one inaccessible split would block drivers that never
        // needed that split in the first place. Only error out if literally
        // none of them opened.
        let mut opened = Vec::new();
        let mut last_error = None;
        for path in paths {
            match api.open_path(&path) {
                Ok(device) => opened.push(device),
                Err(error) => last_error = Some(error.to_string()),
            }
        }
        if opened.is_empty() {
            return Err(last_error.unwrap_or_else(|| "No HID interface could be opened.".into()));
        }
        applog!("[hid] hid_open: opened {} split(s)", opened.len());
        Ok(opened)
    })?;

    let key = interface_key(vendor_id, product_id);
    // Shared across every split's reader thread for this group — see
    // `spawn_reader`'s own docs for why this exists.
    let last_reply = Arc::new(Mutex::new(None));
    let mut splits = Vec::new();
    let mut readers = Vec::new();
    for device in devices {
        let stop = Arc::new(AtomicBool::new(false));
        let split = Arc::new(OpenSplit {
            device: Mutex::new(device),
            stop: stop.clone(),
        });
        readers.push(spawn_reader(
            app.clone(),
            key.clone(),
            split.clone(),
            last_reply.clone(),
        ));
        splits.push(split);
    }

    registry.0.lock().unwrap().insert(
        group_key,
        OpenGroup {
            splits,
            readers,
            routes: Mutex::new(HashMap::new()),
        },
    );
    applog!("[hid] hid_open: done, registered group");
    Ok(())
}

/// Standard HID++ short/long report ids (0x10/0x11) — see `brands.ts`'s own
/// list: nearly every driver in this app that answers a request/response
/// exchange through an input report (not a plain feature report) uses these
/// two, Logitech's HID++ included.
const REQUEST_REPLY_REPORT_IDS: [u8; 2] = [0x10, 0x11];

/// How recent a byte-identical prior reply has to be to count as the SAME
/// physical report delivered twice (macOS handing it to every open split's
/// handle) rather than a coincidentally-identical reply to a later, distinct
/// request. Duplicate deliveries land within a couple of poll cycles of each
/// other; a real request/reply round trip (write + device turnaround + read)
/// is comfortably longer than this even on a fast wired connection.
const DUPLICATE_DELIVERY_WINDOW: Duration = Duration::from_millis(50);

/// HID++ 2.0 error codes (byte 4 of a 0xFF error reply) — mirrors
/// `HIDPP20_ERRORS` in mouse-protocol's `logitech/index.ts`, purely so a
/// rejection is legible in these logs without needing the webview's own
/// devtools console open.
fn hidpp20_error_name(code: u8) -> Option<&'static str> {
    Some(match code {
        0x01 => "unknown request",
        0x02 => "invalid argument",
        0x03 => "value out of range",
        0x04 => "hardware error",
        0x05 => "Logitech internal error",
        0x06 => "invalid feature index",
        0x07 => "invalid function",
        0x08 => "device busy",
        0x09 => "unsupported",
        _ => return None,
    })
}

/// HID++ 1.0 error codes (byte 4 of a 0x8F error reply) — mirrors
/// `HIDPP10_ERRORS` in the same file.
fn hidpp10_error_name(code: u8) -> Option<&'static str> {
    Some(match code {
        0x01 => "invalid command",
        0x02 => "invalid address",
        0x03 => "invalid value",
        0x04 => "connection request failed",
        0x05 => "too many devices",
        0x06 => "already exists",
        0x07 => "device busy",
        0x08 => "unknown device",
        0x09 => "resource error",
        0x0a => "request unavailable",
        0x0b => "unsupported parameter value",
        0x0c => "wrong PIN code",
        _ => return None,
    })
}

/// Decodes an outgoing HID++ short/long request's own header — deviceIndex,
/// featureIndex, function id (the software id in the low nibble stripped
/// off), and parameter bytes — so a request can be matched by eye against
/// the reply (or rejection) it produced a few lines later in the log.
fn describe_hidpp_request(data: &[u8]) -> String {
    if data.len() < 3 {
        return format!("raw={data:02x?}");
    }
    let device_index = data[0];
    let feature_index = data[1];
    let function_id = data[2] >> 4;
    let params = &data[3..];
    format!(
        "hidpp deviceIndex=0x{device_index:02x} featureIndex=0x{feature_index:02x} \
         functionId=0x{function_id:02x} params={params:02x?}"
    )
}

/// Decodes an incoming report as a HID++ error notification, if it is one —
/// `[deviceIndex, 0xFF|0x8F, featureIndex, function+softwareId, errorCode]`,
/// mirroring `hidppErrorForRequest` in mouse-protocol's own driver. Returns
/// `None` for anything else (a normal reply, or a raw input/status report).
fn describe_hidpp_error(report_id: u8, data: &[u8]) -> Option<String> {
    if report_id != 0x10 && report_id != 0x11 {
        return None;
    }
    // `data` is the payload after the leading report id byte, same
    // alignment as mouse-protocol's own `report` array (WebHID strips the
    // report id out separately too, into `event.reportId`): data[0] is
    // deviceIndex, data[1] is the 0xFF/0x8F error marker, data[2] is the
    // echoed featureIndex from the rejected request, data[3] its echoed
    // function+softwareId byte, data[4] the error code.
    let marker = *data.get(1)?;
    let feature_index = *data.get(2)?;
    let function_byte = *data.get(3)?;
    let error_code = *data.get(4).unwrap_or(&0);
    let (kind, name) = match marker {
        0xff => ("HID++ 2.0", hidpp20_error_name(error_code)),
        0x8f => ("HID++ 1.0", hidpp10_error_name(error_code)),
        _ => return None,
    };
    let reason = name.unwrap_or("unknown error");
    Some(format!(
        "{kind} ERROR: featureIndex=0x{feature_index:02x} functionByte=0x{function_byte:02x} \
         code=0x{error_code:02x} ({reason})"
    ))
}

fn spawn_reader(
    app: tauri::AppHandle,
    key: String,
    split: Arc<OpenSplit>,
    last_reply: Arc<Mutex<Option<(u8, Vec<u8>, Instant)>>>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut buffer = [0u8; 64];
        while !split.stop.load(Ordering::Relaxed) {
            // try_lock, not lock: a query/response exchange (hid_send_report
            // / hid_get_feature_report, called from the JS driver's own
            // readStatus()) needs this same device's lock and must never
            // wait behind this loop's read — reads are continuous and
            // best-effort (missing one poll cycle is fine), but a stuck
            // query is what "Connecting…" hanging looked like. If a writer
            // currently holds the lock, skip this cycle instead of blocking
            // for it.
            //
            // CONFIRMED on real hardware: this alone isn't enough. When
            // read_timeout() finds no data waiting, it blocks for the full
            // READ_POLL_TIMEOUT_MS while STILL HOLDING the try_lock guard,
            // and this loop went straight back into try_lock() the instant
            // that guard dropped, with nothing in between. A writer parked
            // on the blocking `.lock()` in try_each has no fairness
            // guarantee against that: macOS's pthread_mutex (what
            // std::sync::Mutex wraps here) doesn't queue a parked waiter
            // ahead of a thread that keeps re-acquiring via try_lock, so
            // this loop could win every single re-lock race indefinitely —
            // a real livelock, not a slow poll. Seen live: resolveDeviceIndex
            // walking receiver pairing slots got every reply promptly right
            // up until the write for deviceIndex=0xff, whose try_each sat on
            // "waiting for lock…" and never progressed — this loop had just
            // finished draining a backlog and immediately re-entered another
            // 200ms held read_timeout, over and over, never leaving a gap
            // for the scheduler to hand the lock to the parked writer. The
            // short sleep below (outside the lock, every iteration — not
            // only after a failed try_lock) is what actually creates that
            // gap.
            let read = match split.device.try_lock() {
                Ok(device) => device.read_timeout(&mut buffer, READ_POLL_TIMEOUT_MS),
                Err(_) => {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
            };
            thread::sleep(Duration::from_millis(5));
            match read {
                Ok(0) => continue,
                Ok(length) => {
                    // Some backends prefix numbered reports with the report
                    // id, some don't — assume they do (report id in
                    // buffer[0]), matching the equivalent ambiguity the
                    // Node adapter and the Rust Pulsar driver both handle
                    // the same way. Drivers here match responses by their
                    // own payload content, not strictly by report id.
                    let report_id = buffer[0];
                    let data = buffer[1..length].to_vec();
                    // CONFIRMED on real hardware: macOS delivers the same
                    // physical report to every open split's handle, not just
                    // the one collection that actually declares it — every
                    // request/reply exchange gets emitted here once per
                    // split, not once. A driver's own reply matching keys on
                    // (deviceIndex, featureIndex, functionId), not report
                    // content, and several unrelated requests can share that
                    // key (every HID++ getFeature() call does) — so a
                    // duplicate arriving late enough to land after the next
                    // request went out gets consumed as that request's
                    // answer instead, silently corrupting whichever field
                    // was being read at the time. WebHID never has this
                    // problem: one collection is one `HIDDevice`, so a
                    // report is only ever delivered to the listener for the
                    // collection that actually declared it. Collapsing an
                    // exact repeat of the immediately preceding
                    // request/reply-shaped report is the equivalent fix
                    // here. Left unrestricted to every report id — a raw
                    // input stream (movement, clicks) can legitimately
                    // repeat the same idle payload many times in a row, and
                    // collapsing those would silently drop real reports.
                    //
                    // CONFIRMED on real hardware this needs a time window,
                    // not a bare content match: an IRoot.getFeature() "not
                    // found" reply carries no trace of which feature id was
                    // asked (deviceIndex, echoed featureIndex=0, echoed
                    // functionId, all-zero payload) — so two DIFFERENT,
                    // genuinely sequential getFeature() calls that both come
                    // back "not found" are byte-for-byte identical. A bare
                    // content-match dedup silently ate the second one
                    // (getFeature(0x1001) after getFeature(0x1000), both
                    // absent on this mouse) and readStatus() hung waiting
                    // for a reply that was dropped, not missing. The actual
                    // macOS duplicate-delivery bug this exists for lands
                    // within a millisecond or two — the same physical report
                    // handed to every open split's reader thread nearly
                    // simultaneously — while a new request's real reply is
                    // always at least a full write+round-trip later. Only
                    // treat a content match as a duplicate within a window
                    // comfortably under that round trip.
                    let mut last = last_reply.lock().unwrap();
                    let is_duplicate = REQUEST_REPLY_REPORT_IDS.contains(&report_id)
                        && matches!(
                            last.as_ref(),
                            Some((last_id, last_data, seen_at))
                                if *last_id == report_id
                                    && *last_data == data
                                    && seen_at.elapsed() < DUPLICATE_DELIVERY_WINDOW
                        );
                    if is_duplicate {
                        continue;
                    }
                    if REQUEST_REPLY_REPORT_IDS.contains(&report_id) {
                        *last = Some((report_id, data.clone(), Instant::now()));
                    }
                    drop(last);
                    // Only log HID++ protocol traffic (report ids 0x10/0x11),
                    // and only once it's cleared the dedup check above — not
                    // every raw report a split hands us. The continuous
                    // stream of movement/click/battery-poll reports on other
                    // report ids is real and gets emitted below same as
                    // always, it's just not useful to print: at full poll
                    // rate it drowns the handful of request/reply lines that
                    // actually matter for debugging a connect in thousands of
                    // irrelevant ones.
                    if REQUEST_REPLY_REPORT_IDS.contains(&report_id) {
                        applog!(
                            "[hid] reader {key}: got {length} bytes, reportId=0x{report_id:02x} data={data:02x?}"
                        );
                        if let Some(decoded) = describe_hidpp_error(report_id, &data) {
                            applog!("[hid] reader {key}: *** {decoded} ***");
                        }
                    }
                    let _ = app.emit(
                        "hid-input-report",
                        HidInputReportPayload {
                            key: key.clone(),
                            report_id,
                            data,
                        },
                    );
                }
                Err(error) => {
                    // A transient read error (e.g. device unplugged) — the
                    // stop flag (set by hid_close) is what ends this loop
                    // deliberately; anything else just backs off briefly.
                    applog!("[hid] reader {key}: read_timeout error: {error}");
                    thread::sleep(Duration::from_millis(50));
                }
            }
        }
    })
}

#[tauri::command]
pub async fn hid_close(
    registry: tauri::State<'_, HidRegistry>,
    vendor_id: u16,
    product_id: u16,
) -> Result<(), String> {
    let group = registry.0.lock().unwrap().remove(&(vendor_id, product_id));
    if let Some(group) = group {
        for split in &group.splits {
            split.stop.store(true, Ordering::Relaxed);
        }
        // Joining here (instead of a fire-and-forget stop signal) means the
        // underlying HID handles are guaranteed closed — and the OS has
        // released the device — before this command returns, not up to
        // READ_POLL_TIMEOUT_MS later. Bounded by that same timeout per
        // reader, so this never blocks for long.
        for reader in group.readers {
            let _ = reader.join();
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn hid_send_report(
    registry: tauri::State<'_, HidRegistry>,
    vendor_id: u16,
    product_id: u16,
    report_id: u8,
    data: Vec<u8>,
) -> Result<(), String> {
    applog!(
        "[hid] hid_send_report {vendor_id:04x}:{product_id:04x} reportId=0x{report_id:02x} {}",
        describe_hidpp_request(&data),
    );
    let result = with_open_group(&registry, vendor_id, product_id, |splits, routes| {
        let mut frame = Vec::with_capacity(data.len() + 1);
        frame.push(report_id);
        frame.extend_from_slice(&data);
        try_each(splits, routes, report_id, |device| {
            device.write(&frame).map(|_| ())
        })
    });
    applog!("[hid] hid_send_report done: {result:?}");
    result
}

#[tauri::command]
pub async fn hid_send_feature_report(
    registry: tauri::State<'_, HidRegistry>,
    vendor_id: u16,
    product_id: u16,
    report_id: u8,
    data: Vec<u8>,
) -> Result<(), String> {
    applog!(
        "[hid] hid_send_feature_report {vendor_id:04x}:{product_id:04x} reportId=0x{report_id:02x} {}",
        describe_hidpp_request(&data),
    );
    let result = with_open_group(&registry, vendor_id, product_id, |splits, routes| {
        let mut frame = Vec::with_capacity(data.len() + 1);
        frame.push(report_id);
        frame.extend_from_slice(&data);
        try_each(splits, routes, report_id, |device| {
            device.send_feature_report(&frame)
        })
    });
    applog!("[hid] hid_send_feature_report done: {result:?}");
    result
}

#[tauri::command]
pub async fn hid_get_feature_report(
    registry: tauri::State<'_, HidRegistry>,
    vendor_id: u16,
    product_id: u16,
    report_id: u8,
    length: usize,
) -> Result<Vec<u8>, String> {
    applog!("[hid] hid_get_feature_report {vendor_id:04x}:{product_id:04x} reportId=0x{report_id:02x} length={length}");
    let result = with_open_group(&registry, vendor_id, product_id, |splits, routes| {
        let mut result: Option<Vec<u8>> = None;
        let outcome = try_each(splits, routes, report_id, |device| {
            let mut buffer = vec![0u8; length + 1];
            buffer[0] = report_id;
            let read = device.get_feature_report(&mut buffer)?;
            result = Some(buffer[..read].to_vec());
            Ok(())
        });
        outcome.and(result.ok_or_else(|| "no split returned data".to_string()))
    });
    applog!("[hid] hid_get_feature_report done: {result:?}");
    result
}

/// Runs `operation` against the open splits for a group (plus its
/// report-id→split route cache — see `OpenGroup::routes`), trying every
/// split in turn on a cache miss — the report id in question may only be
/// declared on one of them, same reasoning as the Node adapter's
/// `sendReport`/`receiveFeatureReport`.
fn with_open_group<T>(
    registry: &tauri::State<'_, HidRegistry>,
    vendor_id: u16,
    product_id: u16,
    operation: impl FnOnce(&[Arc<OpenSplit>], &Mutex<HashMap<u8, usize>>) -> Result<T, String>,
) -> Result<T, String> {
    let map = registry.0.lock().unwrap();
    let group = map
        .get(&(vendor_id, product_id))
        .ok_or_else(|| "This HID interface is not open.".to_string())?;
    operation(&group.splits, &group.routes)
}

fn try_each(
    splits: &[Arc<OpenSplit>],
    routes: &Mutex<HashMap<u8, usize>>,
    report_id: u8,
    mut operation: impl FnMut(&hidapi::HidDevice) -> hidapi::HidResult<()>,
) -> Result<(), String> {
    // Try whichever split answered this exact report id last time first —
    // see `OpenGroup::routes`. Falls through to the rest in order on a miss
    // (first call for this report id, or the routed split stopped working),
    // so this is a pure optimization, never a correctness change.
    let hinted = routes.lock().unwrap().get(&report_id).copied();
    let order: Vec<usize> = match hinted {
        Some(hint) if hint < splits.len() => std::iter::once(hint)
            .chain((0..splits.len()).filter(|&i| i != hint))
            .collect(),
        _ => (0..splits.len()).collect(),
    };

    let mut last_error = None;
    for index in order {
        let split = &splits[index];
        applog!(
            "[hid] try_each: split {index}/{} waiting for lock…",
            splits.len()
        );
        let device = split.device.lock().unwrap();
        applog!("[hid] try_each: split {index} got lock, calling operation…");
        let outcome = operation(&device);
        applog!("[hid] try_each: split {index} operation returned: {outcome:?}");
        match outcome {
            Ok(()) => {
                routes.lock().unwrap().insert(report_id, index);
                return Ok(());
            }
            Err(error) => last_error = Some(error.to_string()),
        }
    }
    Err(last_error.unwrap_or_else(|| "No HID interface accepted the request.".to_string()))
}
