//! Persisted app settings, mirroring the `localStorage` keys in
//! `src/lib/themes.ts` (`openmouse.theme*`) and `src/lib/window-opacity.ts`
//! (`openmouse.window-*`).
//!
//! Native home: `<config_dir>/openmouse/settings.json` (e.g.
//! `~/Library/Application Support/openmouse/settings.json` on macOS).
//! Load is best-effort — corrupt/missing files yield defaults, never an error
//! surface. Saves are synchronous small-file writes on explicit user action
//! (release/apply), matching the Tauri `save*` call sites.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::native_window::OverlayCorner;
use crate::theme::ThemeChoice;

const SETTINGS_FILE: &str = "settings.json";

/// Transparency floor: below 40% text stops reading as a window (same rule as
/// `MIN_OPACITY` in `window-opacity.ts`).
pub const MIN_OPACITY: f32 = 40.0;
/// Blur ceiling in px (same as `MAX_BLUR`).
pub const MAX_BLUR: f32 = 80.0;
/// Vibrancy ceiling in percent (same as the Settings slider max).
pub const MAX_VIBRANCY: f32 = 200.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredSettings {
    /// Last sampled Dynamic accent as [r, g, b] (mirrors DYNAMIC_HEX_KEY):
    /// lets a relaunch repaint the real color instantly instead of flashing
    /// the fallback green before the background sample lands.
    #[serde(default)]
    dynamic_hex: Option<[u8; 3]>,

    #[serde(default)]
    launch_on_startup: bool,
    #[serde(default)]
    discord_presence: bool,
    /// What Rich Presence shows: the connected mouse's name, the game being
    /// played, and live stats (DPI and polling rate).
    #[serde(default = "default_true")]
    discord_show_device: bool,
    #[serde(default = "default_true")]
    discord_show_game: bool,
    #[serde(default)]
    discord_show_stats: bool,
    /// Write device changes as they are made, instead of staging them behind
    /// the Apply bar.
    #[serde(default)]
    instant_apply: bool,
    /// Show the on-screen game toast at all.
    #[serde(default = "default_true")]
    overlay_enabled: bool,
    /// A short chime with each game notification.
    #[serde(default = "default_true")]
    overlay_sound: bool,
    /// Corner of the screen the game overlay toast uses.
    #[serde(default = "default_corner")]
    overlay_corner: String,
    #[serde(default = "default_theme")]
    theme: String,
    #[serde(default = "default_opacity")]
    window_opacity: f32,
    #[serde(default)]
    panel_blur: f32,
    #[serde(default = "default_vibrancy")]
    dynamic_vibrancy: f32,
}

fn default_true() -> bool {
    true
}
fn default_corner() -> String {
    OverlayCorner::default().as_str().to_string()
}
fn default_theme() -> String {
    "dark".to_string()
}
fn default_opacity() -> f32 {
    100.0
}
fn default_vibrancy() -> f32 {
    100.0
}

impl Default for StoredSettings {
    fn default() -> Self {
        Self {
            launch_on_startup: false,
            discord_presence: false,
            discord_show_device: true,
            discord_show_game: true,
            discord_show_stats: false,
            instant_apply: false,
            overlay_enabled: true,
            overlay_sound: true,
            overlay_corner: default_corner(),
            dynamic_hex: None,
            theme: default_theme(),
            window_opacity: default_opacity(),
            panel_blur: 0.0,
            dynamic_vibrancy: default_vibrancy(),
        }
    }
}

/// Live settings owned by [`crate::app::AppView`].
#[derive(Debug, Clone)]
pub struct Settings {
    pub dynamic_hex: Option<(u8, u8, u8)>,

    /// Placeholder: LaunchAgent/plist wiring lands with autostart support.
    pub launch_on_startup: bool,
    /// Placeholder: Discord IPC wiring lands with rich-presence support.
    pub discord_presence: bool,
    pub discord_show_device: bool,
    pub discord_show_game: bool,
    pub discord_show_stats: bool,
    /// See `StoredSettings::instant_apply`.
    pub instant_apply: bool,
    pub overlay_enabled: bool,
    pub overlay_sound: bool,
    pub overlay_corner: OverlayCorner,
    pub theme: ThemeChoice,
    pub window_opacity: f32,
    pub panel_blur: f32,
    pub dynamic_vibrancy: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self::load()
    }
}

fn theme_from_str(s: &str) -> ThemeChoice {
    match s {
        "system" => ThemeChoice::System,
        "light" => ThemeChoice::Light,
        "dynamic" => ThemeChoice::Dynamic,
        _ => ThemeChoice::Dark,
    }
}

fn theme_to_str(choice: ThemeChoice) -> &'static str {
    match choice {
        ThemeChoice::System => "system",
        ThemeChoice::Dark => "dark",
        ThemeChoice::Light => "light",
        ThemeChoice::Dynamic => "dynamic",
    }
}

fn settings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("openmouse").join(SETTINGS_FILE))
}

impl Settings {
    pub fn load() -> Self {
        let stored: StoredSettings = settings_path()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        Self {
            dynamic_hex: stored.dynamic_hex.map(|[r, g, b]| (r, g, b)),
            launch_on_startup: stored.launch_on_startup,
            discord_presence: stored.discord_presence,
            discord_show_device: stored.discord_show_device,
            discord_show_game: stored.discord_show_game,
            discord_show_stats: stored.discord_show_stats,
            instant_apply: stored.instant_apply,
            overlay_enabled: stored.overlay_enabled,
            overlay_sound: stored.overlay_sound,
            overlay_corner: OverlayCorner::parse(&stored.overlay_corner),
            theme: theme_from_str(&stored.theme),
            window_opacity: stored.window_opacity.clamp(MIN_OPACITY, 100.0),
            panel_blur: stored.panel_blur.clamp(0.0, MAX_BLUR),
            dynamic_vibrancy: stored.dynamic_vibrancy.clamp(0.0, MAX_VIBRANCY),
        }
    }

    pub fn save(&self) {
        let Some(path) = settings_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let stored = StoredSettings {
            dynamic_hex: self.dynamic_hex.map(|(r, g, b)| [r, g, b]),
            launch_on_startup: self.launch_on_startup,
            discord_presence: self.discord_presence,
            discord_show_device: self.discord_show_device,
            discord_show_game: self.discord_show_game,
            discord_show_stats: self.discord_show_stats,
            instant_apply: self.instant_apply,
            overlay_enabled: self.overlay_enabled,
            overlay_sound: self.overlay_sound,
            overlay_corner: self.overlay_corner.as_str().to_string(),
            theme: theme_to_str(self.theme).to_string(),
            window_opacity: self.window_opacity,
            panel_blur: self.panel_blur,
            dynamic_vibrancy: self.dynamic_vibrancy,
        };
        if let Ok(text) = serde_json::to_string_pretty(&stored) {
            let _ = std::fs::write(path, text);
        }
    }
}
