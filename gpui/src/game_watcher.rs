//! Lifetime-bound background detection. Process and install scans have separate
//! owners so launcher I/O cannot delay the two-second running-game poll.

use crate::activity::ACTIVITY;
use crate::app::AppView;
use crate::games::{self, Game};
use gpui_kit::{Context, Task};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use sysinfo::{ProcessRefreshKind, ProcessStatus, ProcessesToUpdate, System, UpdateKind};

const PROCESS_INTERVAL: Duration = Duration::from_secs(2);
const INSTALL_INTERVAL: Duration = Duration::from_secs(10);
// While the window is not active the scans slow down rather than stop: a game
// can still launch behind the window and its profile should still apply, just
// a few seconds later. Coming back to the window rescans straight away.
const PROCESS_INTERVAL_INACTIVE: Duration = Duration::from_secs(10);
const INSTALL_INTERVAL_INACTIVE: Duration = Duration::from_secs(60);

/// Dropping the view drops these tasks, including any awaited background task.
/// The foreground callbacks capture only the weak entity supplied by GPUI.
pub(crate) struct GameWatcher {
    _processes: Task<()>,
    _installations: Task<()>,
}

impl GameWatcher {
    pub(crate) fn start(cx: &Context<AppView>) -> Self {
        let processes = cx.spawn(async move |view, cx| {
            let mut tracker = cx
                .background_executor()
                .spawn(async {
                    RunningTracker::new(games::load_catalog(), cfg!(target_os = "macos"))
                })
                .await;
            loop {
                let started = Instant::now();
                let (updated, changes) = cx
                    .background_executor()
                    .spawn(async move {
                        let changes = tracker.poll();
                        (tracker, changes)
                    })
                    .await;
                tracker = updated;
                if view
                    .update(cx, |view, cx| view.apply_game_updates(&changes, cx))
                    .is_err()
                {
                    break;
                }
                let elapsed = started.elapsed();
                ACTIVITY
                    .sleep(
                        cx.background_executor(),
                        PROCESS_INTERVAL.saturating_sub(elapsed),
                        PROCESS_INTERVAL_INACTIVE.saturating_sub(elapsed),
                    )
                    .await;
            }
        });
        let installations = cx.spawn(async move |view, cx| {
            let mut tracker = cx
                .background_executor()
                .spawn(async { InstallationTracker::new(games::load_catalog()) })
                .await;
            loop {
                let started = Instant::now();
                let (updated, changes) = cx
                    .background_executor()
                    .spawn(async move {
                        let changes = tracker.poll();
                        (tracker, changes)
                    })
                    .await;
                tracker = updated;
                if view
                    .update(cx, |view, cx| view.apply_game_updates(&changes, cx))
                    .is_err()
                {
                    break;
                }
                let elapsed = started.elapsed();
                ACTIVITY
                    .sleep(
                        cx.background_executor(),
                        INSTALL_INTERVAL.saturating_sub(elapsed),
                        INSTALL_INTERVAL_INACTIVE.saturating_sub(elapsed),
                    )
                    .await;
            }
        });
        Self {
            _processes: processes,
            _installations: installations,
        }
    }
}

/// Each detector owns only its status field; neither replaces catalog entries.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct GameUpdate {
    pub(crate) id: String,
    pub(crate) installed: Option<bool>,
    pub(crate) running: Option<bool>,
}

pub(crate) fn apply_updates(catalog: &mut [Game], updates: &[GameUpdate]) -> bool {
    let mut changed = false;
    for update in updates {
        let Some(game) = catalog.iter_mut().find(|game| game.id == update.id) else {
            continue;
        };
        if let Some(installed) = update.installed {
            changed |= game.installed != installed;
            game.installed = installed;
        }
        if let Some(running) = update.running {
            changed |= game.running != running;
            game.running = running;
        }
    }
    changed
}

struct RunningTracker {
    system: System,
    matches: ProcessMatches,
}

impl RunningTracker {
    fn new(catalog: Vec<Game>, macos: bool) -> Self {
        Self {
            system: System::new(),
            matches: ProcessMatches::new(catalog, macos),
        }
    }

    fn poll(&mut self) -> Vec<GameUpdate> {
        // Retain System and refresh just executable identity, not CPU, memory,
        // command lines or Linux threads. Remove exited processes on every poll.
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .without_tasks()
                .with_exe(UpdateKind::OnlyIfNotSet),
        );
        self.matches.begin_poll();
        for process in self.system.processes().values() {
            if matches!(
                process.status(),
                ProcessStatus::Zombie | ProcessStatus::Dead
            ) {
                continue;
            }
            // An executable path is authoritative when available. This also
            // avoids macOS's truncated process names for long game executables.
            let name = process
                .exe()
                .and_then(|path| path.file_name())
                .unwrap_or_else(|| process.name());
            if let Some(name) = name.to_str() {
                self.matches.observe(name);
            }
        }
        self.matches.finish_poll()
    }
}

struct ProcessMatches {
    game_ids: Vec<String>,
    executables: HashMap<String, Vec<usize>>,
    running: Vec<bool>,
    next_running: Vec<bool>,
    name_buffer: String,
}

impl ProcessMatches {
    fn new(catalog: Vec<Game>, macos: bool) -> Self {
        let mut executables: HashMap<String, Vec<usize>> = HashMap::new();
        let mut game_ids = Vec::with_capacity(catalog.len());
        for (index, game) in catalog.into_iter().enumerate() {
            for executable in game.executables {
                let name = executable_basename(&executable).to_ascii_lowercase();
                if name.is_empty() {
                    continue;
                }
                if macos {
                    if let Some(native) = name.strip_suffix(".exe") {
                        executables
                            .entry(native.to_owned())
                            .or_default()
                            .push(index);
                    }
                }
                executables.entry(name).or_default().push(index);
            }
            // The macOS in-game executable inside the League/TFT .app bundle
            // has no spaces. Do not match LeagueClient/RiotClient helpers.
            if macos && game.id == "league-of-legends" {
                executables
                    .entry("leagueoflegends".into())
                    .or_default()
                    .push(index);
            }
            game_ids.push(game.id);
        }
        let count = game_ids.len();
        Self {
            game_ids,
            executables,
            running: vec![false; count],
            next_running: vec![false; count],
            name_buffer: String::new(),
        }
    }

    fn begin_poll(&mut self) {
        self.next_running.fill(false);
    }

    fn observe(&mut self, name: &str) {
        // Reuse one buffer rather than allocating for every system process.
        self.name_buffer.clear();
        self.name_buffer.push_str(executable_basename(name));
        self.name_buffer.make_ascii_lowercase();
        if let Some(indices) = self.executables.get(self.name_buffer.as_str()) {
            for &index in indices {
                self.next_running[index] = true;
            }
        }
    }

    fn finish_poll(&mut self) -> Vec<GameUpdate> {
        let mut changes = Vec::new();
        for (index, (previous, running)) in
            self.running.iter_mut().zip(&self.next_running).enumerate()
        {
            if *previous != *running {
                changes.push(GameUpdate {
                    id: self.game_ids[index].clone(),
                    installed: None,
                    running: Some(*running),
                });
                *previous = *running;
            }
        }
        changes
    }
}

fn executable_basename(name: &str) -> &str {
    name.rsplit(['/', '\\']).next().unwrap_or(name)
}

struct InstallationTracker {
    catalog: Vec<Game>,
    installed: Vec<bool>,
}

impl InstallationTracker {
    fn new(catalog: Vec<Game>) -> Self {
        let installed = catalog.iter().map(|game| game.installed).collect();
        Self { catalog, installed }
    }

    fn poll(&mut self) -> Vec<GameUpdate> {
        games::mark_installed(&mut self.catalog);
        self.updates()
    }

    fn updates(&mut self) -> Vec<GameUpdate> {
        let mut changes = Vec::new();
        for (game, previous) in self.catalog.iter().zip(&mut self.installed) {
            if game.installed != *previous {
                changes.push(GameUpdate {
                    id: game.id.clone(),
                    installed: Some(game.installed),
                    running: None,
                });
                *previous = game.installed;
            }
        }
        changes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(id: &str, executable: &str) -> Game {
        Game {
            id: id.into(),
            name: format!("Name {id}"),
            artwork: Some(format!("https://example.com/{id}.jpg")),
            steam_app_id: None,
            epic_id: None,
            riot_id: None,
            executables: vec![executable.into()],
            installed: false,
            running: false,
        }
    }

    fn poll(matcher: &mut ProcessMatches, names: &[&str]) -> Vec<GameUpdate> {
        matcher.begin_poll();
        for name in names {
            matcher.observe(name);
        }
        matcher.finish_poll()
    }

    #[test]
    fn windows_matches_exact_case_insensitive_basenames_only() {
        let mut matcher = ProcessMatches::new(vec![game("valorant", "VALORANT.exe")], false);
        assert!(poll(
            &mut matcher,
            &["valorant", "VALORANT.exe.helper", "ValorantHelper.exe"]
        )
        .is_empty());
        assert_eq!(
            poll(&mut matcher, &[r"C:\\Riot Games\\VALORANT\\valorant.EXE"]),
            vec![GameUpdate {
                id: "valorant".into(),
                installed: None,
                running: Some(true),
            }]
        );
        assert!(poll(&mut matcher, &["VALORANT.EXE"]).is_empty());
    }

    #[test]
    fn macos_matches_native_exe_equivalents_and_league_game_not_clients() {
        let mut matcher = ProcessMatches::new(
            vec![game("league-of-legends", "League of Legends.exe")],
            true,
        );
        for helper in [
            "LeagueClient",
            "LeagueClientUx",
            "RiotClientServices",
            "LeagueofLegends Helper",
        ] {
            assert!(poll(&mut matcher, &[helper]).is_empty());
        }
        assert_eq!(
            poll(&mut matcher, &["/Applications/League of Legends.app/Contents/LoL/Game/League of Legends.app/Contents/MacOS/LeagueofLegends"])[0].running,
            Some(true)
        );
        assert!(poll(&mut matcher, &["League of Legends"]).is_empty());
        assert!(poll(&mut matcher, &["LEAGUE OF LEGENDS.EXE"]).is_empty());
        assert_eq!(
            poll(&mut matcher, &["LeagueClient"])[0].running,
            Some(false)
        );
    }

    #[test]
    fn running_lifecycle_notifies_only_transitions_and_survives_one_process_exit() {
        let mut matcher = ProcessMatches::new(vec![game("cs2", "cs2.exe")], true);
        let mut catalog = vec![game("cs2", "cs2.exe")];
        assert!(poll(&mut matcher, &[]).is_empty());
        assert!(apply_updates(
            &mut catalog,
            &poll(&mut matcher, &["cs2", "CS2.EXE"])
        ));
        assert!(catalog[0].running);
        assert!(!catalog[0].installed);
        assert!(poll(&mut matcher, &["cs2.exe"]).is_empty());
        assert!(apply_updates(&mut catalog, &poll(&mut matcher, &[])));
        assert!(!catalog[0].running);
        assert!(poll(&mut matcher, &[]).is_empty());
        assert!(apply_updates(&mut catalog, &poll(&mut matcher, &["cs2"])));
    }

    #[test]
    fn status_updates_follow_stable_ids_preserve_art_and_ignore_unknown_ids() {
        let mut catalog = vec![game("other", "other.exe"), game("cs2", "cs2.exe")];
        catalog[1].name = "Custom name".into();
        let original_art = catalog[1].artwork.clone();
        let update = GameUpdate {
            id: "cs2".into(),
            installed: None,
            running: Some(true),
        };
        assert!(apply_updates(&mut catalog, std::slice::from_ref(&update)));
        assert!(!apply_updates(&mut catalog, std::slice::from_ref(&update)));
        assert!(!catalog[0].running);
        assert!(catalog[1].running);
        assert!(!catalog[1].installed);
        assert_eq!(catalog[1].name, "Custom name");
        assert_eq!(catalog[1].artwork, original_art);
        assert_eq!(catalog[1].executables, ["cs2.exe"]);
        assert!(!apply_updates(
            &mut catalog,
            &[GameUpdate {
                id: "removed".into(),
                installed: Some(true),
                running: Some(true),
            }]
        ));
    }

    #[test]
    fn installation_lifecycle_does_not_reset_concurrent_running_state() {
        let mut tracker = InstallationTracker::new(vec![game("cs2", "cs2.exe")]);
        let mut catalog = vec![game("cs2", "cs2.exe")];
        catalog[0].running = true;
        assert!(tracker.updates().is_empty());
        tracker.catalog[0].installed = true;
        assert!(apply_updates(&mut catalog, &tracker.updates()));
        assert!(catalog[0].installed && catalog[0].running);
        assert!(tracker.updates().is_empty());
        tracker.catalog[0].installed = false;
        assert!(apply_updates(&mut catalog, &tracker.updates()));
        assert!(!catalog[0].installed && catalog[0].running);
        assert!(tracker.updates().is_empty());
    }

    #[test]
    fn shared_executable_marks_all_configured_games_without_guessing_helpers() {
        let mut matcher = ProcessMatches::new(
            vec![game("first", "shared.exe"), game("second", "shared.exe")],
            false,
        );
        assert!(poll(&mut matcher, &["shared-helper.exe"]).is_empty());
        let updates = poll(&mut matcher, &["shared.exe"]);
        assert_eq!(updates.len(), 2);
        assert!(updates.iter().all(|update| update.running == Some(true)));
    }
}
