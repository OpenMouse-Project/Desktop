use std::sync::Mutex;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::webview::WebviewWindowBuilder;
use tauri::{AppHandle, LogicalSize, Manager, Size, State, WebviewWindow, WindowEvent};

#[macro_use]
mod applog;
mod conflicting_apps;
mod cross_grade;
mod discord_rpc;
mod games;
mod hid;
mod linux_permissions;
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

struct ModeState(Mutex<Option<AppMode>>);

/// Where the chosen mode is persisted — in this build's own (identifier-
/// scoped) app-data dir, deliberately NOT the shared one cross_grade.rs
/// uses for the installed-variants manifest: Bridge and Desktop are
/// separate installs with their own identifiers, and each should remember
/// its own last-chosen mode independently.
fn mode_prefs_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_data_dir().ok().map(|dir| dir.join("mode.json"))
}

/// `None` here — on disk or in ModeState — means "never chosen yet", which
/// is what tells the frontend to show WelcomeChooser instead of a mode's
/// normal view. This used to default straight to AppMode::FullDesktop with
/// nothing written to disk (CONFIRMED bug: every launch, of either build,
/// reset back to Full Desktop instead of remembering the last choice —
/// harmless for the Desktop build, since it bundles both UIs, but meant
/// the Bridge build re-showed CrossGradePrompt on every single launch
/// instead of just the first).
fn load_persisted_mode(app: &AppHandle) -> Option<AppMode> {
    let path = mode_prefs_path(app)?;
    let contents = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

fn save_persisted_mode(app: &AppHandle, mode: AppMode) {
    let Some(path) = mode_prefs_path(app) else {
        applog!("[mode] couldn't resolve app data dir, mode won't be remembered");
        return;
    };
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            applog!("[mode] couldn't create app data dir: {e}");
            return;
        }
    }
    match serde_json::to_string(&mode) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                applog!("[mode] couldn't persist mode: {e}");
            }
        }
        Err(e) => applog!("[mode] couldn't serialize mode: {e}"),
    }
}

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

/// `None` = never chosen yet (fresh install, or the prefs file couldn't be
/// read) — the frontend renders WelcomeChooser in that case instead of
/// either mode's normal view.
#[tauri::command]
fn get_mode(state: State<ModeState>) -> Option<AppMode> {
    *state.0.lock().unwrap()
}

#[tauri::command]
fn set_mode(mode: AppMode, window: WebviewWindow, state: State<ModeState>, app: AppHandle) -> AppMode {
    *state.0.lock().unwrap() = Some(mode);
    save_persisted_mode(&app, mode);
    resize_for_mode(&window, mode);
    mode
}

/// Brings the main window to front, recreating it first if it doesn't
/// exist. On macOS, closing the last window doesn't quit the whole process
/// (that's normal platform convention — the app, and its tray icon, stay
/// alive) but Full Desktop Mode's close handler doesn't intercept that
/// close, so the window itself is genuinely destroyed rather than hidden.
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
                    // its on-disk default size, not whatever mode the app
                    // was actually in — reapply that the same way the
                    // initial launch does (see `setup` below). No-op if a
                    // mode was never chosen (WelcomeChooser still showing).
                    if let Some(mode) = *app.state::<ModeState>().0.lock().unwrap() {
                        resize_for_mode(&window, mode);
                    }
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
        .manage(ModeState(Mutex::new(None)))
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
            games::scan_installed_games,
            conflicting_apps::detect_conflicting_apps,
            resource_monitor::sample_resource_usage,
            linux_permissions::install_udev_rules,
            cross_grade::register_variant,
            cross_grade::get_installed_variants,
            cross_grade::switch_to_installed_variant,
            cross_grade::find_installer_asset,
            cross_grade::download_and_launch_installer,
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

            // Load whatever mode was last chosen (see save_persisted_mode /
            // load_persisted_mode) — None means this is a fresh install
            // that's never had a mode chosen yet, in which case the
            // frontend shows WelcomeChooser and there's nothing to resize
            // for until that picks one (via set_mode, which persists and
            // resizes itself).
            let mode = load_persisted_mode(app.handle());
            *app.state::<ModeState>().0.lock().unwrap() = mode;
            if let (Some(window), Some(mode)) = (app.get_webview_window("main"), mode) {
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
            // Closing the main window always hides it to the tray instead
            // of quitting, in both modes — the app (and its tray icon)
            // keeps running either way, same as it always did in Bridge
            // Mode. "Quit" from the tray menu is the actual exit. Guarded
            // to the main window specifically so this doesn't interfere
            // with the overlay window's own show/hide, which it manages
            // itself (see OverlayApp.tsx).
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
