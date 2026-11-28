//! The Settings page.

use super::*;

impl AppView {
    /// Shared section surface and heading keep settings on one alignment spine.
    pub(super) fn render_settings_section(
        &self,
        cx: &mut Context<Self>,
        title: &'static str,
        description: &'static str,
    ) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(description),
                    ),
            )
            .child(div().h(px(1.)).w_full().bg(cx.theme().border))
    }

    /// Labeled switch row: title + description on the left, [`Switch`] on the
    /// right. `switch_id` selects which placeholder setting it drives.
    pub(super) fn render_switch_row(
        &self,
        switch_id: &'static str,
        title: &'static str,
        description: &'static str,
        checked: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap_4()
            .py_1()
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(cx.theme().foreground)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(description),
                    )
                    .children(
                        match switch_id {
                            "startup-switch" => self.startup_error.clone(),
                            "discord-switch" => self.discord_error.clone(),
                            _ => None,
                        }
                        .map(|error| div().text_xs().text_color(cx.theme().danger).child(error)),
                    ),
            )
            .child(
                Switch::new(switch_id)
                    .checked(checked)
                    .on_change(cx.listener(move |this, enabled: &bool, _, cx| {
                        if switch_id == "startup-switch" {
                            this.set_launch_on_startup(*enabled);
                        } else if switch_id == "discord-device" {
                            this.settings.discord_show_device = *enabled;
                            this.refresh_presence(cx);
                        } else if switch_id == "discord-game" {
                            this.settings.discord_show_game = *enabled;
                            this.refresh_presence(cx);
                        } else if switch_id == "discord-stats" {
                            this.settings.discord_show_stats = *enabled;
                            this.refresh_presence(cx);
                        } else if switch_id == "overlay-sound" {
                            this.settings.overlay_sound = *enabled;
                            if *enabled {
                                crate::audio::play(crate::audio::Chime::Success);
                            }
                        } else if switch_id == "overlay-switch" {
                            this.settings.overlay_enabled = *enabled;
                            if *enabled {
                                this.announce("Game notifications are on.".into(), false, true, cx);
                            } else if let Some(open) = this.overlay.take() {
                                let _ = open.update(cx, |_, window, _| window.remove_window());
                            }
                        } else if switch_id == "instant-apply-switch" {
                            this.settings.instant_apply = *enabled;
                            // Nothing may stay staged once edits stop being staged.
                            this.devices.staged = Staged::default();
                        } else {
                            this.settings.discord_presence = *enabled;
                            this.set_discord(*enabled, cx);
                        }
                        this.settings.save();
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    /// Appearance controls retain their entities and immediate persistence.
    pub(super) fn render_settings(&self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let selected = THEME_CHOICES
            .iter()
            .position(|choice| *choice == self.settings.theme);
        let show_vibrancy = self.settings.theme == ThemeChoice::Dynamic;
        div()
            .size_full()
            .overflow_y_scrollbar()
            .child(
                div()
                    .w_full()
                    .max_w(rems(48.))
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_6()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Settings"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Appearance and app preferences"),
                            ),
                    )
                    .child(
                        self.render_settings_section(
                            cx,
                            "Appearance",
                            "Choose a theme and adjust the window backdrop.",
                        )
                        .child(
                            RadioGroup::horizontal("theme-choice")
                                .children(THEME_CHOICES.into_iter().map(|choice| {
                                    let description = match choice {
                                        ThemeChoice::System => {
                                            if cfg!(target_os = "macos") {
                                                "Follow macOS"
                                            } else if cfg!(target_os = "windows") {
                                                "Follow Windows"
                                            } else {
                                                "Follow system"
                                            }
                                        }
                                        ThemeChoice::Dark => "Warm, muted",
                                        ThemeChoice::Oled => "True black",
                                        ThemeChoice::Light => "Bright, clear",
                                        ThemeChoice::Dynamic => "From wallpaper",
                                    };
                                    Radio::new(choice.label())
                                        .label(choice.label())
                                        .flex_1()
                                        .min_w(rems(7.5))
                                        .p_3()
                                        .border_1()
                                        .rounded(cx.theme().radius)
                                        .border_color(if choice == self.settings.theme {
                                            cx.theme().primary
                                        } else {
                                            cx.theme().border
                                        })
                                        .when(choice == self.settings.theme, |this| {
                                            this.bg(Hsla { a: 0.12, ..cx.theme().primary })
                                        })
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(description),
                                        )
                                }))
                                .selected_index(selected)
                                .on_click(cx.listener(|this, index: &usize, window, cx| {
                                    if let Some(choice) = THEME_CHOICES.get(*index) {
                                        this.set_theme(*choice, window, cx);
                                    }
                                })),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_start()
                                .gap_2()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .child("Accent color"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child("Override the theme's accent, or reset to use the theme's own."),
                                )
                                .child(
                                    h_flex()
                                        .items_center()
                                        .gap_3()
                                        .child(self.render_accent_picker(cx))
                                        .when(self.settings.accent.is_some(), |row| {
                                            row.child(
                                                Button::new("accent-reset")
                                                    .compact()
                                                    .label("Reset")
                                                    .on_click(cx.listener(|this, _, window, cx| {
                                                        this.set_accent(None, window, cx);
                                                    })),
                                            )
                                        }),
                                ),
                        )
                        .child(div().h(px(1.)).w_full().bg(cx.theme().border))
                        .child(self.render_effect_slider(
                            "Background opacity",
                            "Lower the tint to reveal more of the desktop behind the window.",
                            &self.opacity_slider,
                            format!("{:.0}%", self.settings.window_opacity),
                            cx,
                        ))
                        .child(
                            h_flex()
                                .items_center()
                                .justify_between()
                                .gap_4()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .flex_1()
                                        .min_w_0()
                                        .gap_1()
                                        .child(div().text_sm().child("Background blur"))
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child("Soften the desktop behind the window. macOS controls blur strength."),
                                        ),
                                )
                                .child(
                                    Switch::new("background-blur")
                                        .checked(self.settings.panel_blur > 0.0)
                                        .on_change(cx.listener(|this, enabled: &bool, window, cx| {
                                            this.settings.panel_blur = if *enabled { MAX_BLUR } else { 0.0 };
                                            window.set_background_appearance(if *enabled {
                                                WindowBackgroundAppearance::Blurred
                                            } else {
                                                WindowBackgroundAppearance::Transparent
                                            });
                                            this.settings.save();
                                            cx.notify();
                                        })),
                                ),
                        )
                        .when(show_vibrancy, |this| {
                            this.child(self.render_effect_slider(
                                "Vibrancy",
                                "Adjust the intensity of your wallpaper-derived colors.",
                                &self.vibrancy_slider,
                                format!("{:.0}%", self.settings.dynamic_vibrancy),
                                cx,
                            ))
                        }),
                    )
                    .child(
                        self.render_settings_section(
                            cx,
                            "General",
                            "Startup and integration preferences.",
                        )
                        .child(self.render_switch_row(
                            "startup-switch",
                            "Launch on startup",
                            "Open OpenMouse automatically when you log in.",
                            self.settings.launch_on_startup,
                            cx,
                        ))
                        .child(div().h(px(1.)).w_full().bg(cx.theme().border))
                        .child(self.render_switch_row(
                            "discord-switch",
                            "Discord Rich Presence",
                            "Show OpenMouse in your Discord status. Requires Discord to be running.",
                            self.settings.discord_presence,
                            cx,
                        ))
                        .when(self.settings.discord_presence, |section| {
                            let (details, state) = self.presence_text();
                            section
                                .child(div().h(px(1.)).w_full().bg(cx.theme().border))
                                .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(cx.theme().foreground).child("What to show"))
                                .child(self.render_switch_row(
                                    "discord-device",
                                    "Connected mouse",
                                    "Your mouse's name, e.g. PRO X SUPERLIGHT 2c.",
                                    self.settings.discord_show_device,
                                    cx,
                                ))
                                .child(self.render_switch_row(
                                    "discord-game",
                                    "Current game",
                                    "\"Playing …\" while a game from your library is running.",
                                    self.settings.discord_show_game,
                                    cx,
                                ))
                                .child(self.render_switch_row(
                                    "discord-stats",
                                    "Live stats",
                                    "Your mouse's current DPI and polling rate.",
                                    self.settings.discord_show_stats,
                                    cx,
                                ))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_0p5()
                                        .p_3()
                                        .rounded_md()
                                        .border_1()
                                        .border_color(cx.theme().border)
                                        .child(div().text_xs().text_color(cx.theme().muted_foreground).child("Your status will read"))
                                        .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(cx.theme().foreground).child("OpenMouse Desktop"))
                                        .child(div().text_sm().text_color(cx.theme().foreground).child(details))
                                        .children((!state.is_empty()).then(|| div().text_sm().text_color(cx.theme().foreground).child(state))),
                                )
                        }),
                    )
                    .child(
                        self.render_settings_section(
                            cx,
                            "Devices",
                            "How changes reach your mouse.",
                        )
                        .child(self.render_switch_row(
                            "instant-apply-switch",
                            "Apply changes instantly",
                            "On: every change is written to the mouse as you make it. Off: changes are held until you press Apply Changes in the bar at the bottom.",
                            self.settings.instant_apply,
                            cx,
                        ))
                        .child(div().h(px(1.)).w_full().bg(cx.theme().border))
                        .child(self.render_switch_row(
                            "overlay-switch",
                            "Game notifications",
                            "Show a message on screen when a game with a profile launches or closes. Appears over the game without taking focus.",
                            self.settings.overlay_enabled,
                            cx,
                        ))
                        .when(self.settings.overlay_enabled, |section| {
                            section
                                .child(div().h(px(1.)).w_full().bg(cx.theme().border))
                                .child(self.render_switch_row(
                                    "overlay-sound",
                                    "Play a sound",
                                    "A short, quiet chime with each game notification.",
                                    self.settings.overlay_sound,
                                    cx,
                                ))
                                .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .py_1()
                                    .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(cx.theme().foreground).child("Notification position"))
                                    .child(div().text_xs().text_color(cx.theme().muted_foreground).child(
                                        "Which corner of the screen the message appears in.",
                                    ))
                                    .child({
                                        let mut row = h_flex().gap_2().flex_wrap();
                                        for corner in crate::native_window::OverlayCorner::ALL {
                                            row = row.child(self.chip(
                                                format!("corner-{}", corner.as_str()),
                                                corner.label().to_string(),
                                                self.settings.overlay_corner == corner,
                                                cx,
                                                move |this, cx| {
                                                    this.settings.overlay_corner = corner;
                                                    this.settings.save();
                                                    // Show it where it will appear.
                                                    this.announce("Notifications will appear here.".into(), false, true, cx);
                                                },
                                            ));
                                        }
                                        row
                                    }),
                            )
                        }),
                    ),
            )
            .into_any_element()
    }

    /// One labeled slider row (label + slider + value readout).
    pub(super) fn render_effect_slider(
        &self,
        label: &'static str,
        description: &'static str,
        state: &Option<Entity<SliderState>>,
        value_text: String,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let slider = match state {
            Some(state) => div().child(Slider::new(state)).into_any_element(),
            None => div().into_any_element(),
        };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().foreground)
                            .font_weight(FontWeight::MEDIUM)
                            .child(label),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().foreground)
                            .child(value_text),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(description),
            )
            .child(slider)
            .into_any_element()
    }
}
