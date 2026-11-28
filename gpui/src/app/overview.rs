//! The Overview page.

use super::*;

/// Overview grid sizing: the smallest card width a column may shrink to, and
/// the layout constants the column count is estimated from.
pub(super) const MIN_CARD_W: f32 = 320.0;

pub(super) const GRID_GAP: f32 = 16.0;

impl AppView {
    /// Overview: the connected mouse and its artwork, read through the shared
    /// protocol drivers (`crate::hid`).
    pub(super) fn render_overview(&self, view_width: f32, view_height: f32, cx: &mut Context<Self>) -> AnyElement {
        if let Some(tile) = self
            .devices
            .open
            .as_ref()
            .and_then(|key| self.devices.tiles.iter().find(|t| &t.key == key))
        {
            return self.render_device_page(tile, view_width, view_height, cx);
        }
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let devices = &self.devices;

        let header = h_flex()
            .justify_between()
            .items_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(fg)
                            .child("Overview"),
                    )
                    .child(div().text_sm().text_color(muted).child(match devices.tiles.len() {
                        0 => "No mouse connected".to_string(),
                        1 => "1 supported device found".to_string(),
                        n => format!("{n} supported devices found"),
                    })),
            );

        let body: AnyElement = if let Some(tile) = devices.tiles.get(devices.selected) {
            let mut stats = div().flex().flex_row().flex_wrap().gap_2();
            let stat = |label: &str, value: String| {
                div()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .min_w(px(110.))
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    .child(div().text_xs().text_color(muted).child(label.to_string()))
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(fg)
                            .child(value),
                    )
            };
            let mut tags = h_flex().gap_1().child(Tag::secondary().small().child(tile.display_brand.clone()));
            let mut details = div().flex().flex_col().gap_3().flex_1().min_w_0();
            match &tile.status {
                TileStatus::Connected(r) => {
                    if let Some(c) = &r.connection {
                        tags = tags.child(accent_badge(c.clone(), cx.theme().primary, cx.theme().foreground));
                    }
                    if let Some(b) = r.battery {
                        let fill = (b / 100.0).clamp(0.0, 1.0) as f32;
                        details = details.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div().text_sm().text_color(muted).child(format!(
                                        "Battery {b:.0}%{}",
                                        if r.charging { " · charging" } else { "" }
                                    )),
                                )
                                .child(
                                    div()
                                        .h(px(8.))
                                        .w_full()
                                        .max_w(px(320.))
                                        .rounded_full()
                                        .bg(Hsla { a: 0.25, ..cx.theme().muted_foreground })
                                        .child(
                                            div()
                                                .h_full()
                                                .w(relative(fill))
                                                .rounded_full()
                                                .bg(cx.theme().primary),
                                        ),
                                ),
                        );
                    }
                    if let Some(d) = r.dpi {
                        stats = stats.child(stat("DPI", format!("{d:.0}")));
                    }
                    // A fixed-rate pipeline (Starlight X) has no rate to show, only its name.
                    if r.polling_read_only && r.polling_rates.is_empty() {
                        stats = stats.child(stat("Polling rate", "Perfect Polling".to_string()));
                    } else if let Some(hz) = r.polling_hz {
                        stats = stats.child(stat("Polling rate", format!("{hz:.0} Hz")));
                    }
                    if !r.firmware.is_empty() {
                        stats = stats.child(stat("Firmware", r.firmware.join(" · ")));
                    }
                    details = details.child(stats);
                }
                TileStatus::Connecting => {
                    details = details.child(div().text_sm().text_color(muted).child("Connecting…"));
                }
                TileStatus::Failed(error) => {
                    details = details
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().danger)
                                .child(format!("Could not read this device: {error}")),
                        )
                        .child({
                            let key = tile.key.clone();
                            Button::new("retry")
                                .compact()
                                .label("Try again")
                                .on_click(cx.listener(move |this, _, _, cx| this.connect_tile(&key, false, cx)))
                        });
                }
                TileStatus::Idle => {
                    details = details.child({
                        let key = tile.key.clone();
                        Button::new("connect")
                            .compact()
                            .label("Connect")
                            .on_click(cx.listener(move |this, _, _, cx| this.connect_tile(&key, false, cx)))
                    });
                }
            }
            let hero = h_flex()
                .gap_5()
                .items_start()
                .p_4()
                .rounded_xl()
                .border_1()
                .border_color(border)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xl()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(fg)
                                        .child(if tile.name.is_empty() {
                                            "Unknown device".to_string()
                                        } else {
                                            tile.name.clone()
                                        }),
                                )
                                .child(tags),
                        )
                        .child(details),
                );

            // Large grid, one card per device: name and status top-left,
            // artwork centred below.
            // Columns: as many as fit at a comfortably large card width, but
            // never more than there are devices — two devices split the page in
            // two, one takes it all. The page padding and sidebar are estimated;
            // a small error only shifts where the column count flips.
            let sidebar = if self.sidebar_collapsed { SIDEBAR_COLLAPSED_W } else { SIDEBAR_W };
            let avail = (view_width - sidebar - 2.0 * PAGE_PAD).max(MIN_CARD_W);
            let fit = (((avail + GRID_GAP) / (MIN_CARD_W + GRID_GAP)).floor() as usize).clamp(1, 6);
            let cols = fit.min(devices.tiles.len()).max(1) as u16;
            let card_w = (avail - GRID_GAP * (cols as f32 - 1.0)) / cols as f32;
            // The art is as big as the card is wide, but must leave room for the
            // title bar, header, name/status lines and padding on a short window.
            let art_side = (card_w - 66.0).min(view_height - 276.0).max(120.0);
            let mut grid = div().grid().grid_cols(cols).gap_4().w_full();
            for (i, t) in devices.tiles.iter().enumerate() {
                let key = t.key.clone();
                let active = i == devices.selected;
                let (status_line, status_color) = match &t.status {
                    TileStatus::Idle => ("Click to connect".to_string(), muted),
                    TileStatus::Connecting => ("Connecting…".to_string(), muted),
                    // A wireless mouse that is off or asleep answers nothing; say what to do.
                    TileStatus::Failed(error) if error.to_lowercase().contains("asleep") || error.to_lowercase().contains("out of range") => {
                        ("Asleep or out of range — wake it, then click to retry".to_string(), cx.theme().danger)
                    }
                    TileStatus::Failed(_) => ("Couldn't connect — click to retry".to_string(), cx.theme().danger),
                    TileStatus::Connected(r) => {
                        let mut parts = Vec::new();
                        if let Some(b) = r.battery {
                            parts.push(format!("{b:.0}%{}", if r.charging { " ⚡" } else { "" }));
                        }
                        if let Some(c) = &r.connection {
                            parts.push(c.clone());
                        }
                        if parts.is_empty() {
                            parts.push("Connected".to_string());
                        }
                        (parts.join("  ·  "), muted)
                    }
                };
                grid = grid.child(
                    div()
                        .id(SharedString::from(format!("tile-{}", t.key)))
                        .cursor_pointer()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .p_5()
                        .rounded_lg()
                        .border_1()
                        .border_color(if active { cx.theme().primary } else { Hsla { a: 0.0, ..border } })
                        // A faint lift off the page, derived from the text colour so
                        // it is a surface in every theme (`muted` is a text tone
                        // in the Matt palettes).
                        .bg(Hsla { a: 0.06, ..fg })
                        .hover(|this| this.bg(Hsla { a: 0.10, ..fg }))
                        .child(
                            div()
                                .text_xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(fg)
                                .truncate()
                                .child(if t.name.is_empty() {
                                    "Unknown device".to_string()
                                } else {
                                    t.name.clone()
                                }),
                        )
                        .child(div().text_sm().text_color(status_color).child(status_line))
                        .child(div().w_full().p_3().flex().justify_center().child(self.device_art(t, art_side, 1.3)))
                        .on_click(cx.listener(move |this, _, _, cx| this.open_device(&key, cx))),
                );
            }
            let col = div()
                .flex()
                .flex_col()
                .gap_4()
                .child(grid)
                .when(SHOW_DEVICE_DETAILS, |col| col.child(hero));
            col.into_any_element()
        } else {
            let message = match (&devices.error, devices.scanning) {
                (Some(error), _) => format!("Could not scan for devices: {error}"),
                (None, true) => "Looking for mice…".to_string(),
                (None, false) => "No supported mouse found. Plug one in and it will appear here.".to_string(),
            };
            div()
                .p_6()
                .rounded_xl()
                .border_1()
                .border_color(border)
                .text_sm()
                .text_color(muted)
                .child(message)
                .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .gap_4()
            .overflow_y_scrollbar()
            .child(header)
            .child(body)
            .into_any_element()
    }
}
