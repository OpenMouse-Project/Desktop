//! Backs the Games page's game detection and "is this game currently running"
//! indicator.
//!
//! This module provides:
//! 1. `running_process_names` — every running process name (for the "Running"
//!    badge on game cards).
//! 2. `scan_installed_games` — parses Steam's libraryfolders.vdf and
//!    appmanifest_*.acf files, Epic Games manifests, and Windows registry to
//!    find genuinely installed games.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use sysinfo::{ProcessesToUpdate, System};

pub struct ProcessListState(Mutex<System>);

impl Default for ProcessListState {
    fn default() -> Self {
        Self(Mutex::new(System::new()))
    }
}

/// Every currently-running process's executable base name, lowercased.
#[tauri::command]
pub fn running_process_names(state: tauri::State<'_, ProcessListState>) -> Vec<String> {
    let mut system = state.0.lock().unwrap();
    system.refresh_processes(ProcessesToUpdate::All, true);
    system
        .processes()
        .values()
        .map(|process| process.name().to_string_lossy().to_lowercase())
        .collect()
}

// ---------------------------------------------------------------------------
// Steam library parsing (libraryfolders.vdf + appmanifest_*.acf)
// ---------------------------------------------------------------------------

/// Minimal text-VDF parser — just enough for Steam's KeyValues format.
/// Steam mixes casing (e.g. `installdir` vs `StateFlags`), so lookups are
/// case-insensitive.
mod vdf {
    use std::collections::HashMap;

    #[derive(Debug)]
    pub enum Value {
        Str(String),
        Map(HashMap<String, Value>),
    }

    pub struct Vdf(HashMap<String, Value>);

    impl Vdf {
        pub fn str(&self, key: &str) -> Option<&str> {
            self.0.get(&key.to_lowercase()).and_then(|v| match v {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            })
        }

        pub fn get(&self, key: &str) -> Option<&Value> {
            self.0.get(&key.to_lowercase())
        }
    }

    /// Parse a Steam text-VDF document. Returns `None` on empty/malformed input.
    pub fn parse(text: &str) -> Option<Vdf> {
        let entries = parse_entries(text, &mut 0)?;
        Some(Vdf(entries))
    }

    fn parse_entries(text: &str, pos: &mut usize) -> Option<HashMap<String, Value>> {
        let mut map = HashMap::new();
        let bytes = text.as_bytes();
        while *pos < bytes.len() {
            skip_whitespace(bytes, pos);
            if *pos >= bytes.len() {
                break;
            }
            // Check for closing brace
            if bytes[*pos] == b'}' {
                *pos += 1;
                return Some(map);
            }
            // Read key
            let key = read_string(bytes, pos)?;
            skip_whitespace(bytes, pos);
            if *pos >= bytes.len() {
                return None;
            }
            if bytes[*pos] == b'{' {
                *pos += 1;
                let sub = parse_entries(text, pos)?;
                map.insert(key.to_lowercase(), Value::Map(sub));
            } else {
                let val = read_string(bytes, pos)?;
                map.insert(key.to_lowercase(), Value::Str(val));
            }
        }
        Some(map)
    }

    fn skip_whitespace(bytes: &[u8], pos: &mut usize) {
        while *pos < bytes.len() && (bytes[*pos] == b' ' || bytes[*pos] == b'\t' || bytes[*pos] == b'\r' || bytes[*pos] == b'\n') {
            *pos += 1;
        }
    }

    fn read_string(bytes: &[u8], pos: &mut usize) -> Option<String> {
        skip_whitespace(bytes, pos);
        if *pos >= bytes.len() {
            return None;
        }
        if bytes[*pos] == b'"' {
            *pos += 1;
            let start = *pos;
            while *pos < bytes.len() && bytes[*pos] != b'"' {
                if bytes[*pos] == b'\\' && *pos + 1 < bytes.len() {
                    *pos += 2;
                } else {
                    *pos += 1;
                }
            }
            let s = String::from_utf8_lossy(&bytes[start..*pos]).to_string();
            if *pos < bytes.len() {
                *pos += 1; // skip closing quote
            }
            Some(s)
        } else {
            // Unquoted token
            let start = *pos;
            while *pos < bytes.len() && bytes[*pos] != b' ' && bytes[*pos] != b'\t' && bytes[*pos] != b'\r' && bytes[*pos] != b'\n' && bytes[*pos] != b'{' && bytes[*pos] != b'}' {
                *pos += 1;
            }
            Some(String::from_utf8_lossy(&bytes[start..*pos]).to_string())
        }
    }
}

/// An app found installed in a Steam library.
#[derive(Debug, Clone)]
struct SteamApp {
    app_id: u32,
    name: String,
    installdir: String,
    library_path: PathBuf,
}

impl SteamApp {
    fn install_path(&self) -> PathBuf {
        self.library_path
            .join("steamapps")
            .join("common")
            .join(&self.installdir)
    }
}

/// Parse `libraryfolders.vdf` to get all Steam library root paths.
fn parse_library_paths(text: &str) -> Vec<PathBuf> {
    let Some(root) = vdf::parse(text) else {
        return Vec::new();
    };
    let Some(folders) = root.get("libraryfolders").and_then(|v| match v {
        vdf::Value::Map(m) => Some(m),
        _ => None,
    }) else {
        return Vec::new();
    };

    folders
        .values()
        .filter_map(|value| match value {
            vdf::Value::Map(m) => m.get("path").and_then(|v| match v {
                vdf::Value::Str(s) => Some(PathBuf::from(s)),
                _ => None,
            }),
            _ => None,
        })
        .collect()
}

/// Parse an `appmanifest_<appid>.acf` file.
fn parse_appmanifest(text: &str, library_path: &Path) -> Option<SteamApp> {
    let root = vdf::parse(text)?;
    let state_map = match root.get("appstate")? {
        vdf::Value::Map(m) => m,
        _ => return None,
    };

    let app_id = match state_map.get("appid") {
        Some(vdf::Value::Str(s)) => s.trim().parse().ok()?,
        _ => return None,
    };
    let installdir = match state_map.get("installdir") {
        Some(vdf::Value::Str(s)) if !s.is_empty() => s.clone(),
        _ => return None,
    };
    let name = match state_map.get("name") {
        Some(vdf::Value::Str(s)) => s.clone(),
        _ => String::new(),
    };

    Some(SteamApp {
        app_id,
        name,
        installdir,
        library_path: library_path.to_path_buf(),
    })
}

/// Find all installed Steam apps across all libraries.
fn scan_steam() -> Vec<SteamApp> {
    let mut apps = Vec::new();

    // Common Steam install locations on Windows
    let program_files_x86 = std::env::var("ProgramFiles(x86)").unwrap_or_default();
    let program_files = std::env::var("ProgramFiles").unwrap_or_default();

    let steam_roots = [
        PathBuf::from(format!(r"{program_files_x86}\Steam")),
        PathBuf::from(format!(r"{program_files}\Steam")),
    ];

    for steam_root in &steam_roots {
        if !steam_root.exists() {
            continue;
        }

        // Parse libraryfolders.vdf to find all library paths
        let vdf_path = steam_root.join("steamapps").join("libraryfolders.vdf");
        let mut libraries = vec![steam_root.clone()];
        if let Ok(text) = std::fs::read_to_string(&vdf_path) {
            libraries.extend(parse_library_paths(&text));
        }
        libraries.sort();
        libraries.dedup();

        // Scan each library for appmanifest_*.acf files
        for library in &libraries {
            let steamapps = library.join("steamapps");
            if !steamapps.exists() {
                continue;
            }
            if let Ok(entries) = std::fs::read_dir(&steamapps) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with("appmanifest_") && name_str.ends_with(".acf") {
                        if let Ok(text) = std::fs::read_to_string(entry.path()) {
                            if let Some(app) = parse_appmanifest(&text, library) {
                                apps.push(app);
                            }
                        }
                    }
                }
            }
        }
    }

    apps
}

// ---------------------------------------------------------------------------
// Epic Games Store manifest scanning
// ---------------------------------------------------------------------------

/// Parse Epic Games manifest files to find installed games.
fn scan_epic() -> Vec<SteamApp> {
    let mut apps = Vec::new();
    let program_data = std::env::var("ProgramData").unwrap_or_default();
    let manifests_dir = PathBuf::from(format!(
        r"{program_data}\Epic\EpicGamesLauncher\Data\Manifests"
    ));

    if !manifests_dir.exists() {
        return apps;
    }

    if let Ok(entries) = std::fs::read_dir(&manifests_dir) {
        for entry in entries.flatten() {
            if entry.path().extension().and_then(|e| e.to_str()) == Some("item") {
                if let Ok(text) = std::fs::read_to_string(entry.path()) {
                    if let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) {
                        let install_path = manifest["InstallLocation"]
                            .as_str()
                            .unwrap_or("");
                        let display_name = manifest["DisplayName"]
                            .as_str()
                            .unwrap_or("Unknown");
                        let catalog_ns = manifest["CatalogNamespace"]
                            .as_str()
                            .unwrap_or("");
                        let app_id = manifest["AppVersionString"]
                            .as_str()
                            .unwrap_or("");

                        // Use namespace+appId as a pseudo ID, or hash the display name
                        let pseudo_id: u32 = format!("{catalog_ns}{app_id}")
                            .bytes()
                            .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));

                        if !install_path.is_empty() && Path::new(install_path).exists() {
                            apps.push(SteamApp {
                                app_id: pseudo_id,
                                name: display_name.to_string(),
                                installdir: install_path.to_string(),
                                library_path: PathBuf::new(),
                            });
                        }
                    }
                }
            }
        }
    }

    apps
}

// ---------------------------------------------------------------------------
// Main command
// ---------------------------------------------------------------------------

/// Scans for genuinely installed games using proper game launcher detection:
/// - Steam: parses libraryfolders.vdf + appmanifest_*.acf
/// - Epic Games Store: parses manifest files
///
/// Returns which of the known Steam games are installed, and all
/// game install directories found.
#[tauri::command]
pub fn scan_installed_games(known_steam_ids: Vec<u32>) -> InstalledGamesResult {
    // 1. Scan Steam
    let steam_apps = scan_steam();
    let mut steam_ids_to_apps: HashMap<u32, SteamApp> = HashMap::new();
    for app in &steam_apps {
        steam_ids_to_apps.insert(app.app_id, app.clone());
    }

    // 2. Scan Epic
    let epic_apps = scan_epic();

    // 3. Match known Steam games against installed
    let mut installed_steam_ids: Vec<u32> = Vec::new();
    let mut install_paths: Vec<String> = Vec::new();

    for &id in &known_steam_ids {
        if let Some(app) = steam_ids_to_apps.get(&id) {
            installed_steam_ids.push(id);
            install_paths.push(app.install_path().to_string_lossy().to_string());
        }
    }

    // Add Epic games
    for app in &epic_apps {
        install_paths.push(app.installdir.clone());
    }

    install_paths.sort();
    install_paths.dedup();
    installed_steam_ids.sort();
    installed_steam_ids.dedup();

    InstalledGamesResult {
        installed_steam_ids,
        install_paths,
    }
}

#[derive(serde::Serialize)]
pub struct InstalledGamesResult {
    /// Steam AppIDs that are installed on this system.
    installed_steam_ids: Vec<u32>,
    /// All game install directory paths found (Steam + Epic + registry).
    install_paths: Vec<String>,
}
