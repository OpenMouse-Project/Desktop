//! Game catalog: static list + install detection.
//!
//! Mirrors `use-game-watcher.ts`'s `Game` shape and `public/games.json`.
//! The catalog ships embedded (no network at startup); artwork URLs resolve
//! lazily per card via `img()`. Native install detection reads launcher
//! manifests on macOS and Windows without touching artwork or running state.

use serde::{Deserialize, Serialize};

mod install_scan;

/// One catalog entry, mirroring the TS `Game` interface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub artwork: Option<String>,
    #[serde(default, rename = "steamAppId", deserialize_with = "de_steam_id")]
    pub steam_app_id: Option<u32>,
    #[serde(default, rename = "epicId")]
    pub epic_id: Option<String>,
    #[serde(default, rename = "riotId")]
    pub riot_id: Option<String>,
    #[serde(default)]
    pub executables: Vec<String>,
    #[serde(skip)]
    pub installed: bool,
    #[serde(skip)]
    pub running: bool,
}

impl Game {
    /// Card art URL: explicit artwork, else Steam CDN cover. Mirrors the
    /// `GameCard` src chain (`artwork ?? steam library_600x900_2x.jpg`).
    pub fn art_url(&self) -> Option<String> {
        if let Some(url) = self.artwork.as_ref() {
            return Some(url.clone());
        }
        self.steam_app_id.map(|id| {
            format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{id}/library_600x900_2x.jpg")
        })
    }

    /// Fallback letter when no art resolves (mirrors `.game-card-mark`).
    pub fn mark(&self) -> char {
        self.name.chars().next().unwrap_or('?')
    }
}

/// `steamAppId` is inconsistently typed in the catalog (number for some
/// titles, string for others) — accept both.
fn de_steam_id<'de, D>(d: D) -> Result<Option<u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize as _;
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Flexible {
        Number(u32),
        Text(String),
        Empty,
    }
    match Flexible::deserialize(d).unwrap_or(Flexible::Empty) {
        Flexible::Number(id) => Ok(Some(id)),
        Flexible::Text(text) => Ok(text.trim().parse::<u32>().ok()),
        Flexible::Empty => Ok(None),
    }
}

/// On-disk art cache: `<cache_dir>/openmouse/game-art/<id>.jpg`.
/// Prefetched at startup on a background thread (mirrors the web app's lazy
/// `<img loading="lazy">`, but ahead-of-time: one small batch instead of
/// per-card fetches during scroll). Cards read `cached_art_path` and fall
/// back to the letter tile until the file lands.
pub fn art_cache_dir() -> Option<std::path::PathBuf> {
    dirs::cache_dir().map(|dir| dir.join("openmouse").join("game-art"))
}

// Include an explicit source in cache identity so catalog artwork changes
// cannot keep displaying the previously downloaded image.
fn art_cache_path(dir: &std::path::Path, game: &Game) -> std::path::PathBuf {
    use std::hash::{Hash, Hasher};
    match game.artwork.as_deref() {
        Some(url) => {
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            url.hash(&mut hash);
            dir.join(format!("{}-{:016x}.jpg", game.id, hash.finish()))
        }
        None => dir.join(format!("{}.jpg", game.id)),
    }
}

/// Local art file for a game, if already prefetched.
pub fn cached_art_path(game: &Game) -> Option<std::path::PathBuf> {
    let path = art_cache_path(&art_cache_dir()?, game);
    path.is_file().then_some(path)
}

/// Download missing art for all games with a resolvable URL. Blocking;
/// callers must run this off the UI thread. Uses the system `curl`
/// (dependency-free) with a per-file timeout; failures are skipped, leaving
/// the letter tile in place.
pub fn prefetch_art(games: &[Game]) {
    let Some(dir) = art_cache_dir() else { return };
    let _ = std::fs::create_dir_all(&dir);
    for game in games {
        let Some(url) = game.art_url() else { continue };
        let target = art_cache_path(&dir, game);
        if target.is_file() {
            continue;
        }
        let tmp = dir.join(format!("{}.part", game.id));
        let status = crate::proc::quiet(
            std::process::Command::new("curl")
                .args(["-sSfL", "-m", "20", "-o"])
                .arg(&tmp)
                .arg(&url)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null()),
        )
        .status();
        if status.is_ok_and(|s| s.success()) && tmp.is_file() {
            let _ = std::fs::rename(&tmp, &target);
        } else {
            let _ = std::fs::remove_file(&tmp);
        }
    }
}

#[derive(Deserialize)]
struct GamesFile {
    games: Vec<Game>,
}

/// Embedded catalog (`public/games.json` at build time).
const GAMES_JSON: &str = include_str!("../../public/games.json");

/// Load the catalog; `installed` starts false until a scan completes.
pub fn load_catalog() -> Vec<Game> {
    match serde_json::from_str::<GamesFile>(GAMES_JSON) {
        Ok(file) => file.games,
        Err(e) => {
            eprintln!("games catalog failed to parse: {e}");
            Vec::new()
        }
    }
}

/// Refresh installed state from Steam, Epic, and Riot manifests. Blocking;
/// callers must run this off the UI thread. Missing launchers clear old state.
pub fn mark_installed(games: &mut [Game]) {
    install_scan::mark_installed(games);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_loads_with_art_chain() {
        let games = load_catalog();
        assert!(!games.is_empty());
        let cs2 = games.iter().find(|g| g.id == "counter-strike-2").unwrap();
        assert_eq!(
            cs2.art_url().as_deref(),
            Some("https://cdn.cloudflare.steamstatic.com/steam/apps/730/library_600x900_2x.jpg")
        );
        assert_eq!(cs2.mark(), 'C');
    }

    #[test]
    fn catalog_retains_launcher_ids_and_skips_runtime_state() {
        let games = load_catalog();
        assert_eq!(
            games
                .iter()
                .find(|game| game.id == "fortnite")
                .unwrap()
                .epic_id
                .as_deref(),
            Some("Fortnite")
        );
        assert_eq!(
            games
                .iter()
                .find(|game| game.id == "league-of-legends")
                .unwrap()
                .riot_id
                .as_deref(),
            Some("league_of_legends")
        );
        assert!(games.iter().all(|game| !game.installed && !game.running));
        let mut game = games[0].clone();
        game.installed = true;
        game.running = true;
        let json = serde_json::to_value(game).unwrap();
        assert!(json.get("installed").is_none());
        assert!(json.get("running").is_none());
        assert!(json.get("epicId").is_some());
        assert!(json.get("riotId").is_some());
    }

    #[test]
    fn explicit_artwork_wins_over_steam() {
        let game = Game {
            id: "x".into(),
            name: "X".into(),
            artwork: Some("https://example.com/a.jpg".into()),
            steam_app_id: Some(1),
            epic_id: None,
            riot_id: None,
            executables: vec![],
            installed: false,
            running: false,
        };
        assert_eq!(game.art_url().as_deref(), Some("https://example.com/a.jpg"));
    }
}
