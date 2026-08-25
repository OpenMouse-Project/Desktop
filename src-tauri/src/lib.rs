use std::sync::Mutex;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{LogicalSize, Manager, Size, State, WebviewWindow, WindowEvent};

#[macro_use]
mod applog;
mod discord_rpc;
mod games;
mod hid;
mod resource_monitor;
use hid::{HidApiHandle, HidRegistry};
use resource_monitor::ResourceMonitorState;

/// The two toggable app modes.
///
/// Bridge Mode: minimal tray-resident companion (game detection, battery
/// alerts, native HID) — closing the window hides it to the tray, and the
/// window itself is small (a tray-popover, not a full app window).
/// Full Desktop Mode: the full device-configuration UI — closing the
/// window quits the app like a normal desktop app, and the window fills
/// 75% of the primary display.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
enum AppMode {
    Bridge,
    FullDesktop,
}

/// Fixed logical size for the Bridge Mode popover window.
const BRIDGE_WINDOW_SIZE: (f64, f64) = (320.0, 420.0);

/// Fraction of the primary display's work area Full Desktop Mode's window
/// should occupy.
const FULL_DESKTOP_SCREEN_FRACTION: f64 = 0.75;

struct ModeState(Mutex<AppMode>);

fn resize_for_mode(window: &WebviewWindow, mode: AppMode) {
    match mode {
        AppMode::Bridge => {
            let _ = window.set_size(Size::Logical(LogicalSize::new(
                BRIDGE_WINDOW_SIZE.0,
                BRIDGE_WINDOW_SIZE.1,
            )));
        }
        AppMode::FullDesktop => {
            if let Ok(Some(monitor)) = window.primary_monitor() {
                let scale = monitor.scale_factor();
                let logical = monitor.size().to_logical::<f64>(scale);
                let _ = window.set_size(Size::Logical(LogicalSize::new(
                    logical.width * FULL_DESKTOP_SCREEN_FRACTION,
                    logical.height * FULL_DESKTOP_SCREEN_FRACTION,
                )));
            }
        }
    }
    let _ = window.center();
}

#[tauri::command]
fn get_mode(state: State<ModeState>) -> AppMode {
    *state.0.lock().unwrap()
}

#[tauri::command]
fn set_mode(mode: AppMode, window: WebviewWindow, state: State<ModeState>) -> AppMode {
    *state.0.lock().unwrap() = mode;
    resize_for_mode(&window, mode);
    mode
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(ModeState(Mutex::new(AppMode::FullDesktop)))
        .manage(discord_rpc::DiscordRpcState::default())
        .manage(HidRegistry::default())
        .manage(HidApiHandle::default())
        .manage(ResourceMonitorState::default())
        .manage(games::ProcessListState::default())
        .invoke_handler(tauri::generate_handler![
            get_mode,
            set_mode,
            hid::hid_list_interfaces,
            hid::hid_open,
            hid::hid_close,
            hid::hid_send_report,
            hid::hid_send_feature_report,
            hid::hid_get_feature_report,
            discord_rpc::enable,
            discord_rpc::disable,
            discord_rpc::update_activity,
            applog::get_logs,
            applog::export_logs,
            games::running_process_names,
            resource_monitor::sample_resource_usage,
        ])
        .setup(|app| {
            let show = MenuItem::with_id(app, "show", "Show OpenMouse", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(app)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &separator, &quit])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            match window.is_visible() {
                                Ok(true) => {
                                    let _ = window.hide();
                                }
                                _ => {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                        }
                    }
                })
                .build(app)?;

            // Size the window to match the initial mode (Full Desktop by
            // default — see ModeState above).
            if let Some(window) = app.get_webview_window("main") {
                let state = app.state::<ModeState>();
                let mode = *state.0.lock().unwrap();
                resize_for_mode(&window, mode);
            }

            // The overlay window (game-switch toasts — src/OverlayApp.tsx)
            // starts invisible and gets shown/hidden/sized/positioned
            // entirely from its own script (it reads the user's saved
            // corner/size preference — lib/overlay-settings.ts, editable in
            // Settings — and recomputes against the current monitor on
            // every show, rather than a fixed position baked in once here).
            // Click-through is the one thing that's the same regardless of
            // preference, set once at startup: it can never block input to
            // whatever's underneath, a passive HUD that doesn't compete
            // with the game for clicks.
            if let Some(overlay) = app.get_webview_window("overlay") {
                let _ = overlay.set_ignore_cursor_events(true);
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // In Bridge Mode, closing the window hides it to the tray instead
            // of quitting — the companion process keeps running. In Full
            // Desktop Mode, closing behaves like a normal app quit.
            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<ModeState>();
                let mode = *state.0.lock().unwrap();
                if mode == AppMode::Bridge {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
