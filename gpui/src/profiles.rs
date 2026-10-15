//! Per-game device profiles: a saved DPI / polling rate / lift-off / surface
//! configuration per game, pushed to the connected mouse on demand or when the
//! game launches (mirrors `src/lib/game-profiles.ts`). A `None` field means
//! "this profile doesn't touch that setting".
//!
//! Native home: `<config_dir>/openmouse/game-profiles.json`, keyed by the
//! games catalog `id`. Load is best-effort; a corrupt file yields no profiles.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GameProfile {
    #[serde(default)]
    pub dpi: Option<f64>,
    #[serde(default)]
    pub polling: Option<u32>,
    #[serde(default)]
    pub lod: Option<String>,
    #[serde(default)]
    pub surface: Option<String>,
    /// Push this profile the moment the game is detected running.
    #[serde(default)]
    pub auto_apply: bool,
}

impl GameProfile {
    /// True if the profile overrides at least one setting.
    pub fn is_meaningful(&self) -> bool {
        self.dpi.is_some() || self.polling.is_some() || self.lod.is_some() || self.surface.is_some()
    }

    /// Short summary of what the profile changes, for notifications.
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if let Some(dpi) = self.dpi {
            parts.push(format!("{dpi:.0} DPI"));
        }
        if let Some(hz) = self.polling {
            parts.push(format!("{hz} Hz"));
        }
        if let Some(lod) = &self.lod {
            parts.push(format!("{lod} lift-off"));
        }
        if let Some(surface) = &self.surface {
            parts.push(format!("surface {surface}"));
        }
        parts.join(", ")
    }

    /// The shape `applyGameProfile` (shared JS) takes.
    pub fn to_js(&self) -> Value {
        let mut out = serde_json::Map::new();
        if let Some(v) = self.dpi {
            out.insert("dpi".into(), json!(v));
        }
        if let Some(v) = self.polling {
            out.insert("pollingRateHz".into(), json!(v));
        }
        if let Some(v) = &self.lod {
            out.insert("liftOffDistance".into(), json!(v));
        }
        if let Some(v) = &self.surface {
            out.insert("gamingSurfaceMode".into(), json!(v));
        }
        out.insert("autoApply".into(), json!(self.auto_apply));
        Value::Object(out)
    }
}

/// Profiles live per game *and* per device (`game id -> device key -> profile`):
/// one game can drive a mouse and a keyboard differently. The device key is the
/// same `vendor:product` key the device list uses.
///
/// The first version stored one flat profile per game. Reading that file puts
/// each such profile under [`ANY_DEVICE`]: it applies to the first mouse-like
/// device and is rewritten under that device's own key the first time it is
/// edited.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Profiles(BTreeMap<String, BTreeMap<String, GameProfile>>);

/// Device key for a profile saved before profiles were per-device.
pub const ANY_DEVICE: &str = "*";

impl<'de> Deserialize<'de> for Profiles {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = BTreeMap::<String, Value>::deserialize(deserializer)?;
        let mut out = BTreeMap::new();
        for (game, entry) in raw {
            let Some(object) = entry.as_object() else { continue };
            let legacy = ["dpi", "polling", "lod", "surface", "auto_apply"].iter().any(|k| object.contains_key(*k));
            let per_device: BTreeMap<String, GameProfile> = if legacy {
                serde_json::from_value::<GameProfile>(entry.clone())
                    .map(|p| BTreeMap::from([(ANY_DEVICE.to_string(), p)]))
                    .unwrap_or_default()
            } else {
                object
                    .iter()
                    .filter_map(|(device, v)| serde_json::from_value::<GameProfile>(v.clone()).ok().map(|p| (device.clone(), p)))
                    .collect()
            };
            if !per_device.is_empty() {
                out.insert(game, per_device);
            }
        }
        Ok(Profiles(out))
    }
}

fn path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("openmouse").join("game-profiles.json"))
}

impl Profiles {
    pub fn load() -> Self {
        path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = path() else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, text);
        }
    }

    /// This device's own profile for the game, if it has one.
    pub fn get(&self, game_id: &str, device: &str) -> Option<&GameProfile> {
        self.0.get(game_id).and_then(|per| per.get(device))
    }

    /// A profile saved before profiles were per-device, if the game has one.
    pub fn legacy(&self, game_id: &str) -> Option<&GameProfile> {
        self.get(game_id, ANY_DEVICE)
    }

    /// This device's profile, or — for the device that inherits it — the legacy
    /// one. `inherits_legacy` is the caller's call (first mouse-like device).
    pub fn effective(&self, game_id: &str, device: &str, inherits_legacy: bool) -> Option<&GameProfile> {
        self.get(game_id, device).or_else(|| if inherits_legacy { self.legacy(game_id) } else { None })
    }

    /// True if any device has a profile that changes something.
    pub fn has_any(&self, game_id: &str) -> bool {
        self.0.get(game_id).is_some_and(|per| per.values().any(GameProfile::is_meaningful))
    }

    /// True if any device's profile for the game applies on launch.
    pub fn any_auto(&self, game_id: &str) -> bool {
        self.0.get(game_id).is_some_and(|per| per.values().any(|p| p.auto_apply && p.is_meaningful()))
    }

    /// Stores `profile` for the device, or drops it when it no longer does
    /// anything. A legacy profile is superseded once the game has a device one.
    pub fn set(&mut self, game_id: &str, device: &str, profile: GameProfile) {
        if profile.is_meaningful() || profile.auto_apply {
            let per = self.0.entry(game_id.to_string()).or_default();
            per.insert(device.to_string(), profile);
            per.remove(ANY_DEVICE);
        } else {
            self.clear(game_id, device);
        }
    }

    pub fn clear(&mut self, game_id: &str, device: &str) {
        if let Some(per) = self.0.get_mut(game_id) {
            per.remove(device);
            if per.is_empty() {
                self.0.remove(game_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dpi(v: f64) -> GameProfile {
        GameProfile { dpi: Some(v), ..Default::default() }
    }

    #[test]
    fn describes_only_what_it_sets() {
        let p = GameProfile { dpi: Some(800.0), polling: Some(1000), ..Default::default() };
        assert_eq!(p.describe(), "800 DPI, 1000 Hz");
        assert!(p.is_meaningful());
        assert!(!GameProfile { auto_apply: true, ..Default::default() }.is_meaningful());
    }

    #[test]
    fn js_shape_omits_untouched_fields() {
        let p = GameProfile { lod: Some("Low".into()), ..Default::default() };
        let v = p.to_js();
        assert_eq!(v["liftOffDistance"], "Low");
        assert!(v.get("dpi").is_none() && v.get("pollingRateHz").is_none());
    }

    #[test]
    fn profiles_are_per_device() {
        let mut ps = Profiles::default();
        ps.set("cs2", "046d:c54d", dpi(400.0));
        ps.set("cs2", "31e3:1322", dpi(1600.0));
        assert_eq!(ps.get("cs2", "046d:c54d").unwrap().dpi, Some(400.0));
        assert_eq!(ps.get("cs2", "31e3:1322").unwrap().dpi, Some(1600.0));
        ps.clear("cs2", "046d:c54d");
        assert!(ps.get("cs2", "046d:c54d").is_none() && ps.has_any("cs2"));
        ps.clear("cs2", "31e3:1322");
        assert!(!ps.has_any("cs2"));
    }

    #[test]
    fn empty_profiles_are_dropped() {
        let mut ps = Profiles::default();
        ps.set("cs2", "a:b", dpi(400.0));
        ps.set("cs2", "a:b", GameProfile::default());
        assert!(ps.get("cs2", "a:b").is_none() && !ps.has_any("cs2"));
    }

    /// The first file format: one flat profile per game. It must still load, and
    /// be taken over by the first device that edits it.
    #[test]
    fn reads_the_old_flat_format() {
        let old = r#"{ "valorant": { "dpi": 800.0, "polling": 8000, "lod": null, "surface": null, "auto_apply": true } }"#;
        let mut ps: Profiles = serde_json::from_str(old).unwrap();
        let legacy = ps.legacy("valorant").expect("kept as the any-device profile");
        assert_eq!((legacy.dpi, legacy.polling, legacy.auto_apply), (Some(800.0), Some(8000), true));
        // Only the device that inherits it sees it.
        assert!(ps.effective("valorant", "046d:c54d", true).is_some());
        assert!(ps.effective("valorant", "31e3:1322", false).is_none());
        // Editing under a device takes it over.
        let mut mine = ps.effective("valorant", "046d:c54d", true).cloned().unwrap();
        mine.dpi = Some(1600.0);
        ps.set("valorant", "046d:c54d", mine);
        assert!(ps.legacy("valorant").is_none());
        assert_eq!(ps.get("valorant", "046d:c54d").unwrap().polling, Some(8000));
    }

    #[test]
    fn round_trips_the_new_format() {
        let mut ps = Profiles::default();
        ps.set("cs2", "046d:c54d", dpi(400.0));
        let back: Profiles = serde_json::from_str(&serde_json::to_string(&ps).unwrap()).unwrap();
        assert_eq!(back.get("cs2", "046d:c54d").unwrap().dpi, Some(400.0));
    }
}
