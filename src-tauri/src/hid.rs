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
//! per interface, so devices here are grouped by (vendor id, product id,
//! interface number) and every split within a group is opened together —
//! same reasoning as the Node adapter's `candidateDevices()`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
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
    #[serde(rename = "interfaceNumber")]
    pub interface_number: i32,
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

#[derive(Default)]
pub struct HidRegistry(Mutex<HashMap<(u16, u16, i32), Vec<Arc<OpenSplit>>>>);

fn interface_key(vendor_id: u16, product_id: u16, interface_number: i32) -> String {
    format!("{vendor_id:04x}:{product_id:04x}:{interface_number}")
}

#[tauri::command]
pub fn hid_list_interfaces(vendor_ids: Vec<u16>) -> Result<Vec<HidInterface>, String> {
    let api = HidApi::new().map_err(|error| error.to_string())?;
    let mut groups: HashMap<(u16, u16, i32), Vec<&hidapi::DeviceInfo>> = HashMap::new();
    for info in api.device_list() {
        if !vendor_ids.contains(&info.vendor_id()) {
            continue;
        }
        groups
            .entry((info.vendor_id(), info.product_id(), info.interface_number()))
            .or_default()
            .push(info);
    }

    let mut result: Vec<HidInterface> = groups
        .into_iter()
        .map(|((vendor_id, product_id, interface_number), infos)| {
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
                key: interface_key(vendor_id, product_id, interface_number),
                vendor_id,
                product_id,
                interface_number,
                product_string,
                manufacturer_string,
            }
        })
        .collect();
    result.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(result)
}

#[tauri::command]
pub fn hid_open(
    app: tauri::AppHandle,
    registry: tauri::State<HidRegistry>,
    vendor_id: u16,
    product_id: u16,
    interface_number: i32,
) -> Result<(), String> {
    let group_key = (vendor_id, product_id, interface_number);
    if registry.0.lock().unwrap().contains_key(&group_key) {
        return Ok(());
    }

    let api = HidApi::new().map_err(|error| error.to_string())?;
    let paths: Vec<_> = api
        .device_list()
        .filter(|info| {
            info.vendor_id() == vendor_id
                && info.product_id() == product_id
                && info.interface_number() == interface_number
        })
        .map(|info| info.path().to_owned())
        .collect();
    if paths.is_empty() {
        return Err("No matching HID interface is currently connected.".into());
    }

    let key = interface_key(vendor_id, product_id, interface_number);
    let mut splits = Vec::new();
    for path in paths {
        let device = api.open_path(&path).map_err(|error| error.to_string())?;
        let stop = Arc::new(AtomicBool::new(false));
        let split = Arc::new(OpenSplit {
            device: Mutex::new(device),
            stop: stop.clone(),
        });
        spawn_reader(app.clone(), key.clone(), split.clone());
        splits.push(split);
    }

    registry.0.lock().unwrap().insert(group_key, splits);
    Ok(())
}

fn spawn_reader(app: tauri::AppHandle, key: String, split: Arc<OpenSplit>) {
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
    });
}

#[tauri::command]
pub fn hid_close(
    registry: tauri::State<HidRegistry>,
    vendor_id: u16,
    product_id: u16,
    interface_number: i32,
) -> Result<(), String> {
    if let Some(splits) = registry
        .0
        .lock()
        .unwrap()
        .remove(&(vendor_id, product_id, interface_number))
    {
        for split in splits {
            split.stop.store(true, Ordering::Relaxed);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn hid_send_report(
    registry: tauri::State<HidRegistry>,
    vendor_id: u16,
    product_id: u16,
    interface_number: i32,
    report_id: u8,
    data: Vec<u8>,
) -> Result<(), String> {
    with_open_group(&registry, vendor_id, product_id, interface_number, |splits| {
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
    interface_number: i32,
    report_id: u8,
    data: Vec<u8>,
) -> Result<(), String> {
    with_open_group(&registry, vendor_id, product_id, interface_number, |splits| {
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
    interface_number: i32,
    report_id: u8,
    length: usize,
) -> Result<Vec<u8>, String> {
    with_open_group(&registry, vendor_id, product_id, interface_number, |splits| {
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
    interface_number: i32,
    operation: impl FnOnce(&[Arc<OpenSplit>]) -> Result<T, String>,
) -> Result<T, String> {
    let map = registry.0.lock().unwrap();
    let splits = map
        .get(&(vendor_id, product_id, interface_number))
        .ok_or_else(|| "This HID interface is not open.".to_string())?;
    operation(splits)
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
