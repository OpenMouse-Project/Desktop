//! Per-game profiles: applying them as games start and stop, and their editor.

use super::*;

/// While a game's profile is in control: what to put back when it closes.
pub(super) struct ProfileOverride {
    pub(super) game_id: String,
    pub(super) game_name: String,
    /// The device the profile was applied to (and will be restored on).
    pub(super) device: String,
    pub(super) restore: crate::profiles::GameProfile,
}

impl AppView {
    pub(crate) fn apply_game_updates(
        &mut self,
        updates: &[crate::game_watcher::GameUpdate],
        cx: &mut Context<Self>,
    ) {
        // Edges, found by comparing with what was known before this batch.
        let mut started = Vec::new();
        let mut closed = Vec::new();
        for update in updates {
            let Some(running) = update.running else { continue };
            let was = self.games.iter().find(|g| g.id == update.id).map(|g| g.running);
            match (was, running) {
                (Some(false), true) => started.push(update.id.clone()),
                (Some(true), false) => closed.push(update.id.clone()),
                _ => {}
            }
        }
        if crate::game_watcher::apply_updates(&mut self.games, updates) {
            cx.notify();
            self.refresh_presence(cx);
        }
        for id in closed {
            self.on_game_closed(&id, cx);
        }
        for id in started {
            self.on_game_started(&id, cx);
        }
    }

    // ── toasts ───────────────────────────────────────────────────────────

    /// The device that inherits a profile saved before profiles were per-device:
    /// the first mouse-like (has a DPI) connected one.
    pub(super) fn legacy_heir(&self) -> Option<String> {
        self.connected_devices().into_iter().find(|(_, r)| r.dpi.is_some()).map(|(k, _)| k)
    }

    /// The profile a game has for this device (its own, or the inherited legacy one).
    pub(super) fn profile_for(&self, game_id: &str, device: &str) -> Option<crate::profiles::GameProfile> {
        let heir = self.legacy_heir().as_deref() == Some(device);
        self.profiles.effective(game_id, device, heir).cloned()
    }

    pub(super) fn game_name(&self, id: &str) -> String {
        self.games.iter().find(|g| g.id == id).map(|g| g.name.clone()).unwrap_or_else(|| id.to_string())
    }

    /// A game launched: announce profiles that don't auto-apply, and apply the
    /// ones that do.
    pub(super) fn on_game_started(&mut self, id: &str, cx: &mut Context<Self>) {
        let notice = self
            .connected_devices()
            .iter()
            .filter_map(|(key, _)| self.profile_for(id, key))
            .find(|p| p.is_meaningful() && !p.auto_apply)
            .map(|p| p.describe());
        if let Some(described) = notice {
            let text = format!("\"{}\" has a profile ({described}). Apply it from the Games page.", self.game_name(id));
            self.announce(text, false, true, cx);
        }
        self.sync_running_profiles(cx);
    }

    /// Applies every auto-apply profile that should be in force: for each game
    /// that is running and each connected device, unless that pair is already
    /// applied or being applied. Safe to call whenever a game starts or a
    /// device connects — a device that shows up after the game launched is
    /// picked up here too.
    pub(super) fn sync_running_profiles(&mut self, cx: &mut Context<Self>) {
        let running: Vec<String> = self.games.iter().filter(|g| g.running).map(|g| g.id.clone()).collect();
        let devices: Vec<String> = self.connected_devices().into_iter().map(|(k, _)| k).collect();
        for game in running {
            for device in &devices {
                let Some(profile) = self.profile_for(&game, device) else { continue };
                if !profile.auto_apply || !profile.is_meaningful() {
                    continue;
                }
                let pair = (game.clone(), device.clone());
                let taken = self.profile_overrides.iter().any(|o| o.game_id == game && &o.device == device);
                if taken || self.profile_inflight.contains(&pair) {
                    continue;
                }
                self.apply_profile(&game, device, profile, true, cx);
            }
        }
    }

    /// The game closed: put back what each of its profiles replaced.
    pub(super) fn on_game_closed(&mut self, id: &str, cx: &mut Context<Self>) {
        let (closing, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut self.profile_overrides).into_iter().partition(|o| o.game_id == id);
        self.profile_overrides = keep;
        for active in closing {
            if !active.restore.is_meaningful() {
                continue;
            }
            let (name, described) = (active.game_name.clone(), active.restore.describe());
            let text = format!("{name} closed — restored your default {described}.");
            self.apply_profile_inner(id, &name, &active.device, active.restore, None, Some(text), true, cx);
        }
    }

    /// Pushes a profile to one connected device. `auto` marks a launch-triggered
    /// apply, which remembers the pre-game settings to restore on close.
    pub(super) fn apply_profile(&mut self, game_id: &str, device: &str, profile: crate::profiles::GameProfile, auto: bool, cx: &mut Context<Self>) {
        let Some(r) = self.connected_devices().into_iter().find(|(k, _)| k == device).map(|(_, r)| r) else {
            self.announce("Connect this device to apply a profile.".into(), true, auto, cx);
            return;
        };
        let name = self.game_name(game_id);
        let described = profile.describe();
        let override_ = if auto {
            // The first game to take over a device defines its "default"; a
            // second game taking over from a running one only changes who is in
            // control.
            let restore = match self.profile_overrides.iter().find(|o| o.device == device) {
                Some(active) => active.restore.clone(),
                None => crate::profiles::GameProfile {
                    dpi: profile.dpi.and(r.dpi),
                    polling: profile.polling.and(r.polling_hz.map(|hz| hz as u32)),
                    lod: profile.lod.as_ref().and(r.lod.clone()),
                    surface: profile.surface.as_ref().and(r.surface.clone()),
                    auto_apply: false,
                },
            };
            Some(ProfileOverride { game_id: game_id.to_string(), game_name: name.clone(), device: device.to_string(), restore })
        } else {
            None
        };
        if auto {
            self.profile_inflight.push((game_id.to_string(), device.to_string()));
        }
        self.apply_profile_inner(game_id, &name, device, profile, override_, Some(format!("Applied \"{name}\" profile — {described}.")), auto, cx);
    }

    pub(super) fn apply_profile_inner(
        &mut self,
        game_id: &str,
        name: &str,
        device: &str,
        profile: crate::profiles::GameProfile,
        override_: Option<ProfileOverride>,
        success: Option<String>,
        overlay: bool,
        cx: &mut Context<Self>,
    ) {
        let (game_id, name, key) = (game_id.to_string(), name.to_string(), device.to_string());
        let service = self.hid_service();
        cx.spawn(async move |view, cx| {
            let result = service.apply_profile(&key, profile.to_js()).await;
            let _ = view.update(cx, |v: &mut Self, cx| {
                v.profile_inflight.retain(|(g, d)| !(g == &game_id && d == &key));
                match result {
                    Ok(Ok(applied)) => {
                        for (field, action) in [
                            ("dpi", "dpi"),
                            ("pollingRateHz", "pollingRate"),
                            ("liftOffDistance", "liftOff"),
                            ("gamingSurfaceMode", "surface"),
                        ] {
                            if let Some(value) = applied.get(field) {
                                v.patch_readout(&key, action, value);
                            }
                        }
                        // Only take over once the write is confirmed to have landed.
                        if let Some(o) = override_ {
                            v.profile_overrides.push(o);
                        }
                        if let Some(text) = success {
                            v.announce(text, false, overlay, cx);
                        }
                        v.refresh_presence(cx);
                        cx.notify();
                    }
                    Ok(Err(error)) => v.announce(format!("Couldn't apply \"{name}\" profile: {error}"), true, overlay, cx),
                    Err(_) => v.announce(format!("Couldn't apply \"{name}\" profile: device runtime stopped"), true, overlay, cx),
                }
            });
        })
        .detach();
    }

    /// Edits (and persists) one game's profile for one device.
    pub(super) fn edit_profile(&mut self, game_id: &str, device: &str, change: impl FnOnce(&mut crate::profiles::GameProfile), cx: &mut Context<Self>) {
        let mut profile = self.profile_for(game_id, device).unwrap_or_default();
        change(&mut profile);
        self.profiles.set(game_id, device, profile);
        self.profiles.save();
        cx.notify();
    }

    /// The device the profile screen is editing: the one picked, else the first
    /// connected, else the first listed.
    pub(super) fn profile_target(&self) -> Option<usize> {
        let tiles = &self.devices.tiles;
        self.profile_device
            .as_ref()
            .and_then(|key| tiles.iter().position(|t| &t.key == key))
            .or_else(|| tiles.iter().position(|t| matches!(t.status, TileStatus::Connected(_))))
            .or(if tiles.is_empty() { None } else { Some(0) })
    }

    /// One game's device profile: pick a device, then which of its settings the
    /// profile overrides, whether it applies on launch, and Apply now / Clear.
    pub(super) fn render_profile_editor(&self, game_id: &str, cx: &mut Context<Self>) -> AnyElement {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let name = self.game_name(game_id);
        let target = self.profile_target();

        let header = h_flex()
            .gap_3()
            .items_center()
            .child(
                Button::new("profile-back")
                    .ghost()
                    .icon(IconName::ArrowLeft)
                    .tooltip("Back to games")
                    .accessibility_label("Back to games")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.profile_editor = None;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).text_color(fg).child(name))
                    .child(div().text_sm().text_color(muted).child("Device profile")),
            );

        // Device picker: one chip per device. A device that isn't connected yet is
        // connected when picked, since its options come from the device itself.
        let picker: AnyElement = if self.devices.tiles.is_empty() {
            div().into_any_element()
        } else {
            let mut row = h_flex().gap_2().flex_wrap();
            for (i, tile) in self.devices.tiles.iter().enumerate() {
                let key = tile.key.clone();
                let has_profile = self.profiles.get(game_id, &tile.key).is_some_and(|p| p.is_meaningful());
                let label = if tile.name.is_empty() { "Unknown device".to_string() } else { tile.name.clone() };
                row = row.child(self.chip(
                    format!("pf-device-{}", tile.key),
                    if has_profile { format!("{label} ●") } else { label },
                    target == Some(i),
                    cx,
                    move |this, cx| {
                        this.profile_device = Some(key.clone());
                        if let Some(t) = this.devices.tiles.iter().find(|t| t.key == key) {
                            if matches!(t.status, TileStatus::Idle | TileStatus::Failed(_)) {
                                this.connect_tile(&key, false, cx);
                            }
                        }
                        cx.notify();
                    },
                ));
            }
            div()
                .flex()
                .flex_col()
                .gap_2()
                .max_w(px(720.))
                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(fg).child("Device"))
                .child(div().text_xs().text_color(muted).child("This profile is saved for the device you pick. ● marks devices that already have one."))
                .child(row)
                .into_any_element()
        };

        let body: AnyElement = match target.and_then(|i| self.devices.tiles.get(i)) {
            None => div()
                .text_sm()
                .text_color(muted)
                .child("No device found. Connect a mouse and press Rescan on the Overview page.")
                .into_any_element(),
            Some(tile) => match &tile.status {
                TileStatus::Connected(r) => {
                    let device = tile.key.clone();
                    let profile = self.profile_for(game_id, &device).unwrap_or_default();
                    let section = |title: &str, hint: &str| {
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .p_4()
                            .rounded_lg()
                            .border_1()
                            .border_color(border)
                            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(fg).child(title.to_string()))
                            .child(div().text_xs().text_color(muted).child(hint.to_string()))
                    };
                    let id = game_id.to_string();
                    let mut col = div().flex().flex_col().gap_3().max_w(px(720.));
                    let has_options = r.dpi.is_some() || !r.polling_rates.is_empty() || !r.lod_options.is_empty() || r.surface.is_some();
                    if !has_options {
                        col = col.child(div().text_sm().text_color(muted).child(
                            "This device has no settings a game profile can change.",
                        ));
                    }

                    if let Some(c) = &r.controls {
                        if r.dpi.is_some() {
                            let (gid, dev) = (id.clone(), device.clone());
                            let mut row = h_flex().gap_2().flex_wrap().child(self.chip(
                                "pf-dpi-none".into(),
                                "Unchanged".into(),
                                profile.dpi.is_none(),
                                cx,
                                move |this, cx| this.edit_profile(&gid, &dev, |p| p.dpi = None, cx),
                            ));
                            for preset in c.dpi_presets.iter().copied().filter(|p| *p >= c.dpi.min && *p <= c.dpi.max) {
                                let (gid, dev) = (id.clone(), device.clone());
                                row = row.child(self.chip(
                                    format!("pf-dpi-{preset}"),
                                    format!("{preset:.0}"),
                                    profile.dpi.is_some_and(|d| (d - preset).abs() < 0.5),
                                    cx,
                                    move |this, cx| this.edit_profile(&gid, &dev, |p| p.dpi = Some(preset), cx),
                                ));
                            }
                            col = col.child(section("Sensitivity (DPI)", "Set when the profile applies.").child(row));
                        }
                    }
                    if !r.polling_rates.is_empty() {
                        let (gid, dev) = (id.clone(), device.clone());
                        let mut row = h_flex().gap_2().flex_wrap().child(self.chip(
                            "pf-poll-none".into(),
                            "Unchanged".into(),
                            profile.polling.is_none(),
                            cx,
                            move |this, cx| this.edit_profile(&gid, &dev, |p| p.polling = None, cx),
                        ));
                        for rate in r.polling_rates.iter().copied() {
                            let (gid, dev) = (id.clone(), device.clone());
                            row = row.child(self.chip(
                                format!("pf-poll-{rate}"),
                                format!("{rate} Hz"),
                                profile.polling == Some(rate),
                                cx,
                                move |this, cx| this.edit_profile(&gid, &dev, |p| p.polling = Some(rate), cx),
                            ));
                        }
                        col = col.child(section("Polling rate", "Leave on Unchanged to keep the mouse's rate.").child(row));
                    }
                    if !r.lod_options.is_empty() {
                        let (gid, dev) = (id.clone(), device.clone());
                        let mut row = h_flex().gap_2().flex_wrap().child(self.chip(
                            "pf-lod-none".into(),
                            "Unchanged".into(),
                            profile.lod.is_none(),
                            cx,
                            move |this, cx| this.edit_profile(&gid, &dev, |p| p.lod = None, cx),
                        ));
                        for option in r.lod_options.iter() {
                            let (gid, dev, picked) = (id.clone(), device.clone(), option.clone());
                            row = row.child(self.chip(
                                format!("pf-lod-{option}"),
                                option.clone(),
                                profile.lod.as_deref() == Some(option.as_str()),
                                cx,
                                move |this, cx| this.edit_profile(&gid, &dev, |p| p.lod = Some(picked.clone()), cx),
                            ));
                        }
                        col = col.child(section("Lift-off distance", "").child(row));
                    }
                    if let (Some(_), Some(c)) = (&r.surface, &r.controls) {
                        let (gid, dev) = (id.clone(), device.clone());
                        let mut row = h_flex().gap_2().flex_wrap().child(self.chip(
                            "pf-surface-none".into(),
                            "Unchanged".into(),
                            profile.surface.is_none(),
                            cx,
                            move |this, cx| this.edit_profile(&gid, &dev, |p| p.surface = None, cx),
                        ));
                        for mode in c.surface_modes.iter() {
                            let (gid, dev, picked) = (id.clone(), device.clone(), mode.clone());
                            row = row.child(self.chip(
                                format!("pf-surface-{mode}"),
                                mode.clone(),
                                profile.surface.as_deref() == Some(mode.as_str()),
                                cx,
                                move |this, cx| this.edit_profile(&gid, &dev, |p| p.surface = Some(picked.clone()), cx),
                            ));
                        }
                        col = col.child(section("Gaming surface mode", "").child(row));
                    }

                    if has_options {
                        let (gid_auto, dev_auto) = (id.clone(), device.clone());
                        let (gid_apply, dev_apply, applying) = (id.clone(), device.clone(), profile.clone());
                        let (gid_clear, dev_clear) = (id.clone(), device.clone());
                        col = col
                            .child(
                                h_flex()
                                    .gap_4()
                                    .items_center()
                                    .justify_between()
                                    .p_4()
                                    .rounded_lg()
                                    .border_1()
                                    .border_color(border)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_0p5()
                                            .child(div().text_sm().text_color(fg).child("Apply when the game launches"))
                                            .child(div().text_xs().text_color(muted).child("Your previous settings come back when the game closes.")),
                                    )
                                    .child(
                                        Switch::new("profile-auto")
                                            .checked(profile.auto_apply)
                                            .on_change(cx.listener(move |this, on: &bool, _, cx| {
                                                let on = *on;
                                                this.edit_profile(&gid_auto, &dev_auto, |p| p.auto_apply = on, cx)
                                            })),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("profile-apply").label("Apply now").on_click(cx.listener(move |this, _, _, cx| {
                                            if applying.is_meaningful() {
                                                this.apply_profile(&gid_apply, &dev_apply, applying.clone(), false, cx);
                                            } else {
                                                this.show_toast("This profile doesn't change anything yet.", true, cx);
                                            }
                                        })),
                                    )
                                    .child(
                                        Button::new("profile-clear").ghost().label("Clear profile").on_click(cx.listener(move |this, _, _, cx| {
                                            this.profiles.clear(&gid_clear, &dev_clear);
                                            this.profiles.save();
                                            cx.notify();
                                        })),
                                    ),
                            );
                    }
                    col.into_any_element()
                }
                TileStatus::Failed(error) => div()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(format!("Could not read this device: {error}"))
                    .into_any_element(),
                _ => div().text_sm().text_color(muted).child("Connecting…").into_any_element(),
            },
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .gap_4()
            .overflow_y_scrollbar()
            .child(header)
            .child(picker)
            .child(body)
            .into_any_element()
    }
}
