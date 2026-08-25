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
    version: "0.0.9",
    date: "2026-08-25",
    changes: [
      "Overview is wider (720px → 960px) so the device card has more room to breathe — Games and Settings are unchanged",
    ],
  },
  {
    version: "0.0.8",
    date: "2026-08-25",
    changes: [
      "Fixed: the game-switch overlay could land partly under the Windows taskbar on a \"bottom\" corner — now positioned against the screen's actual usable area, not its full size",
      "Removed the overlay's size options — it's always the size that used to be \"Large\"",
      "Overlay position picker is text (TL/TR/BL/BR) instead of icons",
    ],
  },
  {
    version: "0.0.7",
    date: "2026-08-25",
    changes: [
      "Fixed: the game-switch overlay showed a scrollbar and could overflow its box on the \"Small\" size — it's now a single-line pill that always fits",
      "Fixed: the overlay's position picker in Settings could wrap onto two rows — now a compact row of 4",
      "Simplified the overlay to a minimal single-line toast instead of a title+description card",
    ],
  },
  {
    version: "0.0.6",
    date: "2026-08-25",
    changes: [
      "Game-switch notifications now show as a small always-on-top overlay instead of only inside the app window — visible even while a game is fullscreen (OS notifications were skipped since most systems auto-suppress those during fullscreen games)",
      "Settings → Game-switch overlay: choose which corner it appears in and how big it is, with a Test button to preview it",
    ],
  },
  {
    version: "0.0.5",
    date: "2026-08-25",
    changes: [
      "Check for Updates no longer installs automatically — it now shows what version was found and waits for you to confirm before downloading and restarting",
    ],
  },
  {
    version: "0.0.4",
    date: "2026-08-25",
    changes: [
      "Fixed: a game profile could fail to apply with \"this device is already busy\" if it landed at the same moment as the background status refresh — writes now retry for a few seconds instead of failing outright",
    ],
  },
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
