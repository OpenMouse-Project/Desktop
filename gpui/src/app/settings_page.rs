//! The Settings page.
//!
//! Each section is a heading over its own card, and each setting in a card is a
//! row: icon, title and description, then the control.

use super::*;

/// Lines content up with a row's text: the icon tile plus the gap after it.
const ROW_INDENT: f32 = 44.0;

/// An icon in its tile; section headings use the accent colour.
fn settings_icon(icon: IconName, heading: bool, cx: &App) -> Div {
    let (tile, glyph) = if heading { (36.0, 18.0) } else { (32.0, 16.0) };
    let (ink, fill) = if heading {
        (cx.theme().primary, Hsla { a: 0.14, ..cx.theme().primary })
    } else {
        (cx.theme().muted_foreground, Hsla { a: 0.07, ..cx.theme().foreground })
    };
    div()
        .flex_none()
        .size(px(tile))
        .flex()
        .items_center()
        .justify_center()
        .rounded(cx.theme().radius)
        .bg(fill)
        .child(Icon::new(icon).size(px(glyph)).text_color(ink))
}

/// The bordered surface that holds one section's settings.
fn settings_card(cx: &App) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .p_4()
        .rounded_lg()
        .border_1()
        .border_color(cx.theme().border)
        .bg(Hsla { a: 0.45, ..cx.theme().sidebar })
}

/// A heading above its card, so where one section ends is plain to see.
fn settings_section(icon: IconName, title: &'static str, description: &'static str, card: Div, cx: &App) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            h_flex()
                .gap_3()
                .items_center()
                .child(settings_icon(icon, true, cx))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(div().text_base().font_weight(FontWeight::SEMIBOLD).child(title))
                        .child(div().text_xs().text_color(cx.theme().muted_foreground).child(description)),
                ),
        )
        .child(card)
}

/// Icon, title and description, with the control (if any) on the right.
fn settings_row(icon: IconName, title: &'static str, description: &'static str, cx: &App) -> Div {
    h_flex()
        .items_center()
        .gap_3()
        .py_1()
        .child(settings_icon(icon, false, cx))
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
                .child(div().text_xs().text_color(cx.theme().muted_foreground).child(description)),
        )
}

fn settings_divider(cx: &App) -> Div {
    div().h(px(1.)).w_full().bg(cx.theme().border)
}

impl AppView {
    /// A [`settings_row`] with a [`Switch`]. `switch_id` selects which setting
    /// it drives.
    fn render_switch_row(
        &self,
        switch_id: &'static str,
        icon: IconName,
        title: &'static str,
        description: &'static str,
        checked: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let error = match switch_id {
            "startup-switch" => self.startup_error.clone(),
            "discord-switch" => self.discord_error.clone(),
            _ => None,
        };
        let switch = Switch::new(switch_id)
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
            }));
        let row = settings_row(icon, title, description, cx).child(switch);
        div()
            .flex()
            .flex_col()
            .child(row)
            .children(error.map(|error| div().pl(px(ROW_INDENT)).text_xs().text_color(cx.theme().danger).child(error)))
            .into_any_element()
    }

    /// Theme, accent and backdrop. These keep their entities and save at once.
    fn render_appearance_card(&self, cx: &mut Context<Self>) -> Div {
        let selected = THEME_CHOICES
            .iter()
            .position(|choice| *choice == self.settings.theme);
        let show_vibrancy = self.settings.theme == ThemeChoice::Dynamic;
        let themes = RadioGroup::horizontal("theme-choice")
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
            }));
        let reset = self.settings.accent.is_some().then(|| {
            Button::new("accent-reset")
                .compact()
                .label("Reset")
                .on_click(cx.listener(|this, _, window, cx| this.set_accent(None, window, cx)))
        });
        let blur = Switch::new("background-blur")
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
            }));
        let accents = self.render_accent_picker(cx);
        settings_card(cx)
            .child(settings_row(IconName::SunMoon, "Theme", "How the app looks.", cx))
            .child(themes)
            .child(settings_divider(cx))
            .child(
                settings_row(
                    IconName::Pipette,
                    "Accent color",
                    "Override the theme's accent, or reset to use the theme's own.",
                    cx,
                )
                .children(reset),
            )
            .child(div().pl(px(ROW_INDENT)).child(accents))
            .child(settings_divider(cx))
            .child(self.render_effect_slider(
                IconName::Droplets,
                "Background opacity",
                "Lower the tint to reveal more of the desktop behind the window.",
                &self.opacity_slider,
                format!("{:.0}%", self.settings.window_opacity),
                cx,
            ))
            .child(settings_divider(cx))
            .child(
                settings_row(
                    IconName::Blend,
                    "Background blur",
                    "Soften the desktop behind the window. The system controls blur strength.",
                    cx,
                )
                .child(blur),
            )
            .when(show_vibrancy, |card| {
                card.child(settings_divider(cx)).child(self.render_effect_slider(
                    IconName::Sparkles,
                    "Vibrancy",
                    "Adjust the intensity of your wallpaper-derived colors.",
                    &self.vibrancy_slider,
                    format!("{:.0}%", self.settings.dynamic_vibrancy),
                    cx,
                ))
            })
    }

    fn render_general_card(&self, cx: &mut Context<Self>) -> Div {
        let mut card = settings_card(cx)
            .child(self.render_switch_row(
                "startup-switch",
                IconName::Power,
                "Launch on startup",
                "Open OpenMouse automatically when you log in.",
                self.settings.launch_on_startup,
                cx,
            ))
            .child(settings_divider(cx))
            .child(self.render_switch_row(
                "discord-switch",
                IconName::MessageCircle,
                "Discord Rich Presence",
                "Show OpenMouse in your Discord status. Requires Discord to be running.",
                self.settings.discord_presence,
                cx,
            ));
        if !self.settings.discord_presence {
            return card;
        }
        // What the status shows, indented under the switch it belongs to.
        let (details, state) = self.presence_text();
        let options = div()
            .flex()
            .flex_col()
            .gap_3()
            .pl(px(ROW_INDENT))
            .child(self.render_switch_row(
                "discord-device",
                IconName::Mouse,
                "Connected mouse",
                "Your mouse's name, e.g. PRO X SUPERLIGHT 2c.",
                self.settings.discord_show_device,
                cx,
            ))
            .child(self.render_switch_row(
                "discord-game",
                IconName::Gamepad2,
                "Current game",
                "\"Playing …\" while a game from your library is running.",
                self.settings.discord_show_game,
                cx,
            ))
            .child(self.render_switch_row(
                "discord-stats",
                IconName::Gauge,
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
            );
        card = card.child(options);
        card
    }

    fn render_devices_card(&self, cx: &mut Context<Self>) -> Div {
        settings_card(cx).child(self.render_switch_row(
            "instant-apply-switch",
            IconName::Zap,
            "Apply changes instantly",
            "On: every change is written to the mouse as you make it. Off: changes are held until you press Apply Changes in the bar at the bottom.",
            self.settings.instant_apply,
            cx,
        ))
    }

    fn render_notifications_card(&self, cx: &mut Context<Self>) -> Div {
        let card = settings_card(cx).child(self.render_switch_row(
            "overlay-switch",
            IconName::Bell,
            "Game notifications",
            "Show a message on screen when a game with a profile launches or closes. Appears over the game without taking focus.",
            self.settings.overlay_enabled,
            cx,
        ));
        if !self.settings.overlay_enabled {
            return card;
        }
        let mut corners = h_flex().gap_2().flex_wrap();
        for corner in crate::native_window::OverlayCorner::ALL {
            corners = corners.child(self.chip(
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
        card.child(settings_divider(cx))
            .child(self.render_switch_row(
                "overlay-sound",
                IconName::Volume2,
                "Play a sound",
                "A short, quiet chime with each game notification.",
                self.settings.overlay_sound,
                cx,
            ))
            .child(settings_divider(cx))
            .child(settings_row(
                IconName::PictureInPicture2,
                "Notification position",
                "Which corner of the screen the message appears in.",
                cx,
            ))
            .child(div().pl(px(ROW_INDENT)).child(corners))
    }

    /// The logs folder, with a button that opens it.
    fn render_troubleshooting_card(&self, cx: &mut Context<Self>) -> Div {
        let open = Button::new("open-logs").compact().label("Open folder").on_click(cx.listener(
            |this, _, _, cx| match crate::logging::dir().filter(|dir| std::fs::create_dir_all(dir).is_ok()) {
                Some(dir) => cx.open_with_system(&dir),
                None => this.show_toast("Couldn't open the logs folder.", true, cx),
            },
        ));
        settings_card(cx).child(
            settings_row(
                IconName::FolderOpen,
                "Logs and crash reports",
                "Saved only on this computer. Nothing is sent.",
                cx,
            )
            .child(open),
        )
    }

    pub(super) fn render_settings(&self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let appearance = self.render_appearance_card(cx);
        let general = self.render_general_card(cx);
        let devices = self.render_devices_card(cx);
        let notifications = self.render_notifications_card(cx);
        let troubleshooting = self.render_troubleshooting_card(cx);
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
                    .gap_8()
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
                    .child(settings_section(
                        IconName::Palette,
                        "Appearance",
                        "Choose a theme and adjust the window backdrop.",
                        appearance,
                        cx,
                    ))
                    .child(settings_section(
                        IconName::SlidersHorizontal,
                        "General",
                        "Startup and integration preferences.",
                        general,
                        cx,
                    ))
                    .child(settings_section(
                        IconName::Mouse,
                        "Devices",
                        "How changes reach your mouse.",
                        devices,
                        cx,
                    ))
                    .child(settings_section(
                        IconName::Bell,
                        "Notifications",
                        "Messages shown while you play.",
                        notifications,
                        cx,
                    ))
                    .child(settings_section(
                        IconName::Wrench,
                        "Troubleshooting",
                        "Help with finding what went wrong.",
                        troubleshooting,
                        cx,
                    )),
            )
            .into_any_element()
    }

    /// A slider setting: its row shows the value, the slider sits underneath.
    fn render_effect_slider(
        &self,
        icon: IconName,
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
        let value = div().text_sm().text_color(cx.theme().foreground).child(value_text);
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(settings_row(icon, label, description, cx).child(value))
            .child(div().pl(px(ROW_INDENT)).child(slider))
            .into_any_element()
    }
}
