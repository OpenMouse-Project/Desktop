//! System tray menu, and the device/battery lines at the top of it.
//!
//! The menu is built once at startup with two disabled "label" items above
//! the Show/Quit actions, the way Razer Synapse's tray shows the paired mouse
//! and its charge. Their text is rewritten from the frontend through the
//! `tray_set_device_status` command whenever the cached `MouseStatus` in
//! `use-mouse-connection.ts` changes, so right-clicking the tray icon shows
//! the last-read battery level without bringing the window back.
//!
//! Only the text lives here. Reading the battery is the frontend's job (the
//! protocol drivers run in the webview, see `native-hid/`), so a webview that
//! is hidden to the tray still owns the refresh cadence; the frontend just
//! reads on a slower interval while hidden instead of skipping entirely.

use std::sync::Mutex;

use serde::Deserialize;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{App, AppHandle, Manager, Wry};

/// A monochrome silhouette (black ink, transparent elsewhere) derived from
/// the OpenMouse mark, distinct from the full-color Dock/window icon. macOS
/// recolors a "template" image itself for light/dark menu bars, which a
/// flat-color icon can't do — the app's default window icon rendered as a
/// dark, hard-to-spot blob against a dark menu bar before this existed.
const TRAY_ICON_BYTES: &[u8] = include_bytes!("../icons/tray-icon@2x.png");

/// Text shown while nothing is connected, and the tooltip's base.
const NO_DEVICE: &str = "No device connected";
const APP_NAME: &str = "OpenMouse";

/// What the frontend knows about the connected device that is worth showing
/// in a two-line tray menu. `None` battery means the mouse has no cell (wired,
/// or a driver that does not report one).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayDeviceStatus {
    pub name: String,
    pub battery_percent: Option<u8>,
    /// The protocol's `MouseStatus.batteryState` label, e.g. "Charging",
    /// "Discharging", "Full", "Unknown".
    pub battery_state: String,
}

struct TrayHandles {
    icon: TrayIcon<Wry>,
    menu: Menu<Wry>,
    device: MenuItem<Wry>,
    battery: MenuItem<Wry>,
    /// Whether `battery` is currently inserted in `menu`. It is removed
    /// rather than left reading "Battery: —" when nothing is connected, so
    /// the menu collapses to one label line.
    battery_shown: bool,
}

#[derive(Default)]
pub struct TrayState(Mutex<Option<TrayHandles>>);

/// Builds the tray icon and menu. `on_show`/`on_quit` are what the existing
/// Show/Quit items and the left-click/double-click handlers already did in
/// lib.rs; they stay there so this module owns only the menu text.
pub fn build(
    app: &App,
    on_menu: fn(&AppHandle, &str),
    on_tray_icon: fn(&TrayIcon<Wry>, tauri::tray::TrayIconEvent),
) -> tauri::Result<()> {
    let device = MenuItem::with_id(app, "tray-device", NO_DEVICE, false, None::<&str>)?;
    let battery = MenuItem::with_id(app, "tray-battery", "Battery", false, None::<&str>)?;
    let show = MenuItem::with_id(app, "show", "Show OpenMouse", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &device,
            &PredefinedMenuItem::separator(app)?,
            &show,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    let tray_icon = Image::from_bytes(TRAY_ICON_BYTES)
        .unwrap_or_else(|_| app.default_window_icon().unwrap().clone());
    let icon = TrayIconBuilder::new()
        .icon(tray_icon)
        // macOS-only; ignored elsewhere. Tells the system this is a
        // monochrome glyph it should recolor for the current menu bar
        // appearance, rather than a flat icon to draw as-is.
        .icon_as_template(true)
        .tooltip(APP_NAME)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| on_menu(app, event.id.as_ref()))
        .on_tray_icon_event(move |tray, event| on_tray_icon(tray, event))
        .build(app)?;

    app.manage(TrayState(Mutex::new(Some(TrayHandles {
        icon,
        menu,
        device,
        battery,
        battery_shown: false,
    }))));
    Ok(())
}

/// One line for the menu and a shorter one for the icon tooltip.
fn battery_lines(status: &TrayDeviceStatus) -> Option<(String, String)> {
    let percent = status.battery_percent?;
    let state = status.battery_state.trim();
    let menu = if state.is_empty() || state.eq_ignore_ascii_case("unknown") {
        format!("Battery: {percent}%")
    } else {
        format!("Battery: {percent}% ({state})")
    };
    let tooltip = if state.eq_ignore_ascii_case("charging")
        || state.eq_ignore_ascii_case("charging slowly")
        || state.eq_ignore_ascii_case("almost full")
    {
        format!("{percent}%, charging")
    } else {
        format!("{percent}%")
    };
    Some((menu, tooltip))
}

/// Rewrites the device and battery lines. Called by the frontend every time
/// its cached status changes; `None` means the device went away.
#[tauri::command]
pub fn tray_set_device_status(
    state: tauri::State<TrayState>,
    status: Option<TrayDeviceStatus>,
) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    let Some(handles) = guard.as_mut() else {
        // The tray failed to build at startup; nothing to update.
        return Ok(());
    };

    let (device_text, battery_text, tooltip) = match &status {
        Some(status) => {
            let name = if status.name.trim().is_empty() { "Connected device" } else { status.name.trim() };
            match battery_lines(status) {
                Some((menu, tip)) => (name.to_string(), Some(menu), format!("{APP_NAME} · {name} · {tip}")),
                None => (name.to_string(), None, format!("{APP_NAME} · {name}")),
            }
        }
        None => (NO_DEVICE.to_string(), None, APP_NAME.to_string()),
    };

    handles.device.set_text(&device_text).map_err(|e| e.to_string())?;
    match (&battery_text, handles.battery_shown) {
        (Some(text), _) => {
            handles.battery.set_text(text).map_err(|e| e.to_string())?;
            if !handles.battery_shown {
                // Directly under the device line.
                handles.menu.insert(&handles.battery, 1).map_err(|e| e.to_string())?;
                handles.battery_shown = true;
            }
        }
        (None, true) => {
            handles.menu.remove(&handles.battery).map_err(|e| e.to_string())?;
            handles.battery_shown = false;
        }
        (None, false) => {}
    }
    // Tooltip failures are cosmetic (unsupported on some Linux trays), so
    // they are logged rather than failing the whole update.
    if let Err(e) = handles.icon.set_tooltip(Some(&tooltip)) {
        applog!("[tray] set_tooltip failed: {e}");
    }
    Ok(())
}
