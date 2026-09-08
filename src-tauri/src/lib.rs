use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::webview::WebviewWindowBuilder;
use tauri::{AppHandle, LogicalSize, Manager, Size, WebviewWindow, WindowEvent};

#[macro_use]
mod applog;
mod conflicting_apps;
mod discord_rpc;
mod games;
mod hid;
mod linux_permissions;
mod resource_monitor;
use hid::{HidApiHandle, HidRegistry};
use resource_monitor::ResourceMonitorState;

/// Fraction of the primary display's work area the main window should
/// occupy on launch.
const SCREEN_FRACTION: f64 = 0.75;

fn size_window(window: &WebviewWindow) {
    if let Ok(Some(monitor)) = window.primary_monitor() {
        let scale = monitor.scale_factor();
        let logical = monitor.size().to_logical::<f64>(scale);
        let _ = window.set_size(Size::Logical(LogicalSize::new(
            logical.width * SCREEN_FRACTION,
            logical.height * SCREEN_FRACTION,
        )));
    }
    let _ = window.center();
}

/// Brings the main window to front, recreating it first if it doesn't
/// exist. On macOS, closing the last window doesn't quit the whole process
/// (that's normal platform convention — the app, and its tray icon, stay
/// alive) — Show OpenMouse from the tray needs somewhere to bring back.
/// Without this, "Show OpenMouse" from the tray would find no window and
/// silently do nothing (CONFIRMED — this is exactly what happened before
/// this existed).
fn show_main_window(app: &AppHandle) {
    let window = match app.get_webview_window("main") {
        Some(window) => window,
        None => {
            applog!("[tray] 'main' window doesn't exist, recreating it");
            let Some(config) = app.config().app.windows.iter().find(|w| w.label == "main") else {
                applog!("[tray] no 'main' window config found, can't recreate");
                return;
            };
            match WebviewWindowBuilder::from_config(app, config).and_then(|b| b.build()) {
                Ok(window) => {
                    // Recreating from the static config alone gives back
                    // its on-disk default size, not the 75%-of-screen size
                    // the initial launch applies — reapply that the same
                    // way (see `setup` below).
                    size_window(&window);
                    window
                }
                Err(e) => {
                    applog!("[tray] failed to recreate 'main' window: {e}");
                    return;
                }
            }
        }
    };

    if let Err(e) = window.unminimize() {
        applog!("[tray] unminimize() failed: {e}");
    }
    if let Err(e) = window.show() {
        applog!("[tray] show() failed: {e}");
    }
    if let Err(e) = window.set_focus() {
        applog!("[tray] set_focus() failed: {e}");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(discord_rpc::DiscordRpcState::default())
        .manage(HidRegistry::default())
        .manage(HidApiHandle::default())
        .manage(ResourceMonitorState::default())
        .manage(games::ProcessListState::default())
        .invoke_handler(tauri::generate_handler![
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
            applog::log_line,
            games::running_process_names,
            games::scan_installed_games,
            conflicting_apps::detect_conflicting_apps,
            resource_monitor::sample_resource_usage,
            linux_permissions::install_udev_rules,
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
                    "show" => show_main_window(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    // Left click toggles show/hide (only meaningful when
                    // the window still exists — nothing to hide otherwise,
                    // so that case just shows/recreates it, same as
                    // double-click). Double-click (Windows only —
                    // tray-icon doesn't report this on macOS/Linux) always
                    // shows rather than toggling: a double-click is two
                    // rapid single clicks first, so without this arm the
                    // pair would show-then-hide the window right back out
                    // from under the user.
                    match event {
                        tauri::tray::TrayIconEvent::Click {
                            button: tauri::tray::MouseButton::Left,
                            button_state: tauri::tray::MouseButtonState::Up,
                            ..
                        } => {
                            let app = tray.app_handle();
                            match app.get_webview_window("main").map(|w| w.is_visible()) {
                                Some(Ok(true)) => {
                                    if let Some(window) = app.get_webview_window("main") {
                                        let _ = window.hide();
                                    }
                                }
                                _ => show_main_window(app),
                            }
                        }
                        tauri::tray::TrayIconEvent::DoubleClick { .. } => {
                            show_main_window(tray.app_handle());
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            if let Some(window) = app.get_webview_window("main") {
                size_window(&window);
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
            //
            // Linux-only guard: tao's Linux (GTK) backend implements
            // set_ignore_cursor_events via `input_shape_combine_region` and
            // unwraps the window's gtk::Window, which is still None at this
            // point of setup for the transparent undecorated overlay — so
            // calling it here panics the whole app on launch
            // ("called Option::unwrap() on a None value" in tao's
            // event_loop.rs, CursorIgnoreEvents arm). Windows/macOS backends
            // implement it without that unwrap and are fine. The overlay
            // starts invisible anyway, so on Linux it just can't be made
            // click-through this way for now.
            #[cfg(not(target_os = "linux"))]
            if let Some(overlay) = app.get_webview_window("overlay") {
                let _ = overlay.set_ignore_cursor_events(true);
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the main window hides it to the tray instead of
            // quitting — the app (and its tray icon) keeps running.
            // "Quit" from the tray menu is the actual exit. Guarded to the
            // main window specifically so this doesn't interfere with the
            // overlay window's own show/hide, which it manages itself (see
            // OverlayApp.tsx).
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
