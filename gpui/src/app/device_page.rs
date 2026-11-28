//! The page for a single device.

use super::*;

/// The selected device's detail card (battery bar, DPI, polling rate,
/// firmware) under the grid. Hidden until per-device menus exist to hold it.
pub(super) const SHOW_DEVICE_DETAILS: bool = false;

impl AppView {
    /// Per-device settings: sensitivity, polling rate, lift-off distance and
    /// gaming-surface mode, written through the shared protocol drivers.
    /// With instant apply off, edits are staged and the bottom bar applies them.
    pub(super) fn render_device_page(&self, tile: &Tile, view_width: f32, view_height: f32, cx: &mut Context<Self>) -> AnyElement {
        let _ = view_height;
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let busy = self.devices.writing.is_some();
        let staged = &self.devices.staged;
        let instant = self.settings.instant_apply;

        // Quiet one-line summary under the name: brand, link and battery.
        let mut facts = vec![tile.display_brand.clone()];
        if let TileStatus::Connected(r) = &tile.status {
            if let Some(c) = &r.connection {
                facts.push(c.clone());
            }
            if let Some(b) = r.battery {
                facts.push(format!("{b:.0}%{}", if r.charging { " charging" } else { "" }));
            }
        }
        let header = h_flex()
            .gap_3()
            .items_center()
            .child(
                Button::new("device-back")
                    .ghost()
                    .icon(IconName::ArrowLeft)
                    .tooltip("Back to devices")
                    .accessibility_label("Back to devices")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.devices.open = None;
                        this.devices.write_error = None;
                        this.devices.staged = Staged::default();
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(fg)
                            .child(if tile.name.is_empty() { "Unknown device".to_string() } else { tile.name.clone() }),
                    )
                    .child(div().text_sm().text_color(muted).child(facts.join("  ·  "))),
            );

        let section = |title: &str| {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p_4()
                .rounded_lg()
                .border_1()
                .border_color(border)
                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(fg).child(title.to_string()))
        };

        let content: AnyElement = match &tile.status {
            TileStatus::Connected(r) if self.devices.tab != DeviceTab::Performance => self.render_extra_tab(r, cx),
            TileStatus::Connected(r) => {
                let mut col = div().flex().flex_col().gap_3().flex_1().min_w_0().max_w(px(720.));

                // Locked while a game profile is in control of this device.
                if let Some(game) = self.locked_game() {
                    col = col.child(div().text_sm().text_color(muted).child(format!(
                        "Locked while {game} is running — these are your game profile's settings, and they're editable again once the game closes."
                    )));
                }

                // Sensitivity. Hidden when the driver reports no real DPI (a
                // control whose Apply would fail), as the Tauri tab does.
                if let (Some(current), Some(c)) = (r.dpi.filter(|d| *d > 0.0), &r.controls) {
                    let unlinked = r.separate_axes && self.devices.unlinked_axes;
                    let cur_y = r.dpi_y.unwrap_or(current);
                    let x_now = staged.dpi.unwrap_or(current);
                    let y_now = staged.dpi_y.unwrap_or(cur_y);
                    let (min, max, step) = (c.dpi.min, c.dpi.max, c.dpi.step.max(1.0));
                    let mut presets = h_flex().gap_2().flex_wrap();
                    for preset in c.dpi_presets.iter().copied().filter(|p| *p >= min && *p <= max) {
                        presets = presets.child(self.chip(
                            format!("dpi-preset-{preset}"),
                            format!("{preset:.0}"),
                            (x_now - preset).abs() < 0.5,
                            cx,
                            move |this, cx| {
                                let y = this.devices.unlinked_axes.then(|| this.devices.staged.dpi_y.unwrap_or(cur_y));
                                this.edit_dpi(preset, y, cx)
                            },
                        ));
                    }
                    // One stepper per axis: `axis_row(label, value, is_y)`.
                    let axis_row = |label: Option<&str>, value: f64, is_y: bool, cx: &mut Context<Self>| {
                        let (down_id, up_id) = (format!("dpi-down-{is_y}"), format!("dpi-up-{is_y}"));
                        let apply = move |this: &mut Self, delta: f64, cx: &mut Context<Self>| {
                            let x_cur = this.devices.staged.dpi.unwrap_or(current);
                            let y_cur = this.devices.staged.dpi_y.unwrap_or(cur_y);
                            if is_y {
                                this.edit_dpi(x_cur, Some((y_cur + delta).clamp(min, max)), cx);
                            } else {
                                let y = this.devices.unlinked_axes.then_some(y_cur);
                                this.edit_dpi((x_cur + delta).clamp(min, max), y, cx);
                            }
                        };
                        h_flex()
                            .gap_3()
                            .items_center()
                            .children(label.map(|l| div().w(px(14.)).text_sm().text_color(muted).child(l.to_string())))
                            .child(Button::new(SharedString::from(down_id)).compact().label("−").on_click(
                                cx.listener(move |this, _, _, cx| apply(this, -step, cx)),
                            ))
                            .child(
                                div()
                                    .min_w(px(96.))
                                    .text_center()
                                    .text_2xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(fg)
                                    .child(format!("{value:.0}")),
                            )
                            .child(Button::new(SharedString::from(up_id)).compact().label("+").on_click(
                                cx.listener(move |this, _, _, cx| apply(this, step, cx)),
                            ))
                    };
                    let mut card = section("Sensitivity (DPI)").child(axis_row(unlinked.then_some("X"), x_now, false, cx));
                    if unlinked {
                        card = card.child(axis_row(Some("Y"), y_now, true, cx));
                    }
                    card = card.child(presets);
                    if r.separate_axes {
                        card = card.child(
                            h_flex()
                                .gap_3()
                                .items_center()
                                .child(
                                    Switch::new("dpi-link").checked(!unlinked).on_change(cx.listener(move |this, linked: &bool, _, cx| {
                                        if *linked {
                                            // Re-linking puts Y back on X, so what is written is what is shown.
                                            let x = this.devices.staged.dpi.unwrap_or(current);
                                            this.devices.unlinked_axes = false;
                                            this.devices.staged.dpi_y = None;
                                            if (cur_y - x).abs() > 0.5 {
                                                this.edit_dpi(x, None, cx);
                                            }
                                        } else {
                                            this.devices.unlinked_axes = true;
                                        }
                                        cx.notify();
                                    })),
                                )
                                .child(div().text_sm().text_color(fg).child("Link X and Y"))
                                .child(div().text_xs().text_color(muted).child("Move both axes together")),
                        );
                    }
                    let now = if (cur_y - current).abs() > 0.5 { format!("now {current:.0} (Y {cur_y:.0})") } else { format!("now {current:.0}") };
                    col = col.child(card.child(div().text_xs().text_color(muted).child(now)));
                }

                if !r.polling_rates.is_empty() {
                    let shown = staged.polling.or(r.polling_hz.map(|hz| hz as u32));
                    // `pollingReadOnly`: the protocol says this rate can be read but not set.
                    let read_only = r.polling_read_only;
                    let mut row = h_flex().gap_2().flex_wrap();
                    for rate in r.polling_rates.iter().copied() {
                        row = row.child(self.chip(
                            format!("poll-{rate}"),
                            format!("{rate} Hz"),
                            shown == Some(rate),
                            cx,
                            move |this, cx| {
                                if !read_only {
                                    this.edit("pollingRate", serde_json::json!(rate), cx)
                                }
                            },
                        ));
                    }
                    let mut card = section("Polling rate").child(row.when(read_only, |r| r.opacity(0.45)));
                    if read_only {
                        card = card.child(div().text_xs().text_color(muted).child("This mouse reports its polling rate but doesn't let it be changed."));
                    }
                    col = col.child(card);
                }

                let lod_options: Vec<String> =
                    r.lod_options.iter().filter(|o| !(r.hide_lod_low && o.as_str() == "Low")).cloned().collect();
                if !lod_options.is_empty() {
                    let shown = staged.lod.clone().or_else(|| r.lod.clone());
                    // Lift-off only applies while a gaming surface mode is selected.
                    let surface_now = staged.surface.as_deref().or(r.surface.as_deref());
                    let disabled = r.lod_requires_surface && surface_now == Some("Off");
                    let mut row = h_flex().gap_2().flex_wrap();
                    for option in lod_options {
                        let picked = option.clone();
                        row = row.child(self.chip(
                            format!("lod-{option}"),
                            option.clone(),
                            shown.as_deref() == Some(option.as_str()),
                            cx,
                            move |this, cx| {
                                if !disabled {
                                    this.edit("liftOff", serde_json::json!(picked), cx)
                                }
                            },
                        ));
                    }
                    col = col.child(section("Lift-off distance").child(row.when(disabled, |r| r.opacity(0.45))).child(div().text_xs().text_color(muted).child(
                        if disabled {
                            "Available with a gaming surface mode selected — lift-off distance is ignored while surface tracking is off."
                        } else {
                            "How far you can lift the mouse before tracking stops. Higher values keep tracking a little longer."
                        },
                    )));
                }

                if let (Some(current), Some(c)) = (&r.surface, &r.controls) {
                    let shown = staged.surface.clone().unwrap_or_else(|| current.clone());
                    let mut row = h_flex().gap_2().flex_wrap();
                    for mode in c.surface_modes.iter().cloned() {
                        row = row.child(self.chip(
                            format!("surface-{mode}"),
                            mode.clone(),
                            shown == mode,
                            cx,
                            move |this, cx| this.edit("surface", serde_json::json!(mode), cx),
                        ));
                    }
                    col = col.child(section("Gaming surface mode").child(row).child(div().text_xs().text_color(muted).child(
                        "Tunes the sensor for gaming mouse pads. Auto lets the mouse decide; turn it off if tracking misbehaves on a non-gaming surface.",
                    )));
                }

                // Everything else the device reports (sleep, motion sync, DPI stages,
                // debounce, low-power threshold, USB speed, primary button…), from the
                // shared JS with the Tauri tab's own gating. These apply immediately.
                for group in &r.tabs.performance {
                    let mut card = section(&group.title);
                    for control in &group.controls {
                        card = card.child(self.render_control(control, cx));
                    }
                    col = col.child(card);
                }

                if busy && instant {
                    col = col.child(div().text_sm().text_color(muted).child("Applying…"));
                }
                if let Some(error) = &self.devices.write_error {
                    col = col.child(div().text_sm().text_color(cx.theme().danger).child(format!("Could not apply: {error}")));
                }
                col.into_any_element()
            }
            TileStatus::Failed(error) => div()
                .text_sm()
                .text_color(cx.theme().danger)
                .child(format!("Could not read this device: {error}"))
                .into_any_element(),
            _ => div().text_sm().text_color(muted).child("Connecting…").into_any_element(),
        };

        let firmware = match &tile.status {
            TileStatus::Connected(r) if !r.firmware.is_empty() => {
                Some(format!("Firmware {}", r.firmware.join(" · ")))
            }
            _ => None,
        };
        let sidebar = if self.sidebar_collapsed { SIDEBAR_COLLAPSED_W } else { SIDEBAR_W };
        let art_side = ((view_width - sidebar - 2.0 * PAGE_PAD) * 0.36).clamp(260.0, 520.0);

        let mut tab_bar = h_flex().gap_1();
        // Only tabs this device has something for: Buttons if its driver (or a
        // Logitech profile) has remappable buttons, Lighting if it reports
        // lighting, Advanced if its brand has extras.
        let (has_buttons, has_lighting, has_advanced, has_profiles) = match &tile.status {
            TileStatus::Connected(r) => (
                !r.tabs.buttons.is_empty() || r.onboard_profiles,
                r.tabs.lighting.is_some(),
                !r.tabs.advanced.is_empty(),
                r.onboard_profiles,
            ),
            _ => (false, false, false, false),
        };
        let tabs: Vec<DeviceTab> = [
            (DeviceTab::Performance, true),
            (DeviceTab::Buttons, has_buttons),
            (DeviceTab::Lighting, has_lighting),
            (DeviceTab::Advanced, has_advanced),
            (DeviceTab::Profiles, has_profiles),
        ]
        .into_iter()
        .filter_map(|(tab, shown)| shown.then_some(tab))
        .collect();
        for tab in tabs {
            tab_bar = tab_bar.child(self.chip(
                format!("tab-{}", tab.label()),
                tab.label().to_string(),
                self.devices.tab == tab,
                cx,
                move |this, cx| {
                    this.devices.tab = tab;
                    // Logitech's stored profiles back both tabs; read them once.
                    if has_profiles
                        && matches!(tab, DeviceTab::Profiles | DeviceTab::Buttons)
                        && this.devices.onboard.is_none()
                    {
                        this.load_onboard_profiles(cx);
                    }
                    cx.notify();
                },
            ));
        }

        let show_bar = !instant && !staged.is_empty();
        let apply_bar = div()
            .absolute()
            .left(px(PAGE_PAD))
            .right(px(PAGE_PAD))
            .bottom(px(PAGE_PAD))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap_3()
            .px_4()
            .py_3()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().primary)
            .bg(cx.theme().sidebar)
            .shadow_lg()
            .child(div().text_sm().text_color(fg).child("You have unsaved changes"))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("revert")
                            .ghost()
                            .compact()
                            .label("Revert")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.devices.staged = Staged::default();
                                this.devices.write_error = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("apply-changes")
                            .compact()
                            .label(if busy { "Applying…" } else { "Apply Changes" })
                            .on_click(cx.listener(|this, _, _, cx| this.apply_staged(cx))),
                    ),
            );

        div()
            .relative()
            .size_full()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .size_full()
                    .p_4()
                    .gap_4()
                    .when(show_bar, |this| this.pb(px(96.)))
                    .overflow_y_scrollbar()
                    .child(header)
                    .child(
                        h_flex()
                            .gap_6()
                            .items_start()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .gap_2()
                                    .flex_none()
                                    .child(self.device_art(tile, art_side, 1.4))
                                    .when_some(firmware, |this, fw| {
                                        this.child(div().text_xs().text_color(muted).child(fw))
                                    }),
                            )
                            // Tabs sit directly over the controls they switch.
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_3()
                                    .flex_1()
                                    .min_w_0()
                                    .max_w(px(720.))
                                    .child(tab_bar)
                                    .child(content),
                            ),
                    ),
            )
            .when(show_bar, |this| this.child(apply_bar))
            .into_any_element()
    }
}
