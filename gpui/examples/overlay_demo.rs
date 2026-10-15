//! `cargo run --example overlay_demo -- [corner] [error]` — shows only the game
//! overlay toast for a few seconds, so its placement can be checked on its own.
use gpui_kit::*;
use openmouse_gpui::{native_window::OverlayCorner, overlay, theme::{self, ThemeChoice}};

fn main() {
    let corner = OverlayCorner::parse(&std::env::args().nth(1).unwrap_or_default());
    let error = std::env::args().nth(2).as_deref() == Some("error");
    application().with_assets(assets::AllAssets).run(move |cx| {
        init(cx);
        // Same palette the app paints: Dynamic, from the current wallpaper.
        // The accent the app last saved (sampling the wallpaper takes seconds).
        let accent = openmouse_gpui::settings::Settings::load().dynamic_hex;
        theme::apply_choice(ThemeChoice::Dynamic, accent, 100.0, 1.0, None, cx);
        overlay::show(
            cx,
            None,
            if error {
                "Couldn't apply \"Counter-Strike 2\" profile: the mouse is asleep".to_string()
            } else {
                "Applied \"Counter-Strike 2\" profile — 800 DPI, 1000 Hz.".to_string()
            },
            error,
            corner,
        );
        cx.spawn(async move |cx| {
            cx.background_executor().timer(std::time::Duration::from_secs(9)).await;
            let _ = cx.update(|cx| cx.quit());
        })
        .detach();
    });
}
