//! Tauri command wrappers around the shared `openmouse-hid` crate
//! (`crates/openmouse-hid`), which holds the actual transport and its long
//! history of hardware-specific notes. This file only adapts it to Tauri:
//! `State` injection, the `"hid-input-report"` event, and the app log.
//!
//! The frontend's `TauriHidDevice` (`src/native-hid/tauri-hid-device.ts`)
//! calls these and wraps them into the WebHID `HIDDevice` shape that
//! `@openmouse/protocol`'s drivers are written against.

use std::sync::Arc;

use openmouse_hid as core;
use tauri::Emitter;

pub use core::{HidApiHandle, HidInterface, HidRegistry};

/// Routes the shared crate's diagnostics into the in-app log buffer.
pub fn init_logging() {
    let _ = core::LOG_SINK.set(crate::applog::log);
}

// hid_list_interfaces and hid_open stay synchronous (main thread) on purpose;
// see the notes on `with_hid_api` in the shared crate.
#[tauri::command]
pub fn hid_list_interfaces(
    api_handle: tauri::State<HidApiHandle>,
    vendor_ids: Vec<u16>,
) -> Result<Vec<HidInterface>, String> {
    core::hid_list_interfaces(&api_handle, vendor_ids)
}

#[tauri::command]
pub fn hid_open(
    app: tauri::AppHandle,
    api_handle: tauri::State<HidApiHandle>,
    registry: tauri::State<HidRegistry>,
    vendor_id: u16,
    product_id: u16,
    preferred_usage_page: Option<u16>,
    preferred_usage: Option<u16>,
) -> Result<(), String> {
    let sink: core::ReportSink = Arc::new(move |payload| {
        let _ = app.emit("hid-input-report", payload);
    });
    core::hid_open(
        sink,
        &api_handle,
        &registry,
        vendor_id,
        product_id,
        preferred_usage_page,
        preferred_usage,
    )
}

#[tauri::command]
pub async fn hid_close(
    registry: tauri::State<'_, HidRegistry>,
    vendor_id: u16,
    product_id: u16,
) -> Result<(), String> {
    core::hid_close(&registry, vendor_id, product_id)
}

#[tauri::command]
pub async fn hid_send_report(
    registry: tauri::State<'_, HidRegistry>,
    vendor_id: u16,
    product_id: u16,
    report_id: u8,
    data: Vec<u8>,
) -> Result<(), String> {
    core::hid_send_report(&registry, vendor_id, product_id, report_id, data)
}

#[tauri::command]
pub async fn hid_send_feature_report(
    registry: tauri::State<'_, HidRegistry>,
    vendor_id: u16,
    product_id: u16,
    report_id: u8,
    data: Vec<u8>,
) -> Result<(), String> {
    core::hid_send_feature_report(&registry, vendor_id, product_id, report_id, data)
}

#[tauri::command]
pub async fn hid_get_input_report(
    registry: tauri::State<'_, HidRegistry>,
    vendor_id: u16,
    product_id: u16,
    report_id: u8,
    length: usize,
) -> Result<Vec<u8>, String> {
    core::hid_get_input_report(&registry, vendor_id, product_id, report_id, length)
}

#[tauri::command]
pub async fn hid_get_feature_report(
    registry: tauri::State<'_, HidRegistry>,
    vendor_id: u16,
    product_id: u16,
    report_id: u8,
    length: usize,
) -> Result<Vec<u8>, String> {
    core::hid_get_feature_report(&registry, vendor_id, product_id, report_id, length)
}
