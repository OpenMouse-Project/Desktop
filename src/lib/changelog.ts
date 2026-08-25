// Hand-maintained changelog shown in Settings (components/ChangelogModal.tsx)
// — not generated from commit history or tauri.conf.json's own version
// field, since neither reliably says what actually matters to someone
// deciding whether to install an update. Add a new entry at the top each
// time tauri.conf.json's `version` is bumped for a release.

export interface ChangelogEntry {
  version: string;
  /** ISO date (YYYY-MM-DD), not a Date — this is display-only, never compared. */
  date: string;
  changes: string[];
}

export const CHANGELOG: ChangelogEntry[] = [
  {
    version: "0.0.1",
    date: "2026-08-24",
    changes: [
      "Device control: DPI presets and custom X/Y axes, polling rate, lift-off distance, gaming surface mode",
      "Per-game profiles — DPI and polling rate applied automatically when a game launches, restored automatically when it closes",
      "Games page with live running-game detection",
      "In-app CPU/RAM resource monitor",
      "Auto-updates, checked automatically every 30 minutes",
      "Diagnostic log export for bug reports",
    ],
  },
];
