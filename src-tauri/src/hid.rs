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
//! A single `HidApi` instance is kept alive for the app's whole lifetime
//! (`HidApiHandle` below) and refreshed rather than recreated on every call.
//! Repeatedly constructing throwaway `HidApi::new()` instances — one per
//! command — was tried first and caused a real, reproducible regression:
//! scanning would leave a currently-plugged-in mouse's own cursor input
//! dead until the app quit, on hardware whose config protocol legitimately
//! shares a HID collection with the mouse's own input reports (e.g. Razer —
//! see `RazerHidClient.isSupported()`, which matches usage page 0x01/usage
//! 0x02, literally the standard mouse collection). Recreating the
//! underlying HID manager on every scan call repeatedly re-matched/attached
//! to that collection; one shared, refreshed instance does not.
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

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

/// Runs `f` against the shared `HidApi`, initializing it on first use and
/// refreshing its device list (cheap — no manager teardown/recreation) on
/// every later call.
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
            *guard = Some(HidApi::new().map_err(|error| error.to_string())?);
        }
    }
    f(guard.as_mut().expect("just initialized above"))
}

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
            groups.entry((info.vendor_id(), info.product_id())).or_default().push(info);
        }

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

#[tauri::command]
pub fn hid_open(
    app: tauri::AppHandle,
    api_handle: tauri::State<HidApiHandle>,
    registry: tauri::State<HidRegistry>,
    vendor_id: u16,
    product_id: u16,
) -> Result<(), String> {
    let group_key = (vendor_id, product_id);
    if registry.0.lock().unwrap().contains_key(&group_key) {
        return Ok(());
    }

    let devices = with_hid_api(&api_handle, |api| {
        let paths: Vec<_> = api
            .device_list()
            .filter(|info| info.vendor_id() == vendor_id && info.product_id() == product_id)
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
        Ok(opened)
    })?;

    let key = interface_key(vendor_id, product_id);
    let mut splits = Vec::new();
    let mut readers = Vec::new();
    for device in devices {
        let stop = Arc::new(AtomicBool::new(false));
        let split = Arc::new(OpenSplit {
            device: Mutex::new(device),
            stop: stop.clone(),
        });
        readers.push(spawn_reader(app.clone(), key.clone(), split.clone()));
        splits.push(split);
    }

    registry.0.lock().unwrap().insert(group_key, OpenGroup { splits, readers });
    Ok(())
}

fn spawn_reader(app: tauri::AppHandle, key: String, split: Arc<OpenSplit>) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut buffer = [0u8; 64];
        while !split.stop.load(Ordering::Relaxed) {
            let read = {
                let device = split.device.lock().unwrap();
                device.read_timeout(&mut buffer, READ_POLL_TIMEOUT_MS)
            };
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
                    let _ = app.emit(
                        "hid-input-report",
                        HidInputReportPayload { key: key.clone(), report_id, data },
                    );
                }
                Err(_) => {
                    // A transient read error (e.g. device unplugged) — the
                    // stop flag (set by hid_close) is what ends this loop
                    // deliberately; anything else just backs off briefly.
                    thread::sleep(Duration::from_millis(50));
                }
            }
        }
    })
}

#[tauri::command]
pub fn hid_close(
    registry: tauri::State<HidRegistry>,
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
pub fn hid_send_report(
    registry: tauri::State<HidRegistry>,
    vendor_id: u16,
    product_id: u16,
    report_id: u8,
    data: Vec<u8>,
) -> Result<(), String> {
    with_open_group(&registry, vendor_id, product_id, |splits| {
        let mut frame = Vec::with_capacity(data.len() + 1);
        frame.push(report_id);
        frame.extend_from_slice(&data);
        try_each(splits, |device| device.write(&frame).map(|_| ()))
    })
}

#[tauri::command]
pub fn hid_send_feature_report(
    registry: tauri::State<HidRegistry>,
    vendor_id: u16,
    product_id: u16,
    report_id: u8,
    data: Vec<u8>,
) -> Result<(), String> {
    with_open_group(&registry, vendor_id, product_id, |splits| {
        let mut frame = Vec::with_capacity(data.len() + 1);
        frame.push(report_id);
        frame.extend_from_slice(&data);
        try_each(splits, |device| device.send_feature_report(&frame))
    })
}

#[tauri::command]
pub fn hid_get_feature_report(
    registry: tauri::State<HidRegistry>,
    vendor_id: u16,
    product_id: u16,
    report_id: u8,
    length: usize,
) -> Result<Vec<u8>, String> {
    with_open_group(&registry, vendor_id, product_id, |splits| {
        let mut result: Option<Vec<u8>> = None;
        let outcome = try_each(splits, |device| {
            let mut buffer = vec![0u8; length + 1];
            buffer[0] = report_id;
            let read = device.get_feature_report(&mut buffer)?;
            result = Some(buffer[..read].to_vec());
            Ok(())
        });
        outcome.and(result.ok_or_else(|| "no split returned data".to_string()))
    })
}

/// Runs `operation` against the open splits for a group, trying every split
/// in turn — the report id in question may only be declared on one of them,
/// same reasoning as the Node adapter's `sendReport`/`receiveFeatureReport`.
fn with_open_group<T>(
    registry: &tauri::State<HidRegistry>,
    vendor_id: u16,
    product_id: u16,
    operation: impl FnOnce(&[Arc<OpenSplit>]) -> Result<T, String>,
) -> Result<T, String> {
    let map = registry.0.lock().unwrap();
    let group = map
        .get(&(vendor_id, product_id))
        .ok_or_else(|| "This HID interface is not open.".to_string())?;
    operation(&group.splits)
}

fn try_each(
    splits: &[Arc<OpenSplit>],
    mut operation: impl FnMut(&hidapi::HidDevice) -> hidapi::HidResult<()>,
) -> Result<(), String> {
    let mut last_error = None;
    for split in splits {
        let device = split.device.lock().unwrap();
        match operation(&device) {
            Ok(()) => return Ok(()),
            Err(error) => last_error = Some(error.to_string()),
        }
    }
    Err(last_error.unwrap_or_else(|| "No HID interface accepted the request.".to_string()))
}
