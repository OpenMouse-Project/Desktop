use std::sync::Mutex;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, State, WindowEvent};

/// The two toggable app modes.
///
/// Bridge Mode: minimal tray-resident companion (game detection, battery
/// alerts, native HID) — closing the window hides it to the tray.
/// Full Desktop Mode: the full device-configuration UI — closing the
/// window quits the app like a normal desktop app.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
enum AppMode {
    Bridge,
    FullDesktop,
}

struct ModeState(Mutex<AppMode>);

#[tauri::command]
fn get_mode(state: State<ModeState>) -> AppMode {
    *state.0.lock().unwrap()
}

#[tauri::command]
fn set_mode(mode: AppMode, state: State<ModeState>) -> AppMode {
    *state.0.lock().unwrap() = mode;
    mode
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(ModeState(Mutex::new(AppMode::FullDesktop)))
        .invoke_handler(tauri::generate_handler![get_mode, set_mode])
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
