use crate::games::{self, Game};
use crate::settings::{Settings, MAX_BLUR, MAX_VIBRANCY, MIN_OPACITY};
use crate::theme::ThemeChoice;
use gpui_kit::assets::IconName;
use gpui_kit::base::slider::{SliderEvent, SliderState};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::radio::{Radio, RadioGroup};
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::Icon;
use gpui_kit::component::slider::Slider;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{h_flex, ActiveTheme as _, Disableable as _, Sizable as _, TitleBar};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::time::Duration;

mod appearance;
mod ascii;
mod chrome;
mod controls;
mod crash;
mod device_io;
mod device_page;
mod device_state;
mod games_page;
mod overview;
mod profile_ui;
mod settings_page;
mod updates;
mod widgets;

use appearance::*;
use chrome::*;
use device_page::*;
use device_state::*;
use profile_ui::*;
use updates::*;
use widgets::*;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Mirrors `Page` in `src/layouts/FullDesktopView.tsx`: the four top-level
/// screens behind the app sidebar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Overview,
    Games,
    Supported,
    Settings,
    /// Version and release notes; opened from the sidebar footer, not the nav list.
    Updates,
}

impl Page {
    pub fn label(self) -> &'static str {
        match self {
            Page::Overview => "Overview",
            Page::Games => "Games",
            Page::Supported => "Supported mice",
            Page::Settings => "Settings",
            Page::Updates => "Updates",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Page::Overview => IconName::LayoutDashboard,
            Page::Games => IconName::Gamepad2,
            Page::Supported => IconName::BookOpen,
            Page::Settings => IconName::Settings,
            Page::Updates => IconName::CircleArrowUp,
        }
    }
}

/// A short-lived message at the bottom of the window.
struct Toast {
    id: u64,
    text: String,
    error: bool,
}

const PAGE_PAD: f32 = 16.0;
const SIDEBAR_W: f32 = 264.0;
const SIDEBAR_COLLAPSED_W: f32 = 72.0;

/// Root view: custom titlebar + icon sidebar + page content.
/// Mirrors `FullDesktopView` (`TitleBar` + `AppSidebar` + page switch).
pub struct AppView {
    page: Page,
    sidebar_collapsed: bool,
    settings: Settings,
    games: Vec<Game>,
    devices: Devices,
    hid: Option<crate::hid::HidService>,
    presence: std::sync::Arc<openmouse_presence::Presence>,
    profiles: crate::profiles::Profiles,
    /// Game whose profile is being edited on the Games page.
    profile_editor: Option<String>,
    /// One per (game, device) whose profile is currently in control.
    profile_overrides: Vec<ProfileOverride>,
    /// (game, device) pairs with a write in flight, so none is applied twice.
    profile_inflight: Vec<(String, String)>,
    /// The device being edited on the profile screen (None: the first connected).
    profile_device: Option<String>,
    toast: Option<Toast>,
    toast_seq: u64,
    /// A crash report from an earlier run that has not been offered yet.
    crash_report: Option<std::path::PathBuf>,
    /// The Updates page's virtualized list (only the rows on screen are built).
    updates_list: ListState,
    /// Newest version `update_check` has seen, if it's ahead of `APP_VERSION`.
    update_available: Option<crate::update_check::UpdateInfo>,
    checking_for_updates: bool,
    /// The on-screen game overlay while one is up.
    overlay: Option<AnyWindowHandle>,
    /// Newest scheduled presence update wins (Discord rate-limits them).
    presence_seq: u64,
    /// Last failure from the Launch-on-startup / Discord switches, shown under them.
    startup_error: Option<String>,
    discord_error: Option<String>,
    badge_hovered: bool,
    opacity_slider: Option<Entity<SliderState>>,
    vibrancy_slider: Option<Entity<SliderState>>,
    /// One slider per number control on the open device page (`<device>/<control id>`),
    /// minted by `ensure_control_sliders` because rendering has no window.
    control_sliders: std::collections::HashMap<String, Entity<SliderState>>,
    /// What a slider shows while it is being dragged, before the write lands.
    slider_live: std::collections::HashMap<String, f64>,
    slider_seq: u64,
    _subscriptions: Vec<Subscription>,
    _game_watcher: Option<crate::game_watcher::GameWatcher>,
    /// Cached ASCII background rows, keyed by their grid dimensions.
    ascii_rows: Vec<SharedString>,
    ascii_dims: (usize, usize),
    /// Bumped on every theme change; the ASCII field's animation id carries
    /// it, so each switch replays the entrance.
    theme_seq: u64,
}

impl AppView {
    /// Build the root view, owning the Settings sliders. Call from the
    /// `open_window` closure where `cx` can mint the slider entities.
    /// `initial_theme`/`wallpaper` mirror the startup path (Dark first paint
    /// or sampled Dynamic); persisted opacity/vibrancy seed the sliders.
    pub fn build(
        initial_theme: ThemeChoice,
        wallpaper: Option<(u8, u8, u8)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let settings = Settings {
            theme: initial_theme,
            ..Settings::load()
        };
        // NOTE: theme application moved out of construction (see `main.rs`):
        // `Theme::change/update` inside the `open_window` build closure runs
        // mid-window-construction and poisoned first render. `wallpaper` is
        // kept for the post-open apply.
        let _ = wallpaper;

        let opacity_slider = cx.new(|_| {
            SliderState::new()
                .min(MIN_OPACITY)
                .max(100.0)
                .step(1.0)
                .default_value(settings.window_opacity)
        });
        let vibrancy_slider = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(MAX_VIBRANCY)
                .step(1.0)
                .default_value(settings.dynamic_vibrancy)
        });

        // Live repaint on drag (mirrors `onInput` → `apply*`). `Change` fires
        // per pixel of movement, so persistence waits for `Release`: dragging
        // no longer does a synchronous JSON write (plus `create_dir_all`) on
        // the UI thread at frame rate. The value is live in memory throughout,
        // and `Release` fires on mouse-up and on track clicks.
        let sub_opacity = cx.subscribe_in(
            &opacity_slider,
            window,
            |this: &mut Self, _state, event, _window, cx| {
                let value = match event {
                    SliderEvent::Change(value) | SliderEvent::Release(value) => value,
                };
                this.settings.window_opacity = value.end();
                crate::theme::set_surface_opacity(value.end() / 100.0, cx);
                if matches!(event, SliderEvent::Release(_)) {
                    this.settings.save();
                }
                cx.notify();
            },
        );
        let sub_vibrancy = cx.subscribe_in(
            &vibrancy_slider,
            window,
            |this: &mut Self, _state, event, window, cx| {
                let value = match event {
                    SliderEvent::Change(value) | SliderEvent::Release(value) => value,
                };
                this.settings.dynamic_vibrancy = value.end();
                this.repaint_theme(Some(window), cx);
                if matches!(event, SliderEvent::Release(_)) {
                    this.settings.save();
                }
            },
        );

        // The background scans slow down while this window is not in front
        // (see `activity.rs`). Only the main window reports: the game overlay
        // never takes focus and must not count as the app being used.
        let sub_activity = cx.observe_window_activation(window, |this: &mut Self, window, cx| {
            let active = window.is_window_active();
            crate::activity::ACTIVITY.set_active(active);
            if active {
                this.scan_devices(cx);
            }
        });

        let games = games::load_catalog();
        let game_watcher = crate::game_watcher::GameWatcher::start(cx);
        let mut view = Self {
            page: Page::Overview,
            sidebar_collapsed: false,
            settings,
            games,
            devices: Devices::default(),
            hid: None,
            presence: Default::default(),
            profiles: Default::default(),
            profile_editor: None,
            profile_overrides: Vec::new(),
            profile_inflight: Vec::new(),
            profile_device: None,
            toast: None,
            toast_seq: 0,
            crash_report: crate::logging::unseen_report(),
            updates_list: ListState::new(crate::changelog::entries().len() + UPDATES_LEADING_ROWS, ListAlignment::Top, px(400.)),
            update_available: None,
            checking_for_updates: false,
            overlay: None,
            presence_seq: 0,
            startup_error: None,
            discord_error: None,
            badge_hovered: false,
            opacity_slider: Some(opacity_slider),
            vibrancy_slider: Some(vibrancy_slider),
            control_sliders: Default::default(),
            slider_live: Default::default(),
            slider_seq: 0,
            _subscriptions: vec![sub_opacity, sub_vibrancy, sub_activity],
            _game_watcher: Some(game_watcher),
            ascii_rows: Vec::new(),
            ascii_dims: (0, 0),
            theme_seq: 0,
        };
        // The OS entry is the truth for launch-on-startup, not the saved flag.
        view.profiles = crate::profiles::Profiles::load();
        view.settings.launch_on_startup = crate::autostart::is_enabled();
        if view.settings.discord_presence {
            view.set_discord(true, cx);
        }
        view.scan_devices(cx);
        view.watch_devices(cx);
        view
    }

    /// Harness-free constructor for tests (no window/cx for slider entities;
    /// Settings UI paths require `build`).
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for AppView {
    fn default() -> Self {
        Self {
            page: Page::Overview,
            control_sliders: Default::default(),
            slider_live: Default::default(),
            slider_seq: 0,
            sidebar_collapsed: false,
            settings: Settings::default(),
            games: Vec::new(),
            devices: Devices::default(),
            hid: None,
            presence: Default::default(),
            profiles: Default::default(),
            profile_editor: None,
            profile_overrides: Vec::new(),
            profile_inflight: Vec::new(),
            profile_device: None,
            toast: None,
            toast_seq: 0,
            crash_report: None,
            updates_list: ListState::new(crate::changelog::entries().len() + UPDATES_LEADING_ROWS, ListAlignment::Top, px(400.)),
            update_available: None,
            checking_for_updates: false,
            overlay: None,
            presence_seq: 0,
            startup_error: None,
            discord_error: None,
            badge_hovered: false,
            opacity_slider: None,
            vibrancy_slider: None,
            _subscriptions: Vec::new(),
            _game_watcher: None,
            ascii_rows: Vec::new(),
            ascii_dims: (0, 0),
            theme_seq: 0,
        }
    }
}

impl AppView {
    /// What Rich Presence should say right now, from the settings and state:
    /// the game being played (top line) and the mouse and its stats (second line).
    fn presence_text(&self) -> (String, String) {
        let s = &self.settings;
        // Stats come from the first mouse-like connected device (one with a DPI).
        let device = self.connected_devices().into_iter().find(|(_, r)| r.dpi.is_some());
        let name = device
            .as_ref()
            .and_then(|(key, _)| self.devices.tiles.iter().find(|t| &t.key == key))
            .map(|t| t.name.as_str());
        crate::presence_text::compose(
            crate::presence_text::Share { game: s.discord_show_game, device: s.discord_show_device, stats: s.discord_show_stats },
            &crate::presence_text::Facts {
                running_game: self.games.iter().find(|g| g.running).map(|g| g.name.as_str()),
                device_name: name,
                dpi: device.as_ref().and_then(|(_, r)| r.dpi),
                polling_hz: device.as_ref().and_then(|(_, r)| if r.polling_read_only && r.polling_rates.is_empty() { None } else { r.polling_hz }),
            },
        )
    }

    /// Pushes the current presence text to Discord (a moment later, and only the
    /// newest request, because Discord rate-limits activity updates). A no-op
    /// while presence is off or not connected.
    fn refresh_presence(&mut self, cx: &mut Context<Self>) {
        if !self.settings.discord_presence {
            return;
        }
        self.presence_seq += 1;
        let seq = self.presence_seq;
        cx.spawn(async move |view, cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(1500)).await;
            let Ok(Some((presence, text))) = view.update(cx, |v: &mut Self, _| {
                (v.presence_seq == seq && v.settings.discord_presence)
                    .then(|| (v.presence.clone(), v.presence_text()))
            }) else {
                return;
            };
            let (details, state) = text;
            let _ = cx.background_executor().spawn(async move { presence.update_activity(details, state) }).await;
        })
        .detach();
    }

    /// Connects or disconnects Discord Rich Presence off the UI thread. A
    /// failure (Discord not running) is shown under the switch; the preference
    /// stays on, as in the Tauri app, so it connects next time Discord is up.
    fn set_discord(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.discord_error = None;
        let presence = self.presence.clone();
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    if enabled { presence.enable() } else { presence.disable() }
                })
                .await;
            let _ = view.update(cx, |v: &mut Self, cx| {
                let connected = enabled && result.is_ok();
                v.discord_error = result.err().map(|error| {
                    if enabled {
                        format!("Discord Rich Presence is on but unavailable: {error}. Is Discord running?")
                    } else {
                        format!("Could not disconnect from Discord: {error}")
                    }
                });
                if connected {
                    v.refresh_presence(cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Writes or removes the OS launch-on-login entry and reflects the real
    /// outcome in the switch.
    fn set_launch_on_startup(&mut self, enabled: bool) {
        let result = if enabled { crate::autostart::enable() } else { crate::autostart::disable() };
        self.startup_error = result.err();
        self.settings.launch_on_startup = crate::autostart::is_enabled();
    }

    fn show_toast(&mut self, text: impl Into<String>, error: bool, cx: &mut Context<Self>) {
        self.toast_seq += 1;
        let id = self.toast_seq;
        self.toast = Some(Toast { id, text: text.into(), error });
        cx.notify();
        cx.spawn(async move |view, cx| {
            cx.background_executor().timer(std::time::Duration::from_secs(4)).await;
            let _ = view.update(cx, |v: &mut Self, cx| {
                if v.toast.as_ref().is_some_and(|t| t.id == id) {
                    v.toast = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// A message about a game: on the screen overlay (the game has focus, so
    /// the main window is not where the player is looking) or, for something
    /// they did in this window, as an in-app toast.
    fn announce(&mut self, text: String, error: bool, overlay: bool, cx: &mut Context<Self>) {
        if overlay {
            // Off means off: nothing on the screen.
            if self.settings.overlay_enabled {
                let corner = self.settings.overlay_corner;
                self.overlay = crate::overlay::show(cx, self.overlay.take(), text, error, corner);
                if self.settings.overlay_sound {
                    crate::audio::play(if error { crate::audio::Chime::Error } else { crate::audio::Chime::Success });
                }
            }
        } else {
            self.show_toast(text, error, cx);
        }
    }

    // ── game profiles ────────────────────────────────────────────────────

    /// Current page, for tests and owners that drive navigation.
    pub fn page(&self) -> Page {
        self.page
    }

    /// Switch page programmatically; mirrors sidebar `on_click`.
    pub fn set_page(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        cx.notify();
    }

    /// Current theme choice.
    pub fn theme(&self) -> ThemeChoice {
        self.settings.theme
    }

    fn render_page(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.page == Page::Settings {
            return self.render_settings(window, cx);
        }
        if self.page == Page::Updates {
            return self.render_updates(cx);
        }
        if self.page == Page::Games {
            if let Some(id) = self.profile_editor.clone() {
                return self.render_profile_editor(&id, cx);
            }
            return self.render_games(cx);
        }
        if self.page == Page::Overview {
            let size = window.viewport_size();
            return self.render_overview(size.width.into(), size.height.into(), cx);
        }
        let (title, body): (&str, &str) = match self.page {
            Page::Supported => ("Supported mice", "Supported device catalog lives here."),
            _ => unreachable!(),
        };
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .gap_2()
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().foreground)
                    .child(title),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(body),
            )
            .into_any_element()
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_control_sliders(window, cx);
        // Kit's Root paints the translucent semantic background once.
        // Keep the app shell clear so overlapping layers do not compound alpha.
        let clear = Hsla {
            a: 0.0,
            ..cx.theme().background
        };
        let toast = self.render_toast(cx);
        let ascii = self.render_ascii_background(window, cx);
        let crash_notice = self.render_crash_notice(cx);
        // The sidebar's tint and divider run the window's full height, behind
        // the title bar too; the sidebar itself is transparent over it.
        let sidebar_backdrop = div()
            .absolute()
            .top(px(0.))
            .left(px(0.))
            .h_full()
            .w(px(if self.sidebar_collapsed { SIDEBAR_COLLAPSED_W } else { SIDEBAR_W }))
            .border_r_1()
            .border_color(Hsla { a: 0.7, ..cx.theme().border })
            .bg(Hsla { a: 0.35, ..cx.theme().sidebar });
        div().relative().size_full().children(toast).child(ascii).child(sidebar_backdrop).child(
            div()
                .flex()
                .flex_col()
                .size_full()
                .text_color(cx.theme().foreground)
                .child(TitleBar::new().bg(clear).border_b_0().child(self.render_titlebar(cx)))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_1()
                        .min_h_0()
                        .child(self.render_sidebar(cx))
                        .child(
                            div()
                                .flex()
                                .flex_1()
                                .min_w_0()
                                .child(self.render_page(window, cx)),
                        ),
                ),
        )
        .children(crash_notice)
    }
}
