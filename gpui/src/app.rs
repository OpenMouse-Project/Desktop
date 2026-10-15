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
use gpui_kit::component::{h_flex, ActiveTheme as _, Collapsible as _, Sizable as _, TitleBar};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::time::Duration;
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Release stage label shown in the titlebar badge, mirroring
/// `stageForVersion` in `src/lib/version-stage.ts`: 0.x → Alpha,
/// 1.x → Beta, 2.x+ → Public.
fn release_stage(version: &str) -> &'static str {
    let major = version
        .trim_start_matches('v')
        .split('.')
        .next()
        .unwrap_or("0")
        .parse::<u64>()
        .unwrap_or(0);
    if major == 0 {
        "Alpha"
    } else if major == 1 {
        "Beta"
    } else {
        "Public"
    }
}

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

/// While a game's profile is in control: what to put back when it closes.
struct ProfileOverride {
    game_id: String,
    game_name: String,
    /// The device the profile was applied to (and will be restored on).
    device: String,
    restore: crate::profiles::GameProfile,
}

/// Overview grid sizing: the smallest card width a column may shrink to, and
/// the layout constants the column count is estimated from.
const MIN_CARD_W: f32 = 320.0;
const GRID_GAP: f32 = 16.0;
const PAGE_PAD: f32 = 16.0;
const SIDEBAR_W: f32 = 264.0;
const SIDEBAR_COLLAPSED_W: f32 = 72.0;

/// The community server, the same invite openmouse.app links to.
const DISCORD_INVITE_URL: &str = "https://discord.gg/yxC9jzMdw6";

/// The Discord glyph (drawn in the theme colour by `themed_discord`), embedded so
/// there is no file to ship or find at runtime.
static DISCORD_SVG: &str = include_str!("../assets/discord.svg");


/// Rows above the releases on the Updates page: heading, version card, notes title.
const UPDATES_LEADING_ROWS: usize = 3;

/// The Discord glyph drawn in `color`, so it follows the theme like the other
/// sidebar icons instead of being a fixed brand blue. One image per colour.
fn themed_discord(color: Hsla) -> std::sync::Arc<Image> {
    static CACHE: std::sync::Mutex<Vec<(u32, std::sync::Arc<Image>)>> = std::sync::Mutex::new(Vec::new());
    let rgba: Rgba = color.into();
    let hex = ((rgba.r * 255.0).round() as u32) << 16 | ((rgba.g * 255.0).round() as u32) << 8 | (rgba.b * 255.0).round() as u32;
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, image)) = cache.iter().find(|(h, _)| *h == hex) {
        return image.clone();
    }
    let svg = DISCORD_SVG.replace("#5865F2", &format!("#{hex:06X}"));
    let image = std::sync::Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes()));
    cache.push((hex, image.clone()));
    image
}

/// The selected device's detail card (battery bar, DPI, polling rate,
/// firmware) under the grid. Hidden until per-device menus exist to hold it.
const SHOW_DEVICE_DETAILS: bool = false;

/// Overview device state, driven by `crate::hid` (the shared protocol
/// drivers; see that module for how).
#[derive(Default)]
struct Devices {
    scanning: bool,
    error: Option<String>,
    tiles: Vec<Tile>,
    selected: usize,
    /// Key of the device whose settings page is open, if any.
    open: Option<String>,
    /// Edits made on that page and not yet written (instant apply off).
    staged: Staged,
    /// X and Y DPI edited separately (mice with separate axes); linked by default,
    /// since a single number on screen hides a differing Y.
    unlinked_axes: bool,
    tab: DeviceTab,
    /// Stored profiles of the open Logitech device, once read (on demand).
    onboard: Option<crate::hid::OnboardProfiles>,
    onboard_loading: bool,
    /// The long-list control whose options are currently unfolded.
    expanded: Option<String>,
    /// Label of the write in flight (disables further edits meanwhile).
    writing: Option<String>,
    write_error: Option<String>,
}

/// Pending, unwritten lighting edits (each `None` field is untouched).
#[derive(Default, Clone, PartialEq)]
struct LightEdit {
    mode: Option<String>,
    color: Option<String>,
    color2: Option<String>,
    speed: Option<f64>,
}

/// Pending, unwritten edits on a device page.
#[derive(Default, Clone, PartialEq)]
struct Staged {
    dpi: Option<f64>,
    /// Y-axis DPI, only while the axes are unlinked.
    dpi_y: Option<f64>,
    polling: Option<u32>,
    lod: Option<String>,
    surface: Option<String>,
    light: LightEdit,
}

impl Staged {
    fn is_empty(&self) -> bool {
        *self == Staged::default()
    }
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum DeviceTab {
    #[default]
    Performance,
    Buttons,
    Lighting,
    Advanced,
    /// The mouse's own stored profiles (Logitech).
    Profiles,
}

impl DeviceTab {
    fn label(self) -> &'static str {
        match self {
            DeviceTab::Performance => "Performance",
            DeviceTab::Buttons => "Buttons",
            DeviceTab::Lighting => "Lighting",
            DeviceTab::Advanced => "Advanced",
            DeviceTab::Profiles => "Profiles",
        }
    }
}

/// A small status badge: a faint accent-tinted pill with an accent dot and the
/// theme's own text colour. (The toolkit's `Tag::success()` is a fixed green
/// that ignores the palette and reads poorly on the Dynamic theme; coloured
/// text on a tint of itself falls under 4.5:1 on some wallpapers.)
fn accent_badge(text: impl Into<SharedString>, accent: Hsla, ink: Hsla) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_1p5()
        .px_2()
        .py_0p5()
        .rounded_full()
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .bg(Hsla { a: 0.20, ..accent })
        .text_color(ink)
        .child(div().size(px(6.)).flex_none().rounded_full().bg(accent))
        .child(text.into())
}

/// Swatches offered for lighting colours.
const LIGHT_SWATCHES: [&str; 14] = [
    "#ff0000", "#ff6a00", "#ffc800", "#a6ff00", "#00ff40", "#00ffd0", "#00aaff",
    "#0040ff", "#6a00ff", "#c800ff", "#ff00a0", "#ffffff", "#9aa0a6", "#000000",
];

fn hex_color(hex: &str) -> Hsla {
    let h = hex.trim_start_matches('#');
    let v = u32::from_str_radix(h, 16).unwrap_or(0x888888);
    rgb(v).into()
}

struct Tile {
    key: String,
    name: String,
    /// The registry's label for the driver; what gating and write routing use.
    brand: String,
    /// The brand to show (the product's own — see `ConnectedDevice::display_brand`).
    display_brand: String,
    art: Option<LoadedArt>,
    status: TileStatus,
}

struct LoadedArt {
    path: std::path::PathBuf,
    placement: Option<crate::hid::ArtPlacement>,
}

enum TileStatus {
    Idle,
    Connecting,
    Connected(DeviceReadout),
    Failed(String),
}

struct DeviceReadout {
    /// Y-axis DPI, when the mouse reports one (it can differ from `dpi`).
    dpi_y: Option<f64>,
    /// The mouse can hold X and Y sensitivity apart.
    separate_axes: bool,
    /// The driver's own UI hints (`MouseUiHints`): what is meaningful for this
    /// protocol generation.
    polling_read_only: bool,
    hide_lod_low: bool,
    lod_requires_surface: bool,
    /// A Logitech mouse with stored profiles the library can read.
    onboard_profiles: bool,
    /// `Onboard`, `Host` or `Unknown` (Logitech).
    mode: Option<String>,
    tabs: crate::hid::Tabs,
    controls: Option<crate::hid::Controls>,
    polling_rates: Vec<u32>,
    lod: Option<String>,
    lod_options: Vec<String>,
    surface: Option<String>,
    battery: Option<f64>,
    charging: bool,
    dpi: Option<f64>,
    polling_hz: Option<f64>,
    connection: Option<String>,
    firmware: Vec<String>,
}

impl DeviceReadout {
    fn from_status(s: &serde_json::Value, controls: Option<crate::hid::Controls>, tabs: crate::hid::Tabs) -> Self {
        let num = |k: &str| s.get(k).and_then(|v| v.as_f64());
        let strings = |k: &str| -> Vec<String> {
            s.get(k)
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                .unwrap_or_default()
        };
        let ui_flag = |k: &str| s.get("ui").and_then(|u| u.get(k)).and_then(|v| v.as_bool()).unwrap_or(false);
        Self {
            dpi_y: num("dpiY"),
            separate_axes: s.get("supportsSeparateDpiAxes").and_then(|v| v.as_bool()).unwrap_or(false),
            polling_read_only: ui_flag("pollingReadOnly"),
            hide_lod_low: ui_flag("hideLodLow"),
            lod_requires_surface: ui_flag("lodRequiresSurface"),
            onboard_profiles: s
                .get("onboardProfileFormat")
                .and_then(|f| f.get("supported"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
                && s.get("brand").and_then(|b| b.as_str()) == Some("Logitech"),
            mode: s.get("deviceMode").and_then(|v| v.as_str()).map(str::to_string),
            tabs,
            controls,
            polling_rates: s
                .get("supportedPollingRates")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_u64().map(|n| n as u32)).collect())
                .unwrap_or_default(),
            lod: s.get("liftOffDistance").and_then(|v| v.as_str()).map(str::to_string),
            lod_options: strings("supportedLiftOffDistances"),
            surface: s.get("gamingSurfaceMode").and_then(|v| v.as_str()).map(str::to_string),
            battery: num("batteryPercent"),
            charging: s
                .get("batteryState")
                .and_then(|v| v.as_str())
                .is_some_and(|b| b.starts_with("Charging")),
            dpi: num("dpi"),
            polling_hz: num("pollingRateHz"),
            connection: s.get("connectionType").and_then(|v| v.as_str()).map(str::to_string),
            firmware: s
                .get("firmware")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                .unwrap_or_default(),
        }
    }
}

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
    /// The Updates page's virtualized list (only the rows on screen are built).
    updates_list: ListState,
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
}

/// Theme swatches in Settings, in display order. Narrows the web app's seven
/// presets to the four in scope: System follow + Matt dark/light + Dynamic.
/// (Emerald/Violet/Ice/Mono + custom CSS stay web-only for now.)
const THEME_CHOICES: [ThemeChoice; 4] = [
    ThemeChoice::System,
    ThemeChoice::Dark,
    ThemeChoice::Light,
    ThemeChoice::Dynamic,
];

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

        // Live repaint on drag (mirrors `onInput` → `apply*`); persistence
        // happens immediately too — the slider is the setting's own state,
        // and `Settings::save` is a small synchronous file write.
        let sub_opacity = cx.subscribe_in(
            &opacity_slider,
            window,
            |this: &mut Self, _state, event, _window, cx| {
                if let SliderEvent::Change(value) = event {
                    this.settings.window_opacity = value.end();
                    crate::theme::set_surface_opacity(value.end() / 100.0, cx);
                    this.settings.save();
                    cx.notify();
                }
            },
        );
        let sub_vibrancy = cx.subscribe_in(
            &vibrancy_slider,
            window,
            |this: &mut Self, _state, event, window, cx| {
                if let SliderEvent::Change(value) = event {
                    this.settings.dynamic_vibrancy = value.end();
                    this.repaint_theme(Some(window), cx);
                    this.settings.save();
                }
            },
        );

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
            updates_list: ListState::new(crate::changelog::entries().len() + UPDATES_LEADING_ROWS, ListAlignment::Top, px(400.)),
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
            _subscriptions: vec![sub_opacity, sub_vibrancy],
            _game_watcher: Some(game_watcher),
        };
        // The OS entry is the truth for launch-on-startup, not the saved flag.
        view.profiles = crate::profiles::Profiles::load();
        view.settings.launch_on_startup = crate::autostart::is_enabled();
        if view.settings.discord_presence {
            view.set_discord(true, cx);
        }
        view.scan_devices(cx);
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
            updates_list: ListState::new(crate::changelog::entries().len() + UPDATES_LEADING_ROWS, ListAlignment::Top, px(400.)),
            overlay: None,
            presence_seq: 0,
            startup_error: None,
            discord_error: None,
            badge_hovered: false,
            opacity_slider: None,
            vibrancy_slider: None,
            _subscriptions: Vec::new(),
            _game_watcher: None,
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

    fn hid_service(&mut self) -> crate::hid::HidService {
        self.hid.get_or_insert_with(crate::hid::HidService::start).clone()
    }

    /// Lists supported devices, loads their artwork, and connects the first.
    /// Everything blocking runs on `crate::hid`'s threads or the background
    /// executor.
    fn scan_devices(&mut self, cx: &mut Context<Self>) {
        if self.devices.scanning {
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
                cx.notify();
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

    async fn load_art(
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
    fn open_device(&mut self, key: &str, cx: &mut Context<Self>) {
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
    fn patch_dpi_y(&mut self, key: &str, y: f64) {
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
    fn patch_readout(&mut self, key: &str, action: &str, applied: &serde_json::Value) {
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
    fn load_onboard_profiles(&mut self, cx: &mut Context<Self>) {
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
    fn onboard_action(&mut self, action: &'static str, value: serde_json::Value, cx: &mut Context<Self>) {
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
    fn set_control(&mut self, id: String, value: serde_json::Value, cx: &mut Context<Self>) {
        if self.devices.writing.is_some() || (id.starts_with("perf.") && self.blocked_by_profile(cx)) {
            return;
        }
        let Some(tile) = self.devices.tiles.get(self.devices.selected) else { return };
        let (key, brand) = (tile.key.clone(), tile.brand.clone());
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
    fn effective_lighting(&self, spec: &crate::hid::LightingSpec) -> serde_json::Value {
        let e = &self.devices.staged.light;
        serde_json::json!({
            "mode": e.mode.clone().or_else(|| spec.mode.clone()),
            "color": e.color.clone().or_else(|| spec.color.clone()),
            "color2": e.color2.clone().or_else(|| spec.color2.clone()),
            "speed": e.speed.or(spec.speed),
        })
    }

    /// One lighting edit: written now with instant apply, staged otherwise.
    fn edit_lighting(
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
    fn write_setting(&mut self, action: &'static str, value: serde_json::Value, cx: &mut Context<Self>) {
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
    fn apply_staged(&mut self, cx: &mut Context<Self>) {
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
    fn locked_game(&self) -> Option<String> {
        let key = &self.devices.tiles.get(self.devices.selected)?.key;
        self.profile_overrides.iter().find(|o| &o.device == key).map(|o| o.game_name.clone())
    }

    /// Refuses an edit with a note when [`Self::locked_game`] applies.
    fn blocked_by_profile(&mut self, cx: &mut Context<Self>) -> bool {
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
    fn edit_dpi(&mut self, x: f64, y: Option<f64>, cx: &mut Context<Self>) {
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
    fn edit(&mut self, action: &'static str, value: serde_json::Value, cx: &mut Context<Self>) {
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
    fn connect_tile(&mut self, key: &str, quiet: bool, cx: &mut Context<Self>) {
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

    /// Every connected device with its readout, in list order.
    fn connected_devices(&self) -> Vec<(String, &DeviceReadout)> {
        self.devices
            .tiles
            .iter()
            .filter_map(|t| match &t.status {
                TileStatus::Connected(r) => Some((t.key.clone(), r)),
                _ => None,
            })
            .collect()
    }

    /// The device that inherits a profile saved before profiles were per-device:
    /// the first mouse-like (has a DPI) connected one.
    fn legacy_heir(&self) -> Option<String> {
        self.connected_devices().into_iter().find(|(_, r)| r.dpi.is_some()).map(|(k, _)| k)
    }

    /// The profile a game has for this device (its own, or the inherited legacy one).
    fn profile_for(&self, game_id: &str, device: &str) -> Option<crate::profiles::GameProfile> {
        let heir = self.legacy_heir().as_deref() == Some(device);
        self.profiles.effective(game_id, device, heir).cloned()
    }

    fn game_name(&self, id: &str) -> String {
        self.games.iter().find(|g| g.id == id).map(|g| g.name.clone()).unwrap_or_else(|| id.to_string())
    }

    /// A game launched: announce profiles that don't auto-apply, and apply the
    /// ones that do.
    fn on_game_started(&mut self, id: &str, cx: &mut Context<Self>) {
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
    fn sync_running_profiles(&mut self, cx: &mut Context<Self>) {
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
    fn on_game_closed(&mut self, id: &str, cx: &mut Context<Self>) {
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
    fn apply_profile(&mut self, game_id: &str, device: &str, profile: crate::profiles::GameProfile, auto: bool, cx: &mut Context<Self>) {
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

    fn apply_profile_inner(
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
    fn edit_profile(&mut self, game_id: &str, device: &str, change: impl FnOnce(&mut crate::profiles::GameProfile), cx: &mut Context<Self>) {
        let mut profile = self.profile_for(game_id, device).unwrap_or_default();
        change(&mut profile);
        self.profiles.set(game_id, device, profile);
        self.profiles.save();
        cx.notify();
    }

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

    /// Switch theme; mirrors the Settings theme switcher. Paints through
    /// [`crate::theme::apply_choice`] so every window refreshes. Switching
    /// to Dynamic resamples the wallpaper immediately (mirrors Settings'
    /// `force` refresh) instead of showing the fallback until the 2s watcher
    /// tick lands.
    pub fn set_theme(&mut self, theme: ThemeChoice, window: &mut Window, cx: &mut Context<Self>) {
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
        crate::theme::apply_choice(
            ThemeChoice::Dynamic,
            Some(accent),
            self.settings.dynamic_vibrancy,
            self.settings.window_opacity / 100.0,
            None,
            cx,
        );
        cx.notify();
    }

    /// Repaint the active theme from current settings (theme + vibrancy).
    fn repaint_theme(&self, window: Option<&mut Window>, cx: &mut App) {
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
            window,
            cx,
        );
    }

    /// Custom titlebar content mirroring `TitleBar.tsx`: app name plus a
    /// version·stage badge. Dragging, double-click zoom, and the
    /// min/max/close buttons stay owned by Kit's `TitleBar`; this only
    /// supplies the branded row inside it.
    /// Updates: this version, and the release notes as a timeline (version and
    /// date on the left, changes grouped under New / Improved / Fixed on the
    /// right). One virtualized list, so only the rows on screen are built — the
    /// page used to rebuild every release on each scroll tick and pinned a core.
    /// The version card is UI only for now: update checks aren't wired up yet.
    fn render_updates(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = PageColors {
            fg: cx.theme().foreground,
            muted: cx.theme().muted_foreground,
            accent: cx.theme().primary,
            border: Hsla { a: 0.7, ..cx.theme().border },
        };
        let notify = cx.listener(|this, _, _, cx| {
            this.show_toast("Update checks aren't available yet.", false, cx);
        });
        let check_button = Button::new("check-updates").compact().label("Check for updates").on_click(notify).into_any_element();
        // The click handler lives on the card's button, built here once per render
        // of the page (cheap); the list rows are built lazily below.
        let mut check_button = Some(check_button);

        let entries = crate::changelog::entries();
        let list_state = self.updates_list.clone();
        let stage = release_stage(APP_VERSION);

        list(list_state, move |ix, _window, _cx| {
            let c = colors;
            let body: AnyElement = match ix {
                0 => div()
                    .flex()
                    .flex_col()
                    .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).text_color(c.fg).child("Updates"))
                    .child(div().text_sm().text_color(c.muted).child("Your version and what's changed"))
                    .into_any_element(),
                1 => h_flex()
                    .gap_4()
                    .items_center()
                    .justify_between()
                    .p_5()
                    .mt_4()
                    .rounded_xl()
                    .border_1()
                    .border_color(c.border)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_xs().text_color(c.muted).child("OpenMouse Desktop"))
                            .child(
                                h_flex()
                                    .gap_3()
                                    .items_center()
                                    .child(div().text_2xl().font_weight(FontWeight::BOLD).text_color(c.fg).child(format!("v{APP_VERSION}")))
                                    .child(accent_badge(stage.to_string(), c.accent, c.fg)),
                            )
                            .child(div().text_sm().text_color(c.muted).child("You're running this version.")),
                    )
                    .children(check_button.take())
                    .into_any_element(),
                2 => div().pt_6().pb_1().text_base().font_weight(FontWeight::SEMIBOLD).text_color(c.fg).child("Release notes").into_any_element(),
                n => match entries.get(n - UPDATES_LEADING_ROWS) {
                    Some(entry) => release_row(entry, n > UPDATES_LEADING_ROWS, entry.version == APP_VERSION, c),
                    None => div().into_any_element(),
                },
            };
            div().w_full().px_4().pt_2().child(div().max_w(px(880.)).child(body)).into_any_element()
        })
        .size_full()
        .into_any_element()
    }

    /// One nav row: icon and label, a tinted fill and an accent bar when active.
    fn nav_row(&self, id: &'static str, icon: IconName, label: &str, active: bool, collapsed: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let accent = cx.theme().primary;
        div()
            .id(id)
            .relative()
            .cursor_pointer()
            .flex()
            .flex_row()
            .items_center()
            .when(collapsed, |row| row.justify_center())
            .gap_3()
            .h(px(40.))
            .px_3()
            .rounded_lg()
            .text_sm()
            .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
            .text_color(if active { fg } else { muted })
            .when(active, |row| row.bg(Hsla { a: 0.16, ..accent }))
            .when(!active, |row| row.hover(|h| h.bg(Hsla { a: 0.07, ..fg }).text_color(fg)))
            .when(active, |row| {
                row.child(
                    div()
                        .absolute()
                        .left(px(-6.))
                        .top(px(10.))
                        .w(px(3.))
                        .h(px(20.))
                        .rounded_full()
                        .bg(accent),
                )
            })
            .child(Icon::new(icon).size(px(18.)).text_color(if active { accent } else { muted }))
            .when(!collapsed, |row| row.child(label.to_string()))
    }

    /// A footer row (Help & Feedback, Settings): an icon, a title with an optional
    /// subtitle, and a chevron.
    fn footer_row(
        &self,
        id: &'static str,
        glyph: AnyElement,
        title: &str,
        subtitle: Option<&str>,
        active: bool,
        collapsed: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        div()
            .id(id)
            .cursor_pointer()
            .flex()
            .flex_row()
            .items_center()
            .when(collapsed, |row| row.justify_center())
            .gap_3()
            .px_3()
            .py_2p5()
            .when(active, |row| row.bg(Hsla { a: 0.14, ..cx.theme().primary }))
            .when(!active, |row| row.hover(|h| h.bg(Hsla { a: 0.06, ..fg })))
            .child(div().w(px(22.)).flex_none().flex().items_center().justify_center().child(glyph))
            .when(!collapsed, |row| {
                row.child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(if active { fg } else { muted }).child(title.to_string()))
                        .children(subtitle.map(|s| div().text_xs().text_color(muted).truncate().child(s.to_string()))),
                )
                .child(Icon::new(IconName::ChevronRight).size(px(14.)).text_color(muted))
            })
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let collapsed = self.sidebar_collapsed;
        let fg = cx.theme().foreground;
        let border = cx.theme().border;
        let accent = cx.theme().primary;

        let mut nav = div().flex().flex_col().gap_1();
        for (id, page) in [("nav-overview", Page::Overview), ("nav-games", Page::Games), ("nav-supported", Page::Supported)] {
            nav = nav.child(self.nav_row(id, page.icon(), page.label(), self.page == page, collapsed, cx).on_click(
                cx.listener(move |this, _, _, cx| {
                    // Choosing the page you are on goes back to its top level.
                    match page {
                        Page::Overview => this.devices.open = None,
                        Page::Games => this.profile_editor = None,
                        _ => {}
                    }
                    this.page = page;
                    cx.notify();
                }),
            ));
        }

        let discord = img(themed_discord(cx.theme().muted_foreground)).w(px(22.)).h(px(17.)).into_any_element();
        let on_updates = self.page == Page::Updates;
        let on_settings = self.page == Page::Settings;
        let tone = |active: bool| if active { accent } else { cx.theme().muted_foreground };
        let updates_icon = Icon::new(Page::Updates.icon()).size(px(18.)).text_color(tone(on_updates)).into_any_element();
        let gear = Icon::new(Page::Settings.icon()).size(px(18.)).text_color(tone(on_settings)).into_any_element();
        let footer = div()
            .flex()
            .flex_col()
            .rounded_xl()
            .border_1()
            .border_color(Hsla { a: 0.8, ..border })
            .bg(Hsla { a: 0.04, ..fg })
            .overflow_hidden()
            .child(
                self.footer_row("footer-updates", updates_icon, "Updates", Some(&format!("v{APP_VERSION} · Release notes")), on_updates, collapsed, cx)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.page = Page::Updates;
                        cx.notify();
                    })),
            )
            .child(div().h(px(1.)).w_full().bg(Hsla { a: 0.8, ..border }))
            .child(
                self.footer_row("footer-discord", discord, "Help & Feedback", Some("Join the community server"), false, collapsed, cx)
                    .on_click(cx.listener(|_, _, _, cx| cx.open_url(DISCORD_INVITE_URL))),
            )
            .child(div().h(px(1.)).w_full().bg(Hsla { a: 0.8, ..border }))
            .child(
                self.footer_row("footer-settings", gear, Page::Settings.label(), None, self.page == Page::Settings, collapsed, cx)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.page = Page::Settings;
                        cx.notify();
                    })),
            );
        let _ = accent;

        div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(if collapsed { SIDEBAR_COLLAPSED_W } else { SIDEBAR_W }))
            .h_full()
            .px_3()
            .pt_3()
            .pb_3()
            .gap_6()
            .border_r_1()
            .border_color(Hsla { a: 0.7, ..border })
            .bg(Hsla { a: 0.35, ..cx.theme().sidebar })
            .child(nav)
            .child(div().flex_1())
            .child(footer)
    }

    fn render_titlebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Badge fade mirrors the theme-transition feel: 380ms in `App.css`
        // is the full-theme swap; a hover reveal reads better faster.
        const BADGE_FADE: Duration = Duration::from_millis(160);
        let hovered = self.badge_hovered;
        let collapsed = self.sidebar_collapsed;
        // The sidebar toggle lives here, just after the window buttons.
        let toggle = Button::new("toggle-sidebar")
            .ghost()
            .compact()
            .icon(IconName::Menu)
            .accessibility_label(if collapsed { "Expand sidebar" } else { "Collapse sidebar" })
            .tooltip(if collapsed { "Expand sidebar" } else { "Collapse sidebar" })
            .on_click(cx.listener(|this, _, _, cx| {
                this.sidebar_collapsed = !this.sidebar_collapsed;
                cx.notify();
            }));
        let brand = h_flex()
            .id("title-brand")
            .items_center()
            .gap_2()
            .px_1()
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.badge_hovered = *hovered;
                cx.notify();
            }))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().foreground)
                    .child("OpenMouse Desktop"),
            )
            .child(
                div()
                    .text_xs()
                    .px_1p5()
                    .py_0p5()
                    .rounded_md()
                    .bg(cx.theme().muted)
                    .text_color(cx.theme().muted_foreground)
                    .when(!hovered, |this| this.invisible())
                    .with_animation(
                        ElementId::NamedInteger("title-badge".into(), hovered as u64),
                        Animation::new(BADGE_FADE)
                            .with_easing(gpui_kit::base::animation::ease_out_cubic),
                        move |this, delta| {
                            // Fade + slight rise on enter, reversed on exit
                            // via the keyed id retargeting the animation.
                            let progress = if hovered { delta } else { 1.0 - delta };
                            this.opacity(progress).top(px(-4.0 * (1.0 - progress)))
                        },
                    )
                    .child(format!("v{APP_VERSION} · {}", release_stage(APP_VERSION))),
            );
        h_flex().items_center().gap_1().child(toggle).child(brand)
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

    /// Mouse artwork in a `side` x `side` box. Placement is in fractions of
    /// the box (the Tauri app's `deviceArtStyle` normalisation, so every
    /// model's subject comes out a comparable size); `zoom` scales that up
    /// where the art should dominate.
    fn device_art(&self, tile: &Tile, side: f32, zoom: f32) -> Div {
        let frame = div().relative().flex_none().w(px(side)).h(px(side)).overflow_hidden();
        let Some(art) = &tile.art else { return frame };
        let image = img(art.path.clone());
        match &art.placement {
            Some(p) => {
                let h = p.height / 100.0 * zoom;
                let w = h * p.aspect;
                let left = 0.5 - w / 2.0 + p.offset_x / 100.0 * w;
                let top = 0.5 - h / 2.0 + p.offset_y / 100.0 * h;
                frame.child(
                    image
                        .absolute()
                        .left(relative(left))
                        .top(relative(top))
                        .w(relative(w))
                        .h(relative(h))
                        .object_fit(ObjectFit::Fill),
                )
            }
            None => frame.child(image.size_full().object_fit(ObjectFit::Contain)),
        }
    }

    /// Selectable pill used for the page's option lists.
    fn chip(
        &self,
        id: String,
        label: String,
        selected: bool,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        let primary = cx.theme().primary;
        div()
            .id(SharedString::from(id))
            .cursor_pointer()
            .px_3()
            .py_1p5()
            .rounded_md()
            .border_1()
            .text_sm()
            .border_color(if selected { primary } else { cx.theme().border })
            .when(selected, |this| this.bg(Hsla { a: 0.14, ..primary }))
            .text_color(if selected { cx.theme().foreground } else { cx.theme().muted_foreground })
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| on_click(this, cx)))
    }

    /// Creates the sliders the selected device's number controls need. The write
    /// waits until the drag pauses, so a sweep sends one command, not hundreds.
    fn ensure_control_sliders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
            .filter(|c| c.kind == "number")
            .cloned()
            .collect();
        for c in specs {
            let key = format!("{device}/{}", c.id);
            if self.control_sliders.contains_key(&key) {
                continue;
            }
            let (min, max) = (c.min.unwrap_or(0.0), c.max.unwrap_or(255.0));
            let step = c.step.unwrap_or(1.0);
            let start = c.current.as_f64().unwrap_or(min).clamp(min, max);
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
                cx.notify();
                cx.spawn(async move |view, cx| {
                    cx.background_executor().timer(std::time::Duration::from_millis(350)).await;
                    let _ = view.update(cx, |this: &mut Self, cx| {
                        if this.slider_seq == seq {
                            this.set_control(id, serde_json::json!(v), cx);
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
    fn render_control(&self, c: &crate::hid::ControlSpec, cx: &mut Context<Self>) -> AnyElement {
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
                h_flex()
                    .gap_3()
                    .items_center()
                    .children(slider.map(|s| div().w(px(200.)).child(Slider::new(&s))))
                    .child(div().min_w(px(76.)).text_right().text_color(fg).child(match (&c.zero_label, now <= c.min.unwrap_or(0.0)) {
                        (Some(label), true) => label.clone(),
                        _ => format!("{now:.places$}{unit}"),
                    }))
                    .into_any_element()
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
    fn render_logitech_buttons(&self, r: &DeviceReadout, cx: &mut Context<Self>) -> AnyElement {
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
    fn render_onboard_profiles(&self, r: &DeviceReadout, cx: &mut Context<Self>) -> AnyElement {
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
    fn render_extra_tab(&self, r: &DeviceReadout, cx: &mut Context<Self>) -> AnyElement {
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
            for m in spec.modes.iter().cloned() {
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

    /// Per-device settings: sensitivity, polling rate, lift-off distance and
    /// gaming-surface mode, written through the shared protocol drivers.
    /// With instant apply off, edits are staged and the bottom bar applies them.
    fn render_device_page(&self, tile: &Tile, view_width: f32, view_height: f32, cx: &mut Context<Self>) -> AnyElement {
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

    /// Overview: the connected mouse and its artwork, read through the shared
    /// protocol drivers (`crate::hid`).
    fn render_overview(&self, view_width: f32, view_height: f32, cx: &mut Context<Self>) -> AnyElement {
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
                        1 => "1 mouse found".to_string(),
                        n => format!("{n} mice found"),
                    })),
            )
            .child(
                Button::new("rescan")
                    .ghost()
                    .compact()
                    .icon(IconName::Redo)
                    .label(if devices.scanning { "Scanning…" } else { "Rescan" })
                    .on_click(cx.listener(|this, _, _, cx| this.scan_devices(cx))),
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
                (None, false) => "No supported mouse found. Plug one in and press Rescan.".to_string(),
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

    /// The device the profile screen is editing: the one picked, else the first
    /// connected, else the first listed.
    fn profile_target(&self) -> Option<usize> {
        let tiles = &self.devices.tiles;
        self.profile_device
            .as_ref()
            .and_then(|key| tiles.iter().position(|t| &t.key == key))
            .or_else(|| tiles.iter().position(|t| matches!(t.status, TileStatus::Connected(_))))
            .or(if tiles.is_empty() { None } else { Some(0) })
    }

    /// One game's device profile: pick a device, then which of its settings the
    /// profile overrides, whether it applies on launch, and Apply now / Clear.
    fn render_profile_editor(&self, game_id: &str, cx: &mut Context<Self>) -> AnyElement {
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
                        for option in r.lod_options.iter().cloned() {
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
                        for mode in c.surface_modes.iter().cloned() {
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

    /// The bottom-of-window message, if one is showing.
    fn render_toast(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let toast = self.toast.as_ref()?;
        let accent = if toast.error { cx.theme().danger } else { cx.theme().primary };
        Some(
            div()
                .absolute()
                .bottom(px(24.))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(
                    div()
                        .max_w(px(560.))
                        .px_4()
                        .py_3()
                        .rounded_lg()
                        .border_1()
                        .border_color(accent)
                        .bg(cx.theme().sidebar)
                        .shadow_lg()
                        .text_sm()
                        .text_color(cx.theme().foreground)
                        .child(toast.text.clone()),
                )
                .into_any_element(),
        )
    }

    /// Running games without a launcher manifest share the local games section.
    /// Cached art falls back to the name-initial tile until prefetch completes.
    fn render_games(&self, cx: &mut Context<Self>) -> AnyElement {
        let (installed, rest): (Vec<&Game>, Vec<&Game>) =
            self.games.iter().partition(|g| g.installed || g.running);
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .gap_3()
            .overflow_y_scrollbar()
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().foreground)
                    .child("Games"),
            )
            .child(self.render_game_section(cx, "Installed & Running", &installed, false))
            .child(self.render_game_section(cx, "More Games", &rest, true))
            .into_any_element()
    }

    fn render_game_section(
        &self,
        cx: &mut Context<Self>,
        title: &str,
        games: &[&Game],
        muted: bool,
    ) -> AnyElement {
        if games.is_empty() {
            return div().into_any_element();
        }
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{title} ({})", games.len())),
            )
            .child(
                div().flex().flex_row().flex_wrap().gap_3().children(
                    games
                        .iter()
                        .map(|game| self.render_game_card(cx, game, muted)),
                ),
            )
            .into_any_element()
    }

    /// One game card: cached art (or initial tile), name, and detected statuses.
    fn render_game_card(&self, cx: &mut Context<Self>, game: &Game, muted: bool) -> AnyElement {
        // Cached file art when prefetched, else the name-initial tile.
        // Remote `img(url)` is not used: desktop GPUI ships no HTTP client
        // (`with_http_client` is web-backend-only), so URLs would stay blank.
        let art = match crate::games::cached_art_path(game) {
            Some(path) => div()
                .w(px(120.))
                .h(px(180.))
                .rounded_lg()
                .overflow_hidden()
                .child(
                    // PathBuf (not String): String routes to the embedded
                    // asset bundle and misses; PathBuf reads the file.
                    img(path).w_full().h_full().object_fit(ObjectFit::Contain),
                )
                .into_any_element(),
            None => div()
                .w(px(120.))
                .h(px(180.))
                .rounded_lg()
                .bg(cx.theme().muted)
                .items_center()
                .justify_center()
                .text_xl()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().background)
                .child(game.mark().to_string())
                .into_any_element(),
        };
        let game_id = game.id.clone();
        let has_profile = self.profiles.has_any(&game.id);
        let accent = cx.theme().primary;
        let ink = cx.theme().foreground;
        div()
            .id(SharedString::from(format!("game-{}", game.id)))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.profile_editor = Some(game_id.clone());
                this.profile_device = None;
                cx.notify();
            }))
            .flex()
            .flex_col()
            .w(px(120.))
            .flex_shrink_0()
            .gap_1p5()
            .opacity(if muted && !game.running { 0.55 } else { 1.0 })
            .child(art)
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(cx.theme().foreground)
                    .child(game.name.clone()),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap_1()
                    .when(game.running, |row| {
                        row.child(accent_badge("Running", accent, ink))
                    })
                    .when(game.installed, |row| {
                        row.child(Tag::secondary().small().child("Installed"))
                    })
                    .when(has_profile, |row| row.child(accent_badge("Profile", accent, ink))),
            )
            .into_any_element()
    }

    /// Shared section surface and heading keep settings on one alignment spine.
    fn render_settings_section(
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
    fn render_switch_row(
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
    fn render_settings(&self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
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
                                        ThemeChoice::System => "Follow macOS",
                                        ThemeChoice::Dark => "Warm and muted",
                                        ThemeChoice::Light => "Bright and clear",
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
    fn render_effect_slider(
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

/// Colours a list row needs, copied into the list's `'static` render closure.
#[derive(Clone, Copy)]
struct PageColors {
    fg: Hsla,
    muted: Hsla,
    accent: Hsla,
    border: Hsla,
}

/// One release on the Updates timeline. Plain text and a divider — no per-line
/// badges or translucent cards, so each row is cheap to lay out.
fn release_row(entry: &crate::changelog::Entry, divider: bool, current: bool, c: PageColors) -> AnyElement {
    use crate::changelog::{display_date, grouped, Kind};
    let mut changes = div().flex().flex_col().gap_3().flex_1().min_w_0();
    for (kind, lines) in grouped(entry) {
        let (label, tone) = match kind {
            Some(Kind::New) => ("New", c.accent),
            Some(Kind::Improved) => ("Improved", c.accent),
            Some(Kind::Fixed) => ("Fixed", c.muted),
            None => ("Changes", c.muted),
        };
        let mut group = div().flex().flex_col().gap_1p5().child(
            div().text_xs().font_weight(FontWeight::BOLD).text_color(tone).child(label.to_uppercase()),
        );
        for line in lines {
            group = group.child(
                h_flex()
                    .gap_2()
                    .items_start()
                    .child(div().text_sm().text_color(c.muted).child("•"))
                    .child(div().flex_1().min_w_0().text_sm().text_color(c.fg).child(line.to_string())),
            );
        }
        changes = changes.child(group);
    }
    h_flex()
        .gap_8()
        .items_start()
        .py_5()
        .when(divider, |row| row.border_t_1().border_color(c.border))
        .child(
            div()
                .w(px(132.))
                .flex_none()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(if current { c.accent } else { c.fg })
                        .child(format!("v{}", entry.version)),
                )
                .child(div().text_xs().text_color(c.muted).child(display_date(&entry.date)))
                .children(current.then(|| div().text_xs().font_weight(FontWeight::MEDIUM).text_color(c.accent).child("Current"))),
        )
        .child(changes)
        .into_any_element()
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_control_sliders(window, cx);
        let pages = [Page::Overview, Page::Games, Page::Supported];
        // Kit's Root paints the translucent semantic background once.
        // Keep the app shell clear so overlapping layers do not compound alpha.
        let clear = Hsla {
            a: 0.0,
            ..cx.theme().background
        };
        let toast = self.render_toast(cx);
        div().relative().size_full().children(toast).child(
            div()
                .flex()
                .flex_col()
                .size_full()
                .text_color(cx.theme().foreground)
                .child(TitleBar::new().bg(clear).child(self.render_titlebar(cx)))
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
    }
}
