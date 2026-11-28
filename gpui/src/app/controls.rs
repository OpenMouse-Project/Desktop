//! Device controls: sliders, lighting, buttons and onboard profiles.

use super::*;

/// Swatches offered for lighting colours.
pub(super) const LIGHT_SWATCHES: [&str; 14] = [
    "#ff0000", "#ff6a00", "#ffc800", "#a6ff00", "#00ff40", "#00ffd0", "#00aaff",
    "#0040ff", "#6a00ff", "#c800ff", "#ff00a0", "#ffffff", "#9aa0a6", "#000000",
];

pub(super) fn hex_color(hex: &str) -> Hsla {
    let h = hex.trim_start_matches('#');
    let v = u32::from_str_radix(h, 16).unwrap_or(0x888888);
    rgb(v).into()
}

impl AppView {
    /// Creates the sliders the selected device's number controls need. The write
    /// waits until the drag pauses, so a sweep sends one command, not hundreds.
    pub(super) fn ensure_control_sliders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tile) = self.devices.tiles.get(self.devices.selected) else { return };
        let TileStatus::Connected(r) = &tile.status else { return };
        let device = tile.key.clone();
        let specs: Vec<crate::hid::ControlSpec> = r
            .tabs
            .performance
            .iter()
            .chain(&r.tabs.buttons)
            .chain(&r.tabs.advanced)
            .flat_map(|s| s.controls.iter())
            .filter(|c| c.kind == "number" || c.kind == "stepped")
            .cloned()
            .collect();
        for c in specs {
            let key = format!("{device}/{}", c.id);
            if self.control_sliders.contains_key(&key) {
                continue;
            }
            // A stepped control slides over the positions of its option list
            // (the stops need not be evenly spaced); the write sends the
            // option's own value.
            let stepped: Option<Vec<serde_json::Value>> =
                (c.kind == "stepped").then(|| c.options.iter().map(|o| o.value.clone()).collect());
            let (min, max) = match &stepped {
                Some(values) => (0.0, values.len().saturating_sub(1) as f64),
                None => (c.min.unwrap_or(0.0), c.max.unwrap_or(255.0)),
            };
            let step = if stepped.is_some() { 1.0 } else { c.step.unwrap_or(1.0) };
            let start = match &stepped {
                Some(values) => values.iter().position(|v| *v == c.current).unwrap_or(0) as f64,
                None => c.current.as_f64().unwrap_or(min).clamp(min, max),
            };
            let slider = cx.new(|_| SliderState::new().min(0.0).max((max - min) as f32).step(step as f32).default_value((start - min) as f32));
            let id = c.id.clone();
            let places = c.decimals.unwrap_or(0);
            let live_key = key.clone();
            let sub = cx.subscribe_in(&slider, window, move |this: &mut Self, _s, event, _w, cx| {
                let SliderEvent::Change(value) = event else { return };
                let scale = 10f64.powi(places as i32);
                let v = ((value.end() as f64 + min) * scale).round() / scale;
                this.slider_live.insert(live_key.clone(), v);
                this.slider_seq += 1;
                let seq = this.slider_seq;
                let id = id.clone();
                let stepped = stepped.clone();
                cx.notify();
                cx.spawn(async move |view, cx| {
                    cx.background_executor().timer(std::time::Duration::from_millis(350)).await;
                    let _ = view.update(cx, |this: &mut Self, cx| {
                        if this.slider_seq == seq {
                            let sent = match &stepped {
                                Some(values) => values.get(v as usize).cloned().unwrap_or(serde_json::Value::Null),
                                None => serde_json::json!(v),
                            };
                            this.set_control(id, sent, cx);
                        }
                    });
                })
                .detach();
            });
            self._subscriptions.push(sub);
            self.control_sliders.insert(key, slider);
        }
    }

    /// One Buttons/Advanced control: label on the left, the control on the right.
    pub(super) fn render_control(&self, c: &crate::hid::ControlSpec, cx: &mut Context<Self>) -> AnyElement {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let id = c.id.clone();
        let label_block = div()
            .flex()
            .flex_col()
            .gap_0p5()
            .flex_1()
            .min_w_0()
            .child(div().text_sm().text_color(fg).child(c.label.clone()))
            .children(c.description.clone().map(|d| div().text_xs().text_color(muted).child(d)));

        let mut unfolded: Option<AnyElement> = None;
        let control: AnyElement = match c.kind.as_str() {
            "toggle" => {
                let on = c.current.as_bool().unwrap_or(false);
                Switch::new(SharedString::from(format!("sw-{id}")))
                    .checked(on)
                    .on_change(cx.listener(move |this, enabled: &bool, _, cx| {
                        this.set_control(id.clone(), serde_json::json!(*enabled), cx)
                    }))
                    .into_any_element()
            }
            "number" => {
                let places = c.decimals.unwrap_or(0);
                let unit = c.unit.as_deref().map(|u| format!(" {u}")).unwrap_or_default();
                let slider_key = self.devices.tiles.get(self.devices.selected).map(|t| format!("{}/{}", t.key, c.id));
                let live = slider_key.as_ref().and_then(|k| self.slider_live.get(k)).copied();
                let now = live.or(c.current.as_f64()).unwrap_or(c.min.unwrap_or(0.0));
                let slider = slider_key.and_then(|k| self.control_sliders.get(&k).cloned());
                let shown = match (&c.zero_label, now <= c.min.unwrap_or(0.0)) {
                    (Some(label), true) => label.clone(),
                    _ => format!("{now:.places$}{unit}"),
                };
                Self::control_slider_row(slider, shown, cx)
            }
            "stepped" => {
                let slider_key = self.devices.tiles.get(self.devices.selected).map(|t| format!("{}/{}", t.key, c.id));
                let live = slider_key.as_ref().and_then(|k| self.slider_live.get(k)).copied();
                let index = live
                    .map(|v| v as usize)
                    .or_else(|| c.options.iter().position(|o| o.value == c.current))
                    .unwrap_or(0);
                let slider = slider_key.and_then(|k| self.control_sliders.get(&k).cloned());
                Self::control_slider_row(slider, c.options.get(index).map(|o| o.label.clone()).unwrap_or_default(), cx)
            }
            "choice" if c.options.len() <= 8 => {
                let mut row = h_flex().gap_2().flex_wrap().justify_end();
                for option in &c.options {
                    let value = option.value.clone();
                    let sent = value.clone();
                    let cid = id.clone();
                    row = row.child(self.chip(
                        format!("{id}-{}", option.label),
                        option.label.clone(),
                        c.current == value,
                        cx,
                        move |this, cx| this.set_control(cid.clone(), sent.clone(), cx),
                    ));
                }
                row.into_any_element()
            }
            // Long lists (18 Logitech button actions, nine on Endgame Gear): a
            // button showing the current value that unfolds the options below.
            "choice" => {
                let open = self.devices.expanded.as_deref() == Some(id.as_str());
                let shown = c
                    .options
                    .iter()
                    .find(|o| o.value == c.current)
                    .map(|o| o.label.clone())
                    .unwrap_or_else(|| "Custom".to_string());
                if open {
                    let mut grid = h_flex().gap_2().flex_wrap().justify_end();
                    for option in &c.options {
                        let (cid, sent) = (id.clone(), option.value.clone());
                        grid = grid.child(self.chip(
                            format!("{id}-{}", option.label),
                            option.label.clone(),
                            c.current == option.value,
                            cx,
                            move |this, cx| {
                                this.devices.expanded = None;
                                this.set_control(cid.clone(), sent.clone(), cx)
                            },
                        ));
                    }
                    unfolded = Some(grid.into_any_element());
                }
                let toggle_id = id.clone();
                Button::new(SharedString::from(format!("choose-{id}")))
                    .compact()
                    .label(format!("{shown}  {}", if open { "▴" } else { "▾" }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.devices.expanded =
                            if this.devices.expanded.as_deref() == Some(toggle_id.as_str()) { None } else { Some(toggle_id.clone()) };
                        cx.notify();
                    }))
                    .into_any_element()
            }
            _ => div()
                .text_sm()
                .text_color(muted)
                .child(c.current.as_str().map(str::to_string).unwrap_or_else(|| c.current.to_string()))
                .into_any_element(),
        };
        let row = h_flex().gap_4().items_center().justify_between().child(label_block).child(control);
        match unfolded {
            Some(options) => div().flex().flex_col().gap_2().child(row).child(options).into_any_element(),
            None => row.into_any_element(),
        }
    }

    /// Logitech button remapping: the buttons of the profile the mouse is
    /// running, each assignable to any action the protocol knows. Changes are
    /// written to that profile in the mouse's own memory (the library verifies
    /// the write and restores the old sector if it fails), so the mouse has to
    /// be running its stored profiles, i.e. in Onboard mode.
    pub(super) fn render_logitech_buttons(&self, r: &DeviceReadout, cx: &mut Context<Self>) -> AnyElement {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let onboard_mode = r.mode.as_deref() == Some("Onboard");
        let mut col = div().flex().flex_col().gap_3().flex_1().min_w_0().max_w(px(720.));
        let note = |text: &str| div().text_sm().text_color(muted).child(text.to_string());

        if !onboard_mode {
            return col
                .child(note(
                    "Button changes are saved into the mouse's stored profile, which only runs in Onboard memory mode. The mouse is currently under software control.",
                ))
                .child(
                    Button::new("buttons-onboard")
                        .label("Switch to Onboard memory mode")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.onboard_action("mode", serde_json::json!("Onboard"), cx)
                        })),
                )
                .into_any_element();
        }
        let Some(read) = &self.devices.onboard else {
            return col
                .child(note(if self.devices.onboard_loading {
                    "Reading the mouse's buttons…"
                } else {
                    "Buttons haven't been read yet."
                }))
                .into_any_element();
        };
        let Some(running) = read.profiles.iter().find(|p| p.is_current) else {
            return col.child(note("No stored profile is running.")).into_any_element();
        };

        const NAMES: [&str; 5] = ["Left button", "Right button", "Middle button", "Back", "Forward"];
        let options: Vec<crate::hid::OptionSpec> = read
            .actions
            .iter()
            .map(|a| crate::hid::OptionSpec { value: serde_json::json!(a), label: a.clone() })
            .collect();
        let mut card = div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(border)
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(fg)
                    .child(format!("Profile {} — buttons", running.sector)),
            );
        for (n, assignment) in running.button_assignments.iter().enumerate() {
            let label = NAMES.get(n).map(|s| s.to_string()).unwrap_or_else(|| format!("Button {}", n + 1));
            // A macro or custom binding the protocol can't name shows as such.
            let current = assignment.get("action").cloned().unwrap_or(serde_json::Value::Null);
            let spec = crate::hid::ControlSpec {
                id: format!("logi.button.primary.{n}"),
                label,
                description: None,
                kind: "choice".to_string(),
                options: options.clone(),
                min: None,
                max: None,
                step: None,
                decimals: None,
                unit: None,
                zero_label: None,
                current,
            };
            card = card.child(self.render_control(&spec, cx));
        }
        col = col.child(card).child(note(
            "Saved to the mouse's memory. Choose a different action for a button to remap it.",
        ));
        if self.devices.writing.is_some() {
            col = col.child(note("Saving…"));
        }
        if let Some(error) = &self.devices.write_error {
            col = col.child(div().text_sm().text_color(cx.theme().danger).child(format!("Could not save: {error}")));
        }
        col.into_any_element()
    }

    /// The mouse's own stored profiles (Logitech): which one runs, and whether
    /// the mouse runs them at all. Read-only apart from switching — editing a
    /// profile writes flash, and the library flags that sequence as unproven.
    pub(super) fn render_onboard_profiles(&self, r: &DeviceReadout, cx: &mut Context<Self>) -> AnyElement {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let accent = cx.theme().primary;
        let ink = cx.theme().foreground;
        let onboard_mode = r.mode.as_deref() == Some("Onboard");
        let busy = self.devices.writing.is_some();
        let card = || {
            div().flex().flex_col().gap_2().p_4().rounded_lg().border_1().border_color(border)
        };
        let mut col = div().flex().flex_col().gap_3().flex_1().min_w_0().max_w(px(720.));

        col = col.child(
            card().child(
                h_flex()
                    .gap_4()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(fg).child("Onboard memory mode"))
                            .child(div().text_xs().text_color(muted).child(if onboard_mode {
                                "The mouse is running its own stored profile."
                            } else {
                                "Software is controlling the mouse, so its stored profiles are bypassed. Changing DPI or polling rate in this app puts the mouse in this mode."
                            })),
                    )
                    .child(
                        Switch::new("onboard-mode")
                            .checked(onboard_mode)
                            .on_change(cx.listener(|this, on: &bool, _, cx| {
                                this.onboard_action("mode", serde_json::json!(if *on { "Onboard" } else { "Host" }), cx)
                            })),
                    ),
            ),
        );

        match &self.devices.onboard {
            None => {
                col = col.child(div().text_sm().text_color(muted).child(if self.devices.onboard_loading {
                    "Reading the mouse's profiles…"
                } else {
                    "Profiles haven't been read yet."
                }));
            }
            Some(list) => {
                let mut rows = div().flex().flex_col().gap_2();
                for p in &list.profiles {
                    let sector = p.sector;
                    let title = p.name.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| format!("Profile {}", p.sector));
                    let running = p.is_current && onboard_mode;
                    let dpis: Vec<String> = p
                        .dpi_stages
                        .iter()
                        .filter_map(|s| s.get("x").and_then(|v| v.as_f64()))
                        .filter(|d| *d > 0.0)
                        .map(|d| format!("{d:.0}"))
                        .collect();
                    let mut facts = Vec::new();
                    if !dpis.is_empty() {
                        facts.push(format!("DPI {}", dpis.join(" · ")));
                    }
                    if let Some(hz) = p.report_rate_wireless.or(p.report_rate_wired) {
                        facts.push(format!("{hz} Hz"));
                    }
                    if !p.button_assignments.is_empty() {
                        facts.push(format!("{} buttons", p.button_assignments.len()));
                    }
                    if !p.crc_valid {
                        facts.push("checksum mismatch".to_string());
                    }
                    rows = rows.child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .justify_between()
                            .p_3()
                            .rounded_md()
                            .border_1()
                            .border_color(if running { cx.theme().primary } else { border })
                            .when(running, |this| this.bg(Hsla { a: 0.10, ..cx.theme().primary }))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_0p5()
                                    .min_w_0()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(fg).child(title))
                                            .when(running, |row| row.child(accent_badge("Running", accent, ink)))
                                            .when(!p.enabled, |row| row.child(Tag::secondary().small().child("Disabled"))),
                                    )
                                    .child(div().text_xs().text_color(muted).child(facts.join("  ·  "))),
                            )
                            .when(p.enabled && !running, |row| {
                                row.child(
                                    Button::new(SharedString::from(format!("use-profile-{sector}")))
                                        .compact()
                                        .label(if busy { "Switching…" } else { "Use" })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.onboard_action("useProfile", serde_json::json!(sector), cx)
                                        })),
                                )
                            }),
                    );
                }
                col = col.child(card().child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(fg).child("Stored profiles")).child(rows));
                col = col.child(
                    div().text_xs().text_color(muted).child(
                        "Editing a stored profile, or enabling and disabling slots, rewrites the mouse's flash memory. That isn't offered here yet.",
                    ),
                );
            }
        }
        if let Some(error) = &self.devices.write_error {
            col = col.child(div().text_sm().text_color(cx.theme().danger).child(error.clone()));
        }
        col.into_any_element()
    }

    /// The Buttons, Lighting and Advanced tabs. Controls come from the shared
    /// JS (`js/controls.ts`), so any brand's extras appear without Rust knowing them.
    pub(super) fn render_extra_tab(&self, r: &DeviceReadout, cx: &mut Context<Self>) -> AnyElement {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let busy = self.devices.writing.is_some();
        if self.devices.tab == DeviceTab::Profiles {
            return self.render_onboard_profiles(r, cx);
        }
        if self.devices.tab == DeviceTab::Buttons && r.onboard_profiles && r.tabs.buttons.is_empty() {
            return self.render_logitech_buttons(r, cx);
        }
        let mut col = div().flex().flex_col().gap_3().flex_1().min_w_0().max_w(px(720.));
        let placeholder = |text: &str| div().text_sm().text_color(muted).child(text.to_string()).into_any_element();

        let sections: Option<&Vec<crate::hid::Section>> = match self.devices.tab {
            DeviceTab::Buttons => Some(&r.tabs.buttons),
            DeviceTab::Advanced => Some(&r.tabs.advanced),
            _ => None,
        };
        if let Some(sections) = sections {
            if sections.is_empty() {
                return placeholder(if self.devices.tab == DeviceTab::Buttons {
                    "No reconfigurable buttons are exposed by this device's driver."
                } else {
                    "No brand-specific settings are exposed by this device's driver."
                });
            }
            for section in sections {
                let mut card = div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .rounded_lg()
                    .border_1()
                    .border_color(border)
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(fg).child(section.title.clone()));
                for control in &section.controls {
                    card = card.child(self.render_control(control, cx));
                }
                col = col.child(card);
            }
        } else if let Some(spec) = r.tabs.lighting.clone() {
            let e = &self.devices.staged.light;
            let mode = e.mode.clone().or_else(|| spec.mode.clone());
            let in_mode = |list: &[String]| mode.as_ref().is_some_and(|m| list.contains(m));
            let card = |title: &str| {
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
            if spec.write_only {
                col = col.child(div().text_xs().text_color(muted).child(
                    "Lighting is write-only on this device. The current effect is cached from the last change and may not reflect the hardware state.",
                ));
            }
            let mut effects = h_flex().gap_2().flex_wrap();
            for m in spec.modes.iter() {
                let spec2 = spec.clone();
                let picked = m.clone();
                effects = effects.child(self.chip(
                    format!("light-mode-{m}"),
                    m.clone(),
                    mode.as_deref() == Some(m.as_str()),
                    cx,
                    move |this, cx| this.edit_lighting(|l| l.mode = Some(picked.clone()), &spec2, cx),
                ));
            }
            col = col.child(card("Effect").child(effects));

            let swatches = |cx: &mut Context<Self>, which: u8, current: Option<String>| {
                let mut row = h_flex().gap_2().flex_wrap();
                for hex in LIGHT_SWATCHES {
                    let spec2 = spec.clone();
                    let picked = hex.to_string();
                    let selected = current.as_deref().is_some_and(|c| c.eq_ignore_ascii_case(hex));
                    row = row.child(
                        div()
                            .id(SharedString::from(format!("swatch-{which}-{hex}")))
                            .cursor_pointer()
                            .size(px(28.))
                            .rounded_full()
                            .border_2()
                            .border_color(if selected { cx.theme().foreground } else { Hsla { a: 0.0, ..hex_color(hex) } })
                            .bg(hex_color(hex))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let picked = picked.clone();
                                this.edit_lighting(
                                    move |l| if which == 1 { l.color = Some(picked) } else { l.color2 = Some(picked) },
                                    &spec2,
                                    cx,
                                )
                            })),
                    );
                }
                row
            };
            if in_mode(&spec.color_modes) {
                let current = e.color.clone().or_else(|| spec.color.clone());
                col = col.child(card("Color").child(swatches(cx, 1, current)));
            }
            if in_mode(&spec.dual_color_modes) {
                let current = e.color2.clone().or_else(|| spec.color2.clone());
                col = col.child(card("Second color").child(swatches(cx, 2, current)));
            }
            if in_mode(&spec.reactive_modes) && !spec.speeds.is_empty() {
                let current = e.speed.or(spec.speed);
                let mut row = h_flex().gap_2().flex_wrap();
                for sp in spec.speeds.iter().copied() {
                    let spec2 = spec.clone();
                    row = row.child(self.chip(
                        format!("light-speed-{sp}"),
                        format!("{sp:.0}"),
                        current.is_some_and(|c| (c - sp).abs() < 0.01),
                        cx,
                        move |this, cx| this.edit_lighting(|l| l.speed = Some(sp), &spec2, cx),
                    ));
                }
                col = col.child(card("Speed").child(row));
            }
        } else {
            return placeholder("No lighting data available for this device.");
        }

        if busy && self.settings.instant_apply {
            col = col.child(div().text_sm().text_color(muted).child("Applying…"));
        }
        if let Some(error) = &self.devices.write_error {
            col = col.child(div().text_sm().text_color(cx.theme().danger).child(format!("Could not apply: {error}")));
        }
        col.into_any_element()
    }
}
