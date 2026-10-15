use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "openmouse-install-fixture-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }

    fn file(&self, relative: &str, contents: impl AsRef<[u8]>) -> PathBuf {
        let path = self.path(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, contents).unwrap();
        path
    }

    fn app(&self, relative: &str) -> PathBuf {
        self.file(&format!("{relative}/Contents/Info.plist"), "plist fixture");
        self.file(
            &format!("{relative}/Contents/MacOS/Game"),
            "executable fixture",
        );
        self.path(relative)
    }

    fn steam(&self, library: &str, id: u32, directory: &str) {
        self.file(&format!("{library}/steamapps/appmanifest_{id}.acf"), format!(
            "\"AppState\" {{ \"AppId\" \"{id}\" \"InstallDir\" \"{directory}\" \"StateFlags\" \"4\" }}"
        ));
        std::fs::create_dir_all(self.path(&format!("{library}/steamapps/common/{directory}")))
            .unwrap();
    }

    fn epic(
        &self,
        name: &str,
        id: &str,
        path: &Path,
        executable: &str,
        incomplete: bool,
    ) -> PathBuf {
        self.file(
            &format!("epic/{name}.item"),
            serde_json::to_vec(&serde_json::json!({
                "AppName": id, "InstallLocation": path, "LaunchExecutable": executable,
                "bIsIncompleteInstall": incomplete
            }))
            .unwrap(),
        )
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn game(steam: Option<u32>, epic: Option<&str>, riot: Option<&str>) -> Game {
    Game {
        id: "fixture".into(),
        name: "Unrelated Display Name".into(),
        artwork: None,
        steam_app_id: steam,
        epic_id: epic.map(str::to_owned),
        riot_id: riot.map(str::to_owned),
        executables: Vec::new(),
        installed: false,
        running: false,
    }
}

fn quoted_path(path: &Path) -> String {
    path.to_str()
        .unwrap()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

#[test]
fn steam_scans_modern_and_legacy_extra_libraries_and_deduplicates_roots() {
    let f = Fixture::new();
    f.steam("Steam", 10, "Main Game");
    f.steam("Extra", 20, "Extra Game");
    f.steam("Legacy", 30, "Legacy Game");
    f.file("Steam/steamapps/libraryfolders.vdf", format!(
        "// heading\n\"LiBrArYfOlDeRs\" {{\n\"0\" {{ \"PaTh\" \"{}\" }}\n\"1\" {{ \"path\" \"{}\" \"apps\" {{ \"20\" \"1\" }} }}\n\"2\" {{ \"path\" \"{}\" }}\n}}",
        quoted_path(&f.path("Steam")), quoted_path(&f.path("Extra")), quoted_path(&f.path("Extra"))
    ));
    f.file(
        "Steam/config/libraryfolders.vdf",
        format!(
            "/* legacy */ libraryfolders {{ 1 \"{}\" TimeNextStatsReport \"1234\" }}",
            quoted_path(&f.path("Legacy"))
        ),
    );
    let roots = vec![f.path("Steam"), f.path("Steam/.")];
    assert_eq!(steam_libraries(&roots).len(), 3);
    let mut games = [
        game(Some(10), None, None),
        game(Some(20), None, None),
        game(Some(30), None, None),
    ];
    mark_with_roots(
        &mut games,
        &Roots {
            steam: roots,
            ..Roots::default()
        },
    );
    assert!(games.iter().all(|game| game.installed));
}

#[test]
fn vdf_decodes_windows_paths_quotes_unicode_and_comments_without_mangling_paths() {
    let text = r#"// preamble
        "LIBRARYFOLDERS" {
            "1" { "PATH" "D:\\SteamLibrary" }
            "2" "E:\\Games\\Steam"
            "3" { "path" "C:\tools\Steam" } // literal single slash is preserved
            "4" { "path" "/Volumes/遊戲/Steam \"Library\"" }
            TimeNextStatsReport "999"
        }
    "#;
    let mut paths = library_paths(text);
    paths.sort();
    let mut expected = [
        r"D:\SteamLibrary",
        r"E:\Games\Steam",
        r"C:\tools\Steam",
        "/Volumes/遊戲/Steam \"Library\"",
    ]
    .map(PathBuf::from)
    .to_vec();
    expected.sort();
    assert_eq!(paths, expected);
}

#[test]
fn malformed_vdf_is_rejected_not_partially_scanned() {
    for text in [
        r#""libraryfolders" { "1" { "path" "x" }"#,
        r#""libraryfolders" { "1" "x }"#,
        r#""libraryfolders" { "1" "x" }}"#,
        r#""libraryfolders" { "1" "x" "1" "y" }"#,
        r#""libraryfolders" { /* unterminated"#,
        r#""libraryfolders" { "1" }"#,
    ] {
        assert!(vdf::parse(text).is_none(), "accepted {text}");
        assert!(library_paths(text).is_empty());
    }
}

#[test]
fn steam_rejects_stale_incomplete_malformed_mismatched_and_traversing_manifests() {
    let f = Fixture::new();
    f.steam("Steam", 10, "Valid");
    f.steam("Steam", 20, "Removed");
    std::fs::remove_dir(f.path("Steam/steamapps/common/Removed")).unwrap();
    f.file(
        "Steam/steamapps/appmanifest_30.acf",
        r#""AppState" { "appid" "30" "installdir" "Valid" "StateFlags" "1024" }"#,
    );
    f.file(
        "Steam/steamapps/appmanifest_40.acf",
        r#""AppState" { "appid" "40" "installdir" "Valid""#,
    );
    f.file(
        "Steam/steamapps/appmanifest_50.acf",
        r#""AppState" { "appid" "10" "installdir" "Valid" }"#,
    );
    f.file(
        "Steam/steamapps/appmanifest_60.acf",
        r#""AppState" { "appid" "60" "installdir" "../.." }"#,
    );
    f.file(
        "Steam/steamapps/appmanifest_70.acf",
        r#""AppState" { "appid" "70" "installdir" "Valid" "StateFlags" "invalid" }"#,
    );
    let mut ids = HashSet::new();
    scan_steam(&[f.path("Steam")], &mut ids);
    assert_eq!(ids, HashSet::from([10]));
    std::fs::remove_dir(f.path("Steam/steamapps/common/Valid")).unwrap();
    let mut games = [game(Some(10), None, None)];
    games[0].installed = true;
    mark_with_roots(
        &mut games,
        &Roots {
            steam: vec![f.path("Steam")],
            ..Roots::default()
        },
    );
    assert!(!games[0].installed);
}

#[test]
fn epic_matches_appname_and_rejects_incomplete_stale_and_missing_payloads() {
    let f = Fixture::new();
    let location = f
        .file("Games/ArbitraryFolder/Binaries/Game.exe", "fixture")
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    f.epic("valid", "Fortnite", &location, "Binaries/Game.exe", false);
    f.epic(
        "incomplete",
        "Pending",
        &location,
        "Binaries/Game.exe",
        true,
    );
    f.epic("removed", "Gone", &f.path("Removed"), "Game.exe", false);
    f.epic("missing-exe", "Empty", &location, "missing.exe", false);
    f.epic(
        "traversal",
        "Traversal",
        &location,
        "../ArbitraryFolder/Binaries/Game.exe",
        false,
    );
    f.file("epic/broken.item", "{broken");
    f.file(
        "epic/missing-field.item",
        r#"{"AppName":"Missing","InstallLocation":"/"}"#,
    );
    let mut games = [
        game(None, Some("Fortnite"), None),
        game(None, Some("fort"), None),
        game(None, Some("Pending"), None),
    ];
    let roots = Roots {
        epic: vec![f.path("epic"), f.path("epic/.")],
        ..Roots::default()
    };
    mark_with_roots(&mut games, &roots);
    assert!(games[0].installed);
    assert!(!games[1].installed);
    assert!(!games[2].installed);
    let mut ids = HashSet::new();
    scan_epic(&roots.epic, &mut ids);
    assert_eq!(ids, HashSet::from(["Fortnite".to_owned()]));
    std::fs::remove_file(f.path("Games/ArbitraryFolder/Binaries/Game.exe")).unwrap();
    mark_with_roots(&mut games, &roots);
    assert!(!games[0].installed);
}

#[test]
fn epic_supports_mac_app_payload_and_detects_uninstallation() {
    let f = Fixture::new();
    f.app("Games/MacTitle/Game.app");
    f.epic(
        "mac",
        "NativeMacTitle",
        &f.path("Games/MacTitle"),
        "Game.app",
        false,
    );
    let roots = Roots {
        epic: vec![f.path("epic")],
        ..Roots::default()
    };
    let mut games = [game(None, Some("NativeMacTitle"), None)];
    mark_with_roots(&mut games, &roots);
    assert!(games[0].installed);
    std::fs::remove_dir_all(f.path("Games/MacTitle/Game.app/Contents/MacOS")).unwrap();
    mark_with_roots(&mut games, &roots);
    assert!(!games[0].installed);
}

#[test]
fn riot_associated_installs_use_payload_identity_even_in_custom_folders() {
    let f = Fixture::new();
    f.file("Custom/Unrelated/LeagueClient.exe", "fixture");
    f.file(
        "Custom/Other/live/ShooterGame/Binaries/Win64/VALORANT-Win64-Shipping.exe",
        "fixture",
    );
    std::fs::create_dir_all(f.path("Custom/League of Legends backup")).unwrap();
    f.file(
        "riot/RiotClientInstalls.json",
        serde_json::to_vec(&serde_json::json!({
            "associated_client": {
                f.path("Custom/Unrelated").to_str().unwrap(): "unused-client.exe",
                f.path("Custom/Other/live").to_str().unwrap(): "unused-client.exe",
                f.path("Custom/League of Legends backup").to_str().unwrap(): "unused-client.exe"
            }
        }))
        .unwrap(),
    );
    let roots = Roots {
        riot: vec![f.path("riot"), f.path("riot/.")],
        ..Roots::default()
    };
    let mut games = [
        game(None, None, Some("league_of_legends")),
        game(None, None, Some("valorant")),
        game(None, None, Some("league")),
    ];
    mark_with_roots(&mut games, &roots);
    assert!(games[0].installed && games[1].installed);
    assert!(!games[2].installed);
    std::fs::remove_file(f.path("Custom/Unrelated/LeagueClient.exe")).unwrap();
    std::fs::remove_dir_all(f.path("Custom/Other/live/ShooterGame")).unwrap();
    mark_with_roots(&mut games, &roots);
    assert!(games.iter().all(|game| !game.installed));
}

#[test]
fn riot_product_metadata_is_exact_and_uninstalled_metadata_does_not_count() {
    let f = Fixture::new();
    f.file("Custom/Live/LeagueClient.exe", "fixture");
    let settings = f.file(
        "riot/Metadata/league_of_legends.live/league_of_legends.live.product_settings.yaml",
        format!(
            "product_install_full_path: {}\nproduct_install_root: unused\n",
            serde_json::to_string(&f.path("Custom/Live")).unwrap()
        ),
    );
    f.file(
        "riot/Metadata/valorant.live/valorant.live.product_settings.yaml",
        format!(
            "product_install_full_path: {}\n",
            serde_json::to_string(&f.path("Custom/Live")).unwrap()
        ),
    );
    let roots = Roots {
        riot: vec![f.path("riot")],
        ..Roots::default()
    };
    let mut games = [
        game(None, None, Some("league_of_legends")),
        game(None, None, Some("valorant")),
    ];
    mark_with_roots(&mut games, &roots);
    assert!(games[0].installed);
    assert!(!games[1].installed);
    std::fs::write(settings, "product_install_root: unused\n").unwrap();
    mark_with_roots(&mut games, &roots);
    assert!(!games[0].installed);
}

#[test]
fn riot_mac_manifest_and_installed_bundle_fallback_clear_after_uninstall() {
    let f = Fixture::new();
    let bundle = f.path("Applications/League of Legends.app");
    f.app("Applications/League of Legends.app/Contents/LoL/LeagueClient.app");
    f.file("riot/RiotClientInstalls.json", serde_json::to_vec(&serde_json::json!({
        "associated_client": { bundle.join("Contents/LoL").to_str().unwrap(): "Riot Client.app" }
    })).unwrap());
    let mut games = [game(None, None, Some("league_of_legends"))];
    let roots = Roots {
        riot: vec![f.path("riot")],
        ..Roots::default()
    };
    mark_with_roots(&mut games, &roots);
    assert!(games[0].installed);
    let fallback = Roots {
        league_bundles: vec![bundle.clone()],
        ..Roots::default()
    };
    mark_with_roots(&mut games, &fallback);
    assert!(games[0].installed);
    std::fs::remove_dir_all(bundle).unwrap();
    mark_with_roots(&mut games, &roots);
    assert!(!games[0].installed);
    mark_with_roots(&mut games, &fallback);
    assert!(!games[0].installed);
}

#[test]
fn riot_path_scalar_accepts_launcher_formats_and_rejects_malformed_fields() {
    for (text, expected) in [
        (
            "product_install_full_path: \"D:/Riot Games/live\" # comment",
            "D:/Riot Games/live",
        ),
        (
            "product_install_full_path: 'D:/Player''s Games/live'",
            "D:/Player's Games/live",
        ),
        (
            "product_install_full_path: D:/Riot Games/live # comment",
            "D:/Riot Games/live",
        ),
        (
            r#"product_install_full_path: "D:\\Riot Games\\live""#,
            r"D:\Riot Games\live",
        ),
    ] {
        assert_eq!(product_install_path(text), Some(PathBuf::from(expected)));
    }
    for text in [
        "product_install_root: D:/Riot Games",
        "product_install_full_path: \"unterminated",
        "product_install_full_path: 'unterminated",
        "product_install_full_path: []",
        "product_install_full_path: \"x\" garbage",
        " product_install_full_path: x",
        "product_install_full_path: x\nproduct_install_full_path: y",
        "product_install_full_path: ",
    ] {
        assert!(product_install_path(text).is_none(), "accepted {text}");
    }
}

#[test]
fn missing_launchers_and_malformed_manifests_clear_installed_only() {
    let f = Fixture::new();
    f.file("riot/RiotClientInstalls.json", "malformed");
    f.file("epic/broken.item", "malformed");
    f.file("Steam/steamapps/libraryfolders.vdf", "malformed");
    let mut games = [game(Some(10), Some("Fortnite"), Some("league_of_legends"))];
    games[0].installed = true;
    games[0].running = true;
    games[0].artwork = Some("https://example.com/art.jpg".into());
    mark_with_roots(
        &mut games,
        &Roots {
            steam: vec![f.path("Steam"), f.path("MissingSteam")],
            epic: vec![f.path("epic")],
            riot: vec![f.path("riot")],
            league_bundles: vec![f.path("Absent.app")],
        },
    );
    assert!(!games[0].installed);
    assert!(games[0].running);
    assert_eq!(
        games[0].art_url().as_deref(),
        Some("https://example.com/art.jpg")
    );
}
