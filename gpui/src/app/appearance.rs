//! Theme and accent colour.

use super::*;

/// Theme swatches in Settings, in display order. Narrows the web app's seven
/// presets to the five in scope: System follow + Matt dark + OLED dark +
/// Matt light + Dynamic. (Emerald/Violet/Ice/Mono + custom CSS stay web-only
/// for now.)
pub(super) const THEME_CHOICES: [ThemeChoice; 5] = [
    ThemeChoice::System,
    ThemeChoice::Dark,
    ThemeChoice::Oled,
    ThemeChoice::Light,
    ThemeChoice::Dynamic,
];

/// Accent swatches in Settings: (label, hue in degrees).
pub(super) const ACCENT_SWATCHES: [(&str, f32); 8] = [
    ("Red", 0.0),
    ("Orange", 25.0),
    ("Yellow", 48.0),
    ("Green", 130.0),
    ("Teal", 175.0),
    ("Blue", 215.0),
    ("Purple", 265.0),
    ("Pink", 330.0),
];

pub(super) const ACCENT_SWATCH_SIZE: f32 = 28.0;

impl AppView {
    /// Switch theme; mirrors the Settings theme switcher. Paints through
    /// [`crate::theme::apply_choice`] so every window refreshes. Switching
    /// to Dynamic resamples the wallpaper immediately (mirrors Settings'
    /// `force` refresh) instead of showing the fallback until the 2s watcher
    /// tick lands.
    pub fn set_theme(&mut self, theme: ThemeChoice, window: &mut Window, cx: &mut Context<Self>) {
        // Palette before the switch, for the crossfade below.
        let previous = cx.theme().colors;
        self.settings.theme = theme;
        self.settings.save();
        if theme == ThemeChoice::Dynamic {
            let vibrancy = self.settings.dynamic_vibrancy;
            // Instant path: paint the cached hex synchronously (no I/O —
            // mirrors the web app reading `DYNAMIC_HEX_KEY`), then verify
            // the signature off-thread and correct only if the wallpaper
            // actually changed. Same wallpaper → instant, zero subprocess.
            if let Some(accent) = crate::theme::cached_dynamic_accent() {
                crate::theme::apply_choice(
                    ThemeChoice::Dynamic,
                    Some(accent),
                    vibrancy,
                    self.settings.window_opacity / 100.0,
                    self.settings.accent,
                    Some(window),
                    cx,
                );
            }
            cx.spawn({
                async move |view, cx| {
                    let accent = cx
                        .background_executor()
                        .spawn(async { crate::theme::dynamic_accent() })
                        .await;
                    // Fallback green only if sampling truly fails.
                    let accent = accent.unwrap_or((93, 222, 137));
                    let _ = view.update(cx, |view: &mut Self, cx| {
                        if view.settings.theme != ThemeChoice::Dynamic {
                            return;
                        }
                        view.settings.dynamic_hex = Some(accent);
                        view.settings.save();
                        crate::theme::apply_choice(
                            ThemeChoice::Dynamic,
                            Some(accent),
                            vibrancy,
                            view.settings.window_opacity / 100.0,
                            view.settings.accent,
                            None,
                            cx,
                        );
                        cx.notify();
                    });
                }
            })
            .detach();
        } else {
            self.repaint_theme(Some(window), cx);
        }
        // The new palette is live; fade into it, and replay the art's
        // entrance so the field slides back in from the side.
        self.theme_seq += 1;
        crate::theme::crossfade_colors(previous, cx.theme().colors, cx);
        cx.notify();
    }

    /// Set (or clear) the user's accent override. The palette fades to it and
    /// the ASCII field replays its entrance, exactly like a theme switch.
    pub fn set_accent(
        &mut self,
        accent: Option<(u8, u8, u8)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.accent = accent;
        self.settings.save();
        let previous = cx.theme().colors;
        self.repaint_theme(Some(window), cx);
        self.theme_seq += 1;
        crate::theme::crossfade_colors(previous, cx.theme().colors, cx);
        cx.notify();
    }

    /// Wallpaper watcher callback: a fresh sample landed. Repaints only
    /// when Dynamic is the live choice (mirrors `refreshDynamicAccent`'s
    /// signature-moved path); other choices keep their palette.
    pub fn note_wallpaper_sample(
        &mut self,
        accent: (u8, u8, u8),
        _window: gpui_kit::AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if self.settings.theme != ThemeChoice::Dynamic {
            return;
        }
        // Persist the sample so the next launch repaints it instantly
        // instead of flashing fallback green (mirrors DYNAMIC_HEX_KEY).
        self.settings.dynamic_hex = Some(accent);
        self.settings.save();
        let previous = cx.theme().colors;
        crate::theme::apply_choice(
            ThemeChoice::Dynamic,
            Some(accent),
            self.settings.dynamic_vibrancy,
            self.settings.window_opacity / 100.0,
            self.settings.accent,
            None,
            cx,
        );
        // A new wallpaper is a new palette: fade it in, and replay the art's
        // entrance like a theme switch.
        self.theme_seq += 1;
        crate::theme::crossfade_colors(previous, cx.theme().colors, cx);
        cx.notify();
    }

    /// Repaint the active theme from current settings (theme + vibrancy).
    pub(super) fn repaint_theme(&self, window: Option<&mut Window>, cx: &mut App) {
        // Dynamic repaints from the cached sample — passing None would reset
        // the shell to the fallback green on every vibrancy tick.
        let wallpaper = if self.settings.theme == ThemeChoice::Dynamic {
            crate::theme::cached_dynamic_accent()
        } else {
            None
        };
        crate::theme::apply_choice(
            self.settings.theme,
            wallpaper,
            self.settings.dynamic_vibrancy,
            self.settings.window_opacity / 100.0,
            self.settings.accent,
            window,
            cx,
        );
    }

    /// Accent selector: a row of basic color swatches. Each swatch is its own
    /// hit target; the one matching the stored accent gets a ring.
    pub(super) fn render_accent_picker(&self, cx: &mut Context<Self>) -> AnyElement {
        let selected_accent = self.settings.accent;
        h_flex()
            .items_center()
            .gap_2()
            .children(ACCENT_SWATCHES.iter().enumerate().map(|(index, (_, hue))| {
                let hue = *hue;
                let value = crate::theme::picked_accent(hue);
                let selected = selected_accent == Some(value);
                div()
                    .id(("accent-swatch", index))
                    .flex_none()
                    .size(px(ACCENT_SWATCH_SIZE))
                    .rounded_full()
                    .bg(crate::theme::picked_accent_hsla(hue))
                    .border_2()
                    .border_color(if selected {
                        cx.theme().foreground
                    } else {
                        Hsla { a: 0.0, ..cx.theme().foreground }
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.set_accent(Some(value), window, cx);
                    }))
            }))
            .into_any_element()
    }
}
