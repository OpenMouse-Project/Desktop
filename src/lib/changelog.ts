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
    version: "0.0.3",
    date: "2026-08-25",
    changes: [
      "Overview now shows when a game profile has taken over the mouse — a banner with the game's name, live DPI/polling rate numbers that match what's actually applied",
      "The Performance tab locks (with an explanation) while a game profile is in control, so editing your defaults mid-game can't get silently overwritten or redefine what \"default\" means",
    ],
  },
  {
    version: "0.0.2",
    date: "2026-08-25",
    changes: [
      "Fixed: game profiles (DPI/polling rate) could silently fail to apply — the profile editor now saves as you edit instead of requiring a separate Save click",
      "Applying a game profile now reaches the mouse faster and more reliably (DPI and polling rate share one connection instead of two)",
    ],
  },
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
