//! Scanning, connecting to and writing to devices.

use super::*;
use futures::StreamExt as _;

impl AppView {
    pub(super) fn hid_service(&mut self) -> crate::hid::HidService {
        self.hid.get_or_insert_with(crate::hid::HidService::start).clone()
    }

    /// Lists supported devices, loads their artwork, and connects the first.
    /// Everything blocking runs on `crate::hid`'s threads or the background
    /// executor.
    pub(super) fn scan_devices(&mut self, cx: &mut Context<Self>) {
        if self.devices.scanning {
            self.devices.rescan = true;
            return;
        }
        self.devices.scanning = true;
        self.devices.error = None;
        cx.notify();
        let service = self.hid_service();
        cx.spawn(async move |view, cx| {
            let listed = service.list_candidates().await;
            let candidates = match listed {
                Ok(Ok(list)) => list,
                other => {
                    let message = match other {
                        Ok(Err(error)) => error,
                        _ => "device runtime stopped".to_string(),
                    };
                    let _ = view.update(cx, |v: &mut Self, cx| {
                        v.devices.scanning = false;
                        v.devices.error = Some(message);
                        cx.notify();
                        if std::mem::take(&mut v.devices.rescan) {
                            v.scan_devices(cx);
                        }
                    });
                    return;
                }
            };
            let _ = view.update(cx, |v: &mut Self, cx| {
                // Keep what is already known about devices that are still here.
                let mut old: Vec<Tile> = std::mem::take(&mut v.devices.tiles);
                v.devices.tiles = candidates
                    .iter()
                    .map(|c| {
                        match old.iter().position(|t| t.key == c.info.key) {
                            Some(i) => old.swap_remove(i),
                            None => Tile {
                                key: c.info.key.clone(),
                                name: c.info.product_string.clone(),
                                brand: c.brands.first().cloned().unwrap_or_default(),
                                display_brand: c.brands.first().cloned().unwrap_or_default(),
                                art: None,
                                status: TileStatus::Idle,
                            },
                        }
                    })
                    .collect();
                v.devices.selected = v.devices.selected.min(v.devices.tiles.len().saturating_sub(1));
                v.devices.scanning = false;
                v.close_unplugged_page(&old, cx);
                cx.notify();
                if std::mem::take(&mut v.devices.rescan) {
                    v.scan_devices(cx);
                }
            });
            for c in &candidates {
                // Use the name the tile carries now: a receiver that was
                // connected is known by its mouse's name, and resolving the
                // receiver's generic product string would swap the mouse's
                // artwork for the placeholder.
                let current = view
                    .update(cx, |v: &mut Self, _| {
                        v.devices
                            .tiles
                            .iter()
                            .find(|t| t.key == c.info.key)
                            .map(|t| (t.name.clone(), t.art.is_some()))
                    })
                    .ok()
                    .flatten();
                let Some((name, has_art)) = current else { continue };
                if has_art {
                    continue;
                }
                Self::load_art(&service, &c.info.key, &name, &view, cx).await;
            }
            let _ = view.update(cx, |v: &mut Self, cx| {
                if let Some(tile) = v.devices.tiles.first() {
                    if matches!(tile.status, TileStatus::Idle) {
                        let key = tile.key.clone();
                        v.connect_tile(&key, false, cx);
                    }
                }
            });
        })
        .detach();
    }

    /// Looks again whenever a device is plugged in or removed.
    pub(super) fn watch_devices(&mut self, cx: &mut Context<Self>) {
        let changes = crate::hotplug::watch();
        cx.spawn(async move |view, cx| {
            let executor = cx.background_executor().clone();
            let Some(mut changes) = changes else {
                // Nothing reports plug-ins here, so check on a timer.
                loop {
                    crate::activity::ACTIVITY
                        .sleep(&executor, Duration::from_secs(3), Duration::from_secs(15))
                        .await;
                    if view.update(cx, |v: &mut Self, cx| v.scan_devices(cx)).is_err() {
                        return;
                    }
                }
            };
            while changes.next().await.is_some() {
                // A device arrives one interface at a time: wait until it is quiet.
                loop {
                    executor.timer(Duration::from_millis(700)).await;
                    let mut more = false;
                    while changes.try_recv().is_ok() {
                        more = true;
                    }
                    if !more {
                        break;
                    }
                }
                if view.update(cx, |v: &mut Self, cx| v.scan_devices(cx)).is_err() {
                    return;
                }
            }
        })
        .detach();
    }

    /// Leaves the settings page of a device that is no longer there. `gone`
    /// holds the tiles the last scan did not find.
    fn close_unplugged_page(&mut self, gone: &[Tile], cx: &mut Context<Self>) {
        let Some(open) = self.devices.open.as_deref() else { return };
        let Some(name) = gone.iter().find(|t| t.key == open).map(|t| t.name.clone()) else { return };
        self.devices.open = None;
        self.devices.staged = Staged::default();
        self.show_toast(format!("{name} was disconnected."), false, cx);
    }

    pub(super) async fn load_art(
        service: &crate::hid::HidService,
        key: &str,
        name: &str,
        view: &WeakEntity<Self>,
        cx: &mut AsyncApp,
    ) {
        let Ok(Ok(art)) = service.device_image(key, name).await else { return };
        let placement = art.art.clone();
        let Some(path) = cx.background_executor().spawn(async move { art.fetch() }).await else {
            return;
        };
        let key = key.to_string();
        let _ = view.update(cx, |v: &mut Self, cx| {
            if let Some(tile) = v.devices.tiles.iter_mut().find(|t| t.key == key) {
                tile.art = Some(LoadedArt { path, placement });
                cx.notify();
            }
        });
    }

    /// Opens a device's settings page, connecting it first if needed.
    pub(super) fn open_device(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(index) = self.devices.tiles.iter().position(|t| t.key == key) else { return };
        self.devices.open = Some(key.to_string());
        self.devices.selected = index;
        self.devices.staged = Staged::default();
        self.devices.unlinked_axes = false;
        self.devices.tab = DeviceTab::Performance;
        self.devices.expanded = None;
        self.devices.onboard = None;
        self.devices.write_error = None;
        if matches!(
            self.devices.tiles[index].status,
            TileStatus::Idle | TileStatus::Failed(_)
        ) {
            self.connect_tile(key, false, cx);
        }
        cx.notify();
    }

    /// Records the Y sensitivity of a confirmed unlinked write.
    pub(super) fn patch_dpi_y(&mut self, key: &str, y: f64) {
        if let Some(Tile { status: TileStatus::Connected(r), .. }) = self.devices.tiles.iter_mut().find(|t| t.key == key) {
            r.dpi_y = Some(y);
        }
    }

    /// Reflects a confirmed write in the page. The value is what the driver
    /// reports the mouse applied (it can clamp or round), and it is used
    /// instead of a re-read: a Logitech mouse answers its get-report-rate call
    /// with a stale value after a live change (the driver only knows the real
    /// rate from the change notification it saw), so re-reading would show the
    /// old rate straight after a successful write. The Tauri app patches its
    /// status the same way.
    pub(super) fn patch_readout(&mut self, key: &str, action: &str, applied: &serde_json::Value) {
        let mut left_onboard = false;
        let Some(tile) = self.devices.tiles.iter_mut().find(|t| t.key == key) else { return };
        let TileStatus::Connected(r) = &mut tile.status else { return };
        // A Logitech mouse behind its receiver is put under software control by
        // these writes, which stops it running its stored profiles.
        if matches!(action, "dpi" | "pollingRate" | "liftOff" | "surface")
            && r.onboard_profiles
            && r.connection.as_deref() == Some("Wireless")
            && r.mode.as_deref() == Some("Onboard")
        {
            r.mode = Some("Host".to_string());
            left_onboard = true;
        }
        match action {
            "dpi" => {
                r.dpi = applied.as_f64().or(r.dpi);
                // Linked writes put Y on X as well; an unlinked pair sets Y after this.
                r.dpi_y = r.dpi;
            }
            "pollingRate" => r.polling_hz = applied.as_f64().or(r.polling_hz),
            "liftOff" => r.lod = applied.as_str().map(str::to_string).or_else(|| r.lod.take()),
            "surface" => r.surface = applied.as_str().map(str::to_string).or_else(|| r.surface.take()),
            "lighting" => {
                if let Some(l) = &mut r.tabs.lighting {
                    if let Some(mode) = applied.get("mode").and_then(|v| v.as_str()) {
                        l.mode = Some(mode.to_string());
                    }
                    l.color = applied.get("color").and_then(|v| v.as_str()).map(str::to_string).or_else(|| l.color.take());
                    l.color2 = applied.get("color2").and_then(|v| v.as_str()).map(str::to_string).or_else(|| l.color2.take());
                    l.speed = applied.get("speed").and_then(|v| v.as_f64()).or(l.speed);
                }
            }
            _ => {}
        }
        if left_onboard {
            // Which profile is running is no longer what we last read.
            self.devices.onboard = None;
        }
    }

    /// Reads the open Logitech mouse's stored profiles. On demand: a full pass
    /// is dozens of HID++ round trips.
    pub(super) fn load_onboard_profiles(&mut self, cx: &mut Context<Self>) {
        if self.devices.onboard_loading {
            return;
        }
        let Some(tile) = self.devices.tiles.get(self.devices.selected) else { return };
        let key = tile.key.clone();
        self.devices.onboard_loading = true;
        self.devices.write_error = None;
        cx.notify();
        let service = self.hid_service();
        cx.spawn(async move |view, cx| {
            let result = service.onboard_profiles(&key).await;
            let _ = view.update(cx, |v: &mut Self, cx| {
                v.devices.onboard_loading = false;
                match result {
                    Ok(Ok(list)) => v.devices.onboard = Some(list),
                    Ok(Err(error)) => v.devices.write_error = Some(error),
                    Err(_) => v.devices.write_error = Some("device runtime stopped".into()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// `mode`: switch Onboard / Host. `useProfile`: run a stored profile (this
    /// also puts the mouse in Onboard mode, which is the only mode that runs
    /// them). Neither writes flash.
    pub(super) fn onboard_action(&mut self, action: &'static str, value: serde_json::Value, cx: &mut Context<Self>) {
        if self.devices.writing.is_some() {
            return;
        }
        let Some(tile) = self.devices.tiles.get(self.devices.selected) else { return };
        let key = tile.key.clone();
        self.devices.writing = Some(action.to_string());
        self.devices.write_error = None;
        cx.notify();
        let service = self.hid_service();
        cx.spawn(async move |view, cx| {
            let result = service.onboard_action(&key, action, value.clone()).await;
            let _ = view.update(cx, |v: &mut Self, cx| {
                v.devices.writing = None;
                match result {
                    Ok(Ok(applied)) => {
                        if let Some(tile) = v.devices.tiles.iter_mut().find(|t| t.key == key) {
                            if let TileStatus::Connected(r) = &mut tile.status {
                                r.mode = Some(if action == "mode" {
                                    applied.as_str().unwrap_or("Host").to_string()
                                } else {
                                    "Onboard".to_string()
                                });
                            }
                        }
                        if action == "useProfile" {
                            v.show_toast(format!("Now running profile {}.", value), false, cx);
                        }
                        // Which profile is running changed; read the list again.
                        v.load_onboard_profiles(cx);
                    }
                    Ok(Err(error)) => v.devices.write_error = Some(error),
                    Err(_) => v.devices.write_error = Some("device runtime stopped".into()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Writes one Buttons/Advanced control (always immediate, as in the Tauri
    /// app) and shows the value that was sent once the mouse accepts it.
    pub(super) fn set_control(&mut self, id: String, value: serde_json::Value, cx: &mut Context<Self>) {
        if self.devices.writing.is_some() || (id.starts_with("perf.") && self.blocked_by_profile(cx)) {
            return;
        }
        let Some(tile) = self.devices.tiles.get(self.devices.selected) else { return };
        let (key, brand) = (tile.key.clone(), tile.brand.clone());
        // "Link left & right" is one preference for every device that has it.
        if id.ends_with(".link") {
            if let Some(linked) = value.as_bool() {
                self.settings.link_left_right = Some(linked);
                self.settings.save();
                self.hid_service().set_prefs(Some(linked));
            }
        }
        self.devices.writing = Some(id.clone());
        self.devices.write_error = None;
        cx.notify();
        let service = self.hid_service();
        cx.spawn(async move |view, cx| {
            let result = service.write(&key, &brand, &format!("control:{id}"), value.clone()).await;
            let _ = view.update(cx, |v: &mut Self, cx| {
                v.devices.writing = None;
                match result {
                    Ok(Ok(applied)) => {
                        // A control that reshapes the page (a switch gaining analog settings) returns the new layout.
                        let new_tabs = applied.get("tabs").and_then(|t| serde_json::from_value::<crate::hid::Tabs>(t.clone()).ok());
                        if let Some(tile) = v.devices.tiles.iter_mut().find(|t| t.key == key) {
                            if let (Some(tabs), TileStatus::Connected(r)) = (new_tabs, &mut tile.status) {
                                r.tabs = tabs;
                            }
                        }
                        // A Logitech button: reflect it in the stored profile we hold.
                        if let Some(n) = id.strip_prefix("logi.button.primary.").and_then(|n| n.parse::<usize>().ok()) {
                            if let Some(running) = v
                                .devices
                                .onboard
                                .as_mut()
                                .and_then(|o| o.profiles.iter_mut().find(|p| p.is_current))
                            {
                                if let Some(slot) = running.button_assignments.get_mut(n) {
                                    slot["action"] = value.clone();
                                }
                            }
                        }
                        if id.starts_with("logi.button.") {
                            v.show_toast(format!("Button set to {}.", value.as_str().unwrap_or("the new action")), false, cx);
                        }
                        if let Some(tile) = v.devices.tiles.iter_mut().find(|t| t.key == key) {
                            if let TileStatus::Connected(r) = &mut tile.status {
                                for section in r.tabs.buttons.iter_mut().chain(r.tabs.advanced.iter_mut()).chain(r.tabs.performance.iter_mut()) {
                                    for control in &mut section.controls {
                                        if control.id == id {
                                            control.current = value.clone();
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Ok(Err(error)) => v.devices.write_error = Some(error),
                    Err(_) => v.devices.write_error = Some("device runtime stopped".into()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The lighting as it would be written now: staged fields over the device's.
    pub(super) fn effective_lighting(&self, spec: &crate::hid::LightingSpec) -> serde_json::Value {
        let e = &self.devices.staged.light;
        serde_json::json!({
            "mode": e.mode.clone().or_else(|| spec.mode.clone()),
            "color": e.color.clone().or_else(|| spec.color.clone()),
            "color2": e.color2.clone().or_else(|| spec.color2.clone()),
            "speed": e.speed.or(spec.speed),
        })
    }

    /// One lighting edit: written now with instant apply, staged otherwise.
    pub(super) fn edit_lighting(
        &mut self,
        apply: impl FnOnce(&mut LightEdit),
        spec: &crate::hid::LightingSpec,
        cx: &mut Context<Self>,
    ) {
        apply(&mut self.devices.staged.light);
        if self.settings.instant_apply {
            let value = self.effective_lighting(spec);
            self.devices.staged.light = LightEdit::default();
            self.write_setting("lighting", value, cx);
        }
        cx.notify();
        }

    /// Writes one setting and shows the value the mouse confirmed.
    pub(super) fn write_setting(&mut self, action: &'static str, value: serde_json::Value, cx: &mut Context<Self>) {
        if self.devices.writing.is_some() {
            return;
        }
        let Some(tile) = self.devices.tiles.get(self.devices.selected) else { return };
        let (key, brand) = (tile.key.clone(), tile.brand.clone());
        let requested_y = value.get("y").and_then(|y| y.as_f64());
        self.devices.writing = Some(action.to_string());
        self.devices.write_error = None;
        cx.notify();
        let service = self.hid_service();
        cx.spawn(async move |view, cx| {
            let result = service.write(&key, &brand, action, value).await;
            let _ = view.update(cx, |v: &mut Self, cx| {
                v.devices.writing = None;
                match result {
                    Ok(Ok(applied)) => {
                        v.patch_readout(&key, action, &applied);
                        if let Some(y) = requested_y {
                            v.patch_dpi_y(&key, y);
                        }
                        v.refresh_presence(cx);
                    }
                    Ok(Err(error)) => v.devices.write_error = Some(error),
                    Err(_) => v.devices.write_error = Some("device runtime stopped".into()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Writes every staged edit in order (one device session each) and shows
    /// what the mouse confirmed. Stops at the first failure and keeps the rest
    /// staged so nothing is silently dropped.
    pub(super) fn apply_staged(&mut self, cx: &mut Context<Self>) {
        if self.devices.writing.is_some() || self.devices.staged.is_empty() {
            return;
        }
        let Some(tile) = self.devices.tiles.get(self.devices.selected) else { return };
        let (key, brand) = (tile.key.clone(), tile.brand.clone());
        let staged = self.devices.staged.clone();
        let mut jobs: Vec<(&'static str, serde_json::Value)> = Vec::new();
        if staged.dpi.is_some() || staged.dpi_y.is_some() {
            let current_x = match self.devices.tiles.get(self.devices.selected).map(|t| &t.status) {
                Some(TileStatus::Connected(r)) => r.dpi,
                _ => None,
            };
            let x = staged.dpi.or(current_x).unwrap_or(0.0);
            jobs.push(("dpi", match staged.dpi_y {
                Some(y) => serde_json::json!({ "x": x, "y": y }),
                None => serde_json::json!(x),
            }));
        }
        if let Some(v) = staged.polling {
            jobs.push(("pollingRate", serde_json::json!(v)));
        }
        if let Some(v) = staged.lod.clone() {
            jobs.push(("liftOff", serde_json::json!(v)));
        }
        if let Some(v) = staged.surface.clone() {
            jobs.push(("surface", serde_json::json!(v)));
        }
        let light_job = if staged.light == LightEdit::default() {
            None
        } else {
            match &self.devices.tiles.get(self.devices.selected).map(|t| &t.status) {
                Some(TileStatus::Connected(r)) => r.tabs.lighting.as_ref().map(|spec| self.effective_lighting(spec)),
                _ => None,
            }
        };
        if let Some(value) = light_job {
            jobs.push(("lighting", value));
        }
        self.devices.writing = Some("apply".into());
        self.devices.write_error = None;
        cx.notify();
        let service = self.hid_service();
        cx.spawn(async move |view, cx| {
            let mut failure = None;
            let mut done = Staged::default();
            let mut applied_values: Vec<(&'static str, serde_json::Value)> = Vec::new();
            for (action, value) in jobs {
                match service.write(&key, &brand, action, value).await {
                    Ok(Ok(applied)) => {
                        applied_values.push((action, applied));
                        match action {
                        "dpi" => {
                            done.dpi = staged.dpi;
                            done.dpi_y = staged.dpi_y;
                        }
                        "pollingRate" => done.polling = staged.polling,
                        "liftOff" => done.lod = staged.lod.clone(),
                        "lighting" => done.light = staged.light.clone(),
                        _ => done.surface = staged.surface.clone(),
                        }
                    }
                    Ok(Err(error)) => {
                        failure = Some(error);
                        break;
                    }
                    Err(_) => {
                        failure = Some("device runtime stopped".to_string());
                        break;
                    }
                }
            }
            let _ = view.update(cx, |v: &mut Self, cx| {
                v.devices.writing = None;
                let s = &mut v.devices.staged;
                if done.dpi.is_some() { s.dpi = None; }
                if done.dpi_y.is_some() { s.dpi_y = None; }
                if done.polling.is_some() { s.polling = None; }
                if done.lod.is_some() { s.lod = None; }
                if done.surface.is_some() { s.surface = None; }
                if done.light != LightEdit::default() { s.light = LightEdit::default(); }
                v.devices.write_error = failure;
                for (action, applied) in &applied_values {
                    v.patch_readout(&key, action, applied);
                    if *action == "dpi" {
                        if let Some(y) = staged.dpi_y {
                            v.patch_dpi_y(&key, y);
                        }
                    }
                }
                v.refresh_presence(cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// The game whose profile currently controls the open device, if any. While
    /// one does, the Performance tab is locked: its settings are the profile's
    /// until the game closes (the Tauri tab does the same).
    pub(super) fn locked_game(&self) -> Option<String> {
        let key = &self.devices.tiles.get(self.devices.selected)?.key;
        self.profile_overrides.iter().find(|o| &o.device == key).map(|o| o.game_name.clone())
    }

    /// Refuses an edit with a note when [`Self::locked_game`] applies.
    pub(super) fn blocked_by_profile(&mut self, cx: &mut Context<Self>) -> bool {
        match self.locked_game() {
            Some(game) => {
                self.show_toast(format!("Locked while {game} is running — your game profile is in control."), true, cx);
                true
            }
            None => false,
        }
    }

    /// A DPI edit. Linked (the default) writes one number for both axes; with the
    /// axes unlinked, X and Y go together as a pair.
    pub(super) fn edit_dpi(&mut self, x: f64, y: Option<f64>, cx: &mut Context<Self>) {
        if self.blocked_by_profile(cx) {
            return;
        }
        let unlinked = self.devices.unlinked_axes;
        if self.settings.instant_apply {
            let value = if unlinked { serde_json::json!({ "x": x, "y": y.unwrap_or(x) }) } else { serde_json::json!(x) };
            self.write_setting("dpi", value, cx);
            return;
        }
        self.devices.staged.dpi = Some(x);
        self.devices.staged.dpi_y = unlinked.then(|| y.unwrap_or(x));
        cx.notify();
    }

    /// Records an edit: written now with instant apply, staged otherwise.
    pub(super) fn edit(&mut self, action: &'static str, value: serde_json::Value, cx: &mut Context<Self>) {
        if self.blocked_by_profile(cx) {
            return;
        }
        if self.settings.instant_apply {
            self.write_setting(action, value, cx);
            return;
        }
        let st = &mut self.devices.staged;
        match action {
            "dpi" => st.dpi = value.as_f64(),
            "pollingRate" => st.polling = value.as_u64().map(|n| n as u32),
            "liftOff" => st.lod = value.as_str().map(str::to_string),
            _ => st.surface = value.as_str().map(str::to_string),
        }
        cx.notify();
    }

    /// Opens one device, finds the driver that answers and reads its status.
    pub(super) fn connect_tile(&mut self, key: &str, quiet: bool, cx: &mut Context<Self>) {
        let Some(index) = self.devices.tiles.iter().position(|t| t.key == key) else { return };
        if matches!(self.devices.tiles[index].status, TileStatus::Connecting) {
            return;
        }
        // A refresh after a write keeps showing the old readout until the new
        // one lands, so the page does not flash "Connecting…".
        if !quiet {
            self.devices.tiles[index].status = TileStatus::Connecting;
        }
        self.devices.selected = index;
        cx.notify();
        let service = self.hid_service();
        service.set_prefs(self.settings.link_left_right);
        let key = key.to_string();
        cx.spawn(async move |view, cx| {
            let connected = service.connect(&key).await;
            let mut new_name = None;
            let _ = view.update(cx, |v: &mut Self, cx| {
                let mut connected_now = false;
                let Some(tile) = v.devices.tiles.iter_mut().find(|t| t.key == key) else { return };
                match connected {
                    Ok(Ok(device)) => {
                        if let Some(name) = device
                            .status
                            .get("name")
                            .and_then(|n| n.as_str())
                            .filter(|n| !n.is_empty() && *n != tile.name)
                        {
                            tile.name = name.to_string();
                            new_name = Some(name.to_string());
                        }
                        tile.brand = device.brand.clone();
                        tile.display_brand = device.display_brand.clone().filter(|b| !b.is_empty()).unwrap_or_else(|| device.brand.clone());
                        connected_now = true;
                        tile.status = TileStatus::Connected(DeviceReadout::from_status(&device.status, device.controls.clone(), device.tabs.clone().unwrap_or_default()));
                    }
                    Ok(Err(error)) => {
                        if !(quiet && matches!(tile.status, TileStatus::Connected(_))) {
                            tile.status = TileStatus::Failed(error);
                        }
                    }
                    Err(_) => tile.status = TileStatus::Failed("device runtime stopped".into()),
                }
                if connected_now {
                    v.sync_running_profiles(cx);
                }
                v.refresh_presence(cx);
                cx.notify();
            });
            // Receivers share product ids, so the artwork is only knowable
            // from the name the mouse itself reports.
            if let Some(name) = new_name {
                Self::load_art(&service, &key, &name, &view, cx).await;
            }
        })
        .detach();
    }

    /// Every connected device with its readout, in list order.
    pub(super) fn connected_devices(&self) -> Vec<(String, &DeviceReadout)> {
        self.devices
            .tiles
            .iter()
            .filter_map(|t| match &t.status {
                TileStatus::Connected(r) => Some((t.key.clone(), r)),
                _ => None,
            })
            .collect()
    }
}
