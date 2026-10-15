// A release build is a GUI app: no console window behind it on Windows.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use gpui_kit::component::TitleBar;
use gpui_kit::*;
use openmouse_gpui::app::AppView;

fn main() {
    application().with_assets(assets::AllAssets).run(|cx| {
        init(cx);

        // Tauri parity: `decorations: false` — hide the native macOS bar so
        // Kit's custom `TitleBar` owns the top strip (drag, double-click
        // zoom, traffic lights), instead of rendering underneath it.
        let options = WindowOptions {
            window_min_size: Some(size(px(720.), px(500.))),
            // Native compositor backdrop; scene tint supplies surface alpha.
            window_background: WindowBackgroundAppearance::Transparent,
            ..TitleBar::window_options()
        };

        // Theme seed: persisted choice wins (mirrors `initTheme()` reading
        // localStorage before first paint). The wallpaper sample NEVER blocks
        // open: a 20MP JPEG decode costs ~7s in debug and left the app
        // windowless with an empty log. Dynamic starts on the cached hex (or
        // fallback) and repaints when the background sample lands.
        let stored = openmouse_gpui::settings::Settings::load();
        // Seed the Dynamic cache from the last persisted sample so first
        // paint uses the real wallpaper color, not fallback green. The
        // signature revalidates on the first background tick.
        openmouse_gpui::theme::seed_dynamic_cache(None, stored.dynamic_hex);
        let wallpaper = openmouse_gpui::theme::cached_dynamic_accent();

        let (handle, view) = open_window(options, cx, |window, cx| {
            cx.new(|cx| AppView::build(stored.theme, wallpaper, window, cx))
        })
        .expect("failed to open main window");
        // Development aid: `OPENMOUSE_START_PAGE=updates|games|settings|supported`
        // opens straight on that page (handy for profiling and screenshots).
        if let Some(page) = std::env::var("OPENMOUSE_START_PAGE").ok().and_then(|name| match name.as_str() {
            "updates" | "whatsnew" => Some(openmouse_gpui::app::Page::Updates),
            "games" => Some(openmouse_gpui::app::Page::Games),
            "settings" => Some(openmouse_gpui::app::Page::Settings),
            "supported" => Some(openmouse_gpui::app::Page::Supported),
            _ => None,
        }) {
            view.update(cx, |view, cx| view.set_page(page, cx));
        }
        // Started by the OS at login (see autostart.rs): stay out of the way.
        // There is no tray yet, so minimized is the quiet option.
        if openmouse_gpui::autostart::launched_by_os() {
            let _ = cx.update_window(handle, |_, window, _| window.minimize_window());
        } else {
            cx.activate(true);
        }
        // Apply the theme AFTER the window exists: `Theme::change/update`
        // inside the build closure runs mid-construction and poisoned first
        // render (missing titlebar, voided components).
        let theme = stored.theme;
        let vibrancy = stored.dynamic_vibrancy;
        // Surface alpha belongs to theme colors, never the whole native window.
        openmouse_gpui::native_window::set_window_opacity(1.0);
        // `apply_choice` refreshes every window itself; the view's
        // `settings` already carries the same choice/vibrancy.
        let _ = cx.update_window(handle, |_, window, cx| {
            window.set_background_appearance(if stored.panel_blur > 0.0 {
                WindowBackgroundAppearance::Blurred
            } else {
                WindowBackgroundAppearance::Transparent
            });
            openmouse_gpui::theme::apply_choice(
                theme,
                wallpaper,
                vibrancy,
                stored.window_opacity / 100.0,
                Some(window),
                cx,
            );
        });
        // Live Dynamic: 2s wallpaper-signature poll, resample + repaint on
        // change (mirrors `startDynamicAccentWatcher`). Focus-triggered
        // checks ride on the same path via GPUI's window focus events.
        openmouse_gpui::theme::spawn_wallpaper_watcher(cx, &view, &handle);
        // Game art prefetch: download missing covers on the background
        // executor (curl, 20s/file cap), then notify so cards repaint from
        // letter tiles to art. Non-fatal.
        {
            let view_weak = view.downgrade();
            cx.spawn(async move |cx| {
                cx.background_executor()
                    .spawn(async {
                        let games = openmouse_gpui::games::load_catalog();
                        openmouse_gpui::games::prefetch_art(&games);
                    })
                    .await;
                let _ = view_weak.update(cx, |_, cx| {
                    cx.notify();
                });
            })
            .detach();
        }
        // Startup Dynamic sample: resample off-thread and repaint on landing
        // (covers cold start with no cache, not just wallpaper *changes*).
        if stored.theme == openmouse_gpui::theme::ThemeChoice::Dynamic {
            let view_weak = view.downgrade();
            let handle_copy = handle;
            cx.spawn(async move |cx| {
                let accent = cx
                    .background_executor()
                    .spawn(async { openmouse_gpui::theme::dynamic_accent() })
                    .await;
                if let Some(accent) = accent {
                    let _ = view_weak.update(cx, |view, cx| {
                        view.note_wallpaper_sample(accent, handle_copy, cx);
                    });
                }
            })
            .detach();
        }
    });
}
