//! Filesystem-only launcher detection. Manifest IDs, never display names, are
//! matched to the catalog; each scan recomputes installed state from scratch.

use super::Game;
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

#[derive(Default)]
struct Roots {
    steam: Vec<PathBuf>,
    epic: Vec<PathBuf>,
    riot: Vec<PathBuf>,
    league_bundles: Vec<PathBuf>,
}

impl Roots {
    fn discover() -> Self {
        #[cfg(target_os = "macos")]
        {
            let mut roots = Self::default();
            if let Some(home) = dirs::home_dir() {
                roots
                    .steam
                    .push(home.join("Library/Application Support/Steam"));
                // Observed macOS EGL deployment: jpallagrosi/unrealEpic_multiUser,
                // epicManifests_cp.sh (the author's tested ARM/Intel deployment).
                roots.epic.push(
                    home.join("Library/Application Support/Epic/EpicGamesLauncher/Data/Manifests"),
                );
                roots
                    .league_bundles
                    .push(home.join("Applications/League of Legends.app"));
            }
            // Riot's Mac reinstall article documents /Applications and
            // /Users/Shared/Riot Games. The install JSON at this shared root is
            // independently observed in reddit.com/r/leagueoflegends/comments/137bwla.
            roots.riot.push(PathBuf::from("/Users/Shared/Riot Games"));
            roots
                .league_bundles
                .push(PathBuf::from("/Applications/League of Legends.app"));
            roots
        }
        #[cfg(target_os = "windows")]
        {
            let mut roots = Self::default();
            roots.steam.extend(windows_registry::steam_roots());
            for (key, default) in [
                ("ProgramFiles(x86)", r"C:\Program Files (x86)"),
                ("ProgramFiles", r"C:\Program Files"),
            ] {
                roots.steam.push(env_path(key, default).join("Steam"));
            }
            let data = env_path("ProgramData", r"C:\ProgramData");
            roots
                .epic
                .push(data.join("Epic/EpicGamesLauncher/Data/Manifests"));
            roots.riot.push(data.join("Riot Games"));
            roots
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        Self::default()
    }
}

#[cfg(target_os = "windows")]
fn env_path(key: &str, default: &str) -> PathBuf {
    std::env::var_os(key)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(default))
}

#[derive(Default)]
struct Installed {
    steam: HashSet<u32>,
    epic: HashSet<String>,
    riot: HashSet<&'static str>,
}

pub(super) fn mark_installed(games: &mut [Game]) {
    mark_with_roots(games, &Roots::discover());
}

fn mark_with_roots(games: &mut [Game], roots: &Roots) {
    let mut installed = Installed::default();
    scan_steam(&roots.steam, &mut installed.steam);
    scan_epic(&roots.epic, &mut installed.epic);
    scan_riot(roots, &mut installed.riot);
    for game in games {
        game.installed = game
            .steam_app_id
            .is_some_and(|id| installed.steam.contains(&id))
            || game
                .epic_id
                .as_deref()
                .is_some_and(|id| installed.epic.contains(id))
            || game
                .riot_id
                .as_deref()
                .is_some_and(|id| installed.riot.contains(id));
    }
}

fn unique_existing_dirs<P: AsRef<Path>>(paths: impl IntoIterator<Item = P>) -> Vec<PathBuf> {
    let mut paths: Vec<_> = paths
        .into_iter()
        .filter_map(|path| {
            let path = std::fs::canonicalize(path).ok()?;
            path.is_dir().then_some(path)
        })
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

fn steam_libraries(roots: &[PathBuf]) -> Vec<PathBuf> {
    let roots = unique_existing_dirs(roots);
    let mut libraries = Vec::new();
    for root in roots {
        // Current Steam uses steamapps; older clients used config.
        for relative in ["steamapps/libraryfolders.vdf", "config/libraryfolders.vdf"] {
            if let Ok(text) = std::fs::read_to_string(root.join(relative)) {
                libraries.extend(library_paths(&text));
            }
        }
        libraries.push(root);
    }
    unique_existing_dirs(libraries)
}

fn library_paths(text: &str) -> Vec<PathBuf> {
    let Some(vdf) = vdf::parse(text) else {
        return Vec::new();
    };
    let Some(folders) = vdf.map("libraryfolders") else {
        return Vec::new();
    };
    folders
        .0
        .iter()
        .filter_map(|(index, value)| {
            // Ignore legacy TimeNextStatsReport etc., even if their values are strings.
            if index.parse::<u32>().is_err() {
                return None;
            }
            let path = match value {
                vdf::Value::Text(path) => path.as_str(),
                vdf::Value::Map(map) => map.text("path")?,
            };
            (!path.trim().is_empty()).then(|| PathBuf::from(path))
        })
        .collect()
}

fn scan_steam(roots: &[PathBuf], ids: &mut HashSet<u32>) {
    for library in steam_libraries(roots) {
        let Ok(entries) = std::fs::read_dir(library.join("steamapps")) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let name = name.to_ascii_lowercase();
            let Some(file_id) = name
                .strip_prefix("appmanifest_")
                .and_then(|name| name.strip_suffix(".acf"))
                .and_then(|id| id.parse::<u32>().ok())
            else {
                continue;
            };
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Some(doc) = vdf::parse(&text) else {
                continue;
            };
            let Some(state) = doc.map("appstate") else {
                continue;
            };
            let Some(id) = state.text("appid").and_then(|id| id.parse::<u32>().ok()) else {
                continue;
            };
            if id != file_id {
                continue;
            }
            if let Some(flags) = state.text("stateflags") {
                // Steam EAppStateFullyInstalled (4); incomplete downloads must
                // not become installed simply because they created a folder.
                if !flags.parse::<u32>().is_ok_and(|flags| flags & 4 != 0) {
                    continue;
                }
            }
            let Some(dir) = state.text("installdir").and_then(safe_relative) else {
                continue;
            };
            if library.join("steamapps/common").join(dir).is_dir() {
                ids.insert(id);
            }
        }
    }
}

// Manifest-relative payload paths must not resolve to the install root itself
// or outside it. Handle Windows separators on fixture-host platforms too.
fn safe_relative(text: &str) -> Option<PathBuf> {
    let normalized = text.replace('\\', "/");
    if normalized.trim().is_empty() || normalized.contains(':') {
        return None;
    }
    let path = PathBuf::from(normalized);
    if !path
        .components()
        .all(|part| matches!(part, Component::Normal(_)))
    {
        return None;
    }
    Some(path)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
struct EpicManifest {
    app_name: String,
    install_location: PathBuf,
    launch_executable: String,
    #[serde(default, rename = "bIsIncompleteInstall")]
    incomplete: bool,
}

fn scan_epic(roots: &[PathBuf], ids: &mut HashSet<String>) {
    for root in unique_existing_dirs(roots) {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("item"))
            {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(path) else {
                continue;
            };
            let Ok(manifest) = serde_json::from_str::<EpicManifest>(&text) else {
                continue;
            };
            if manifest.app_name.trim().is_empty()
                || manifest.incomplete
                || !manifest.install_location.is_dir()
            {
                continue;
            }
            let Some(executable) = safe_relative(&manifest.launch_executable) else {
                continue;
            };
            let executable = manifest.install_location.join(executable);
            // Native EGL may launch a .app rather than a flat executable.
            if executable.is_file() || is_app_bundle(&executable) {
                ids.insert(manifest.app_name);
            }
        }
    }
}

fn is_app_bundle(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("app"))
        && path.join("Contents/Info.plist").is_file()
        && std::fs::read_dir(path.join("Contents/MacOS"))
            .is_ok_and(|entries| entries.flatten().any(|entry| entry.path().is_file()))
}

fn riot_id_at(path: &Path) -> Option<&'static str> {
    // Recognize known products by exact payload/client names, not an arbitrary
    // substring of their install folder (custom install roots are supported).
    if path
        .join("ShooterGame/Binaries/Win64/VALORANT-Win64-Shipping.exe")
        .is_file()
    {
        return Some("valorant");
    }
    if path.join("LeagueClient.exe").is_file() || mac_league_payload(path) {
        return Some("league_of_legends");
    }
    None
}

fn mac_league_payload(path: &Path) -> bool {
    let lol = if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("app"))
    {
        path.join("Contents/LoL")
    } else {
        path.to_path_buf()
    };
    is_app_bundle(&lol.join("LeagueClient.app"))
        || is_app_bundle(&lol.join("Game/League of Legends.app"))
}

fn scan_riot(roots: &Roots, ids: &mut HashSet<&'static str>) {
    for root in unique_existing_dirs(&roots.riot) {
        if let Ok(text) = std::fs::read_to_string(root.join("RiotClientInstalls.json")) {
            if let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(associated) = manifest
                    .get("associated_client")
                    .and_then(|value| value.as_object())
                {
                    for path in associated.keys() {
                        if let Some(id) = riot_id_at(Path::new(path)) {
                            ids.insert(id);
                        }
                    }
                }
            }
        }
        // Riot's product settings provide stable IDs even for installs with
        // shortcuts disabled. Uninstalled products omit product_install_full_path.
        // Observed schema: galaxy-riot-integration issue #6.
        for id in ["league_of_legends", "valorant"] {
            let product = format!("{id}.live");
            let settings = root
                .join("Metadata")
                .join(&product)
                .join(format!("{product}.product_settings.yaml"));
            let Ok(text) = std::fs::read_to_string(settings) else {
                continue;
            };
            let Some(path) = product_install_path(&text) else {
                continue;
            };
            if riot_id_at(&path) == Some(id) {
                ids.insert(id);
            }
        }
    }
    for bundle in &roots.league_bundles {
        if mac_league_payload(bundle) {
            ids.insert("league_of_legends");
        }
    }
}

// Only consume Riot's documented top-level path scalar, not arbitrary YAML.
// JSON double-quoted strings and YAML single-quoted/plain paths cover the
// launcher-generated field; unsupported compound/indented values are rejected.
fn product_install_path(text: &str) -> Option<PathBuf> {
    let mut found = None;
    for line in text.lines() {
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        let Some(value) = line.strip_prefix("product_install_full_path:") else {
            continue;
        };
        if found.is_some() {
            return None;
        }
        let value = value.trim();
        let value = if value.starts_with('"') {
            let mut stream = serde_json::Deserializer::from_str(value).into_iter::<String>();
            let parsed = stream.next()?.ok()?;
            let rest = value[stream.byte_offset()..].trim();
            if !rest.is_empty() && !rest.starts_with('#') {
                return None;
            }
            parsed
        } else if let Some(quoted) = value.strip_prefix('\'') {
            let mut chars = quoted.chars().peekable();
            let mut parsed = String::new();
            loop {
                match chars.next()? {
                    '\'' if chars.peek() == Some(&'\'') => {
                        chars.next();
                        parsed.push('\'');
                    }
                    '\'' => {
                        let rest: String = chars.collect();
                        if !rest.trim().is_empty() && !rest.trim().starts_with('#') {
                            return None;
                        }
                        break;
                    }
                    ch => parsed.push(ch),
                }
            }
            parsed
        } else {
            if value.starts_with(['[', '{', '&', '*', '|', '>', '!']) {
                return None;
            }
            value.split(" #").next()?.trim().to_owned()
        };
        if value.is_empty() {
            return None;
        }
        found = Some(PathBuf::from(value));
    }
    found
}

mod vdf {
    use super::HashMap;

    pub(super) enum Value {
        Text(String),
        Map(Map),
    }

    pub(super) struct Map(pub(super) HashMap<String, Value>);

    impl Map {
        pub(super) fn text(&self, key: &str) -> Option<&str> {
            match self.0.get(key)? {
                Value::Text(text) => Some(text),
                _ => None,
            }
        }
        pub(super) fn map(&self, key: &str) -> Option<&Map> {
            match self.0.get(key)? {
                Value::Map(map) => Some(map),
                _ => None,
            }
        }
    }

    pub(super) fn parse(text: &str) -> Option<Map> {
        let mut parser = Parser {
            text: text.trim_start_matches('\u{feff}').as_bytes(),
            pos: 0,
        };
        parser.entries(false, 0)
    }

    struct Parser<'a> {
        text: &'a [u8],
        pos: usize,
    }

    impl Parser<'_> {
        fn skip(&mut self) -> Option<()> {
            loop {
                while self.text.get(self.pos).is_some_and(u8::is_ascii_whitespace) {
                    self.pos += 1;
                }
                if self.text.get(self.pos..self.pos + 2) == Some(b"//") {
                    while self.text.get(self.pos).is_some_and(|ch| *ch != b'\n') {
                        self.pos += 1;
                    }
                } else if self.text.get(self.pos..self.pos + 2) == Some(b"/*") {
                    self.pos += 2;
                    while self.text.get(self.pos..self.pos + 2) != Some(b"*/") {
                        if self.pos >= self.text.len() {
                            return None;
                        }
                        self.pos += 1;
                    }
                    self.pos += 2;
                } else {
                    return Some(());
                }
            }
        }

        fn entries(&mut self, nested: bool, depth: usize) -> Option<Map> {
            if depth > 32 {
                return None;
            }
            let mut entries = HashMap::new();
            loop {
                self.skip()?;
                match self.text.get(self.pos) {
                    None => return (!nested).then_some(Map(entries)),
                    Some(b'}') => {
                        if !nested {
                            return None;
                        }
                        self.pos += 1;
                        return Some(Map(entries));
                    }
                    _ => {}
                }
                let key = self.token()?.to_ascii_lowercase();
                self.skip()?;
                let value = if self.text.get(self.pos) == Some(&b'{') {
                    self.pos += 1;
                    Value::Map(self.entries(true, depth + 1)?)
                } else {
                    Value::Text(self.token()?)
                };
                // Ambiguous duplicate keys are malformed, rather than accepting
                // whichever directory/hash iteration happened to win.
                if entries.insert(key, value).is_some() {
                    return None;
                }
            }
        }

        fn token(&mut self) -> Option<String> {
            self.skip()?;
            if self.text.get(self.pos) == Some(&b'"') {
                self.pos += 1;
                let mut result = Vec::new();
                loop {
                    let ch = *self.text.get(self.pos)?;
                    self.pos += 1;
                    match ch {
                        b'"' => return String::from_utf8(result).ok(),
                        b'\\' => {
                            let escaped = *self.text.get(self.pos)?;
                            if escaped == b'\\' || escaped == b'"' {
                                result.push(escaped);
                                self.pos += 1;
                            } else {
                                result.push(ch);
                            }
                        }
                        _ => result.push(ch),
                    }
                }
            }
            let start = self.pos;
            while let Some(ch) = self.text.get(self.pos) {
                if ch.is_ascii_whitespace() || matches!(ch, b'{' | b'}' | b'"') {
                    break;
                }
                self.pos += 1;
            }
            if start == self.pos {
                return None;
            }
            String::from_utf8(self.text[start..self.pos].to_vec()).ok()
        }
    }
}

#[cfg(target_os = "windows")]
mod windows_registry {
    use super::PathBuf;
    use std::ffi::{c_void, OsString};
    use std::os::windows::ffi::OsStringExt;

    type Hkey = *mut c_void;
    #[link(name = "advapi32")]
    extern "system" {
        fn RegOpenKeyExW(
            key: Hkey,
            subkey: *const u16,
            options: u32,
            access: u32,
            result: *mut Hkey,
        ) -> i32;
        fn RegQueryValueExW(
            key: Hkey,
            name: *const u16,
            reserved: *mut u32,
            kind: *mut u32,
            data: *mut u8,
            size: *mut u32,
        ) -> i32;
        fn RegCloseKey(key: Hkey) -> i32;
    }

    fn read_path(root: Hkey, name: &str, view: u32) -> Option<PathBuf> {
        let subkey: Vec<u16> = "Software\\Valve\\Steam\0".encode_utf16().collect();
        let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let mut key = std::ptr::null_mut();
        // These pointers reference live, NUL-terminated UTF-16 buffers; the
        // fixed-size value buffer is sized in bytes for RegQueryValueExW.
        unsafe {
            if RegOpenKeyExW(root, subkey.as_ptr(), 0, 0x20019 | view, &mut key) != 0 {
                return None;
            }
            let mut data = [0u16; 32768];
            let mut size = std::mem::size_of_val(&data) as u32;
            let mut kind = 0;
            let status = RegQueryValueExW(
                key,
                name.as_ptr(),
                std::ptr::null_mut(),
                &mut kind,
                data.as_mut_ptr().cast(),
                &mut size,
            );
            RegCloseKey(key);
            if status != 0
                || kind != 1
                || size % 2 != 0
                || size as usize > std::mem::size_of_val(&data)
            {
                return None;
            }
            let data = &data[..size as usize / 2];
            let end = data.iter().position(|ch| *ch == 0).unwrap_or(data.len());
            (end > 0).then(|| PathBuf::from(OsString::from_wide(&data[..end])))
        }
    }

    pub(super) fn steam_roots() -> Vec<PathBuf> {
        // Predefined HKEYs are sign-extended 32-bit constants, including on Win64.
        let user = 0x80000001u32 as i32 as isize as Hkey;
        let machine = 0x80000002u32 as i32 as isize as Hkey;
        let mut roots = Vec::new();
        if let Some(path) = read_path(user, "SteamPath", 0) {
            roots.push(path);
        }
        for view in [0x0200, 0x0100] {
            if let Some(path) = read_path(machine, "InstallPath", view) {
                roots.push(path);
            }
        }
        roots
    }
}

#[cfg(test)]
mod tests;
