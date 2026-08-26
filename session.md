# OpenMouse Desktop — Session Log

## 2026-08-24

### Evening Session — Project Setup & Core Features

**22:04** — Initial commit: game device profiles, resource/game monitoring, logging, Windows CI build

**22:16** — Fix `set_open_exclusive()` gated to macOS only (doesn't exist on Windows)

**22:57** — Wire up auto-updates: signed releases, background check every 30 min, changelog UI

**23:01** — Restart versioning at v0.0.1, relabel app "Alpha" instead of "Beta"

**23:19** — Fix game profiles silently not applying (panel discarded unsaved changes on close)

**23:24** — Bump to v0.0.2

---

## 2026-08-25

### Morning Session — Rapid v0.0.3–v0.0.9 Releases

**~22:00–22:28 (CDT, Aug 24)** / **~08:00–10:28 IST (Aug 25)** — 7 rapid releases fixing UX issues:

| Time (IST) | Version | Change |
|---|---|---|
| 05:09 | v0.0.1 | Initial GitHub release |
| 05:28 | v0.0.2 | Fix game profiles not applying |
| 06:17 | v0.0.3 | Overview shows active game profile with live DPI/polling, Performance tab locks during game |
| 06:29 | v0.0.4 | Retry HID writes on lock contention |
| 06:42 | v0.0.5 | Confirm before installing update |
| 06:56 | v0.0.6 | Game-switch notifications → always-on-top overlay |
| 07:05 | v0.0.7 | Fix overlay scrollbar/overflow, single-line pill toast |
| 07:14 | v0.0.8 | Fix taskbar overlap, drop size options, text corner labels |
| 07:28 | v0.0.9 | Widen Overview to 960px |

---

### Afternoon Session — Razer Viper Mini Support (v0.0.10)

**~10:30 IST** — User reports Razer Viper Mini doesn't connect via Tauri desktop app, but works fine in browser via WebHID. Starts deep investigation.

#### Investigation Phase

- Confirmed Razer HID descriptor quirk: Razer uses 90-byte feature reports with report ID 0, but does **NOT** declare them in the HID descriptor (`node_modules/@openmouse/protocol/dist/razer/codec.js:2-7`)
- On Windows, `IOCTL_HID_GET_FEATURE` goes through the HID minidriver which validates report IDs against the descriptor → rejects undeclared reports → `ERROR_INVALID_FUNCTION` (0x01) or `ERROR_INVALID_PARAMETER` (0x57)
- WebHID works in browsers because browsers don't validate report IDs against the descriptor
- Analyzed hidapi 2.6.6 Windows source — confirmed `open_path` behavior, `windows_hid_enumerate` module was added but turned out unnecessary (hidapi already finds all sub-collection paths including `&colNN`)
- Collected device tree from user's system via PowerShell

#### Bugs Found & Fixed

**1. Feature report buffer too small** (`src/native-hid/tauri-hid-device.ts`)
- `receiveFeatureReport` hardcoded `length: 64` but Razer packets are 90 bytes
- Fixed: increased to `length: 90`

**2. Report ID byte not stripped from response** (`src-tauri/src/hid.rs`)
- On Windows, `IOCTL_HID_GET_FEATURE`'s `BytesReturned` includes the report ID byte (1 + data_length = 91)
- WebHID's `receiveFeatureReport` returns data only (90 bytes)
- Fixed: platform-aware stripping — on Windows `data_end = read`, on other platforms `data_end = 1 + read`; returns `buffer[1..data_end]`

**Diagnostic logging added** to `hid_list_interfaces` and `hid_open` (per-path usage page/usage, open success/failure)

**Stale filter removed** — `is_live_input_collection()` filter removed from `hid_list_interfaces` (non-exclusive open already prevents cursor freeze)

#### Test Results

- After both fixes compiled: Razer Viper Mini connects and responds
- RazerHidClient initially timed out ("mouse stayed busy") — this was before the report-ID-stripping fix was compiled
- After compiling the report-ID fix: device works

#### Release

**16:39 IST** — Commit `c54ce9c`: "Fix Razer Viper Mini support on Windows"
- Bumped version 0.0.7 → 0.0.10 (0.0.8 and 0.0.9 were already released)
- Updated `changelog.ts` with v0.0.10 entry

**16:41 IST** — Pushed to main. CI triggered but **failed** because `tauri.conf.json` had a missing comma after the version line (merge conflict residue).

**16:42 IST** — Deleted manually-created release (showed "jazzstack" instead of "github-actions", missing assets)

**16:43 IST** — Commit `cd47726`: "Fix missing comma in tauri.conf.json"
- Force-pushed `v0.0.10` tag to trigger the Release workflow
- Both "Build Windows" and "Release" CI workflows triggered

#### Files Changed (v0.0.10)

| File | Change |
|---|---|
| `src-tauri/src/hid.rs` | Report-ID stripping in `hid_get_feature_report`, diagnostic logging, removed `is_live_input_collection` filter |
| `src/native-hid/tauri-hid-device.ts` | `receiveFeatureReport` buffer 64 → 90 bytes |
| `src/native-hid/brands.ts` | Updated Razer comment |
| `src/lib/changelog.ts` | Added v0.0.10 entry |
| `package.json` | Version 0.0.7 → 0.0.10 |
| `src-tauri/Cargo.toml` | Version 0.0.7 → 0.0.10 |
| `src-tauri/tauri.conf.json` | Version 0.0.7 → 0.0.10, fixed missing comma |

---

### Discord RPC — Dev Mode Indicator

**~16:55 IST** — Added dev mode indicator to Discord Rich Presence.

- `src-tauri/src/discord_rpc.rs`: `enable()` now checks `cfg!(debug_assertions)` — in debug builds (`npx tauri dev`) the activity shows "OpenMouse Dev" / "Dev Mode: Build {version}" / "Managing mouse settings", in release builds (`npx tauri build`) it shows "OpenMouse" / "OpenMouse Desktop" / "Managing mouse settings". Version is read from `Cargo.toml` at compile time via `env!("CARGO_PKG_VERSION")`.
- No frontend changes needed — `cfg!(debug_assertions)` is compile-time, automatically `true` in dev and `false` in production

---

### UI Refactor — Device Dashboard & Games Scanner

**~18:00 IST** — Major UI restructuring begins. User wants a clean device dashboard instead of a settings-heavy Overview page.

#### UI Refactor Phase

**AppSidebar** (`src/components/AppSidebar.tsx`) — Created new 56px icon sidebar:
- Overview (LayoutDashboard), Games (Gamepad2) nav at top
- Settings (Settings), Check for Updates (RefreshCw) at bottom
- Device indicator removed from sidebar (redundant with Overview showcase)
- Update button functional — calls `runUpdateCheck()` from `lib/update-check.ts`, shows toast

**AppHeader** (`src/components/AppHeader.tsx`) — Gutted, removed from layout:
- Battery + Connection status moved inline to Overview page "Connected" line
- Component no longer rendered in `FullDesktopView`

**FullDesktopView** (`src/layouts/FullDesktopView.tsx`) — Simplified:
- Removed `AppHeader` import/render
- Removed device tabs from layout level
- Clean: TitleBar → AppSidebar → content area

**OverviewPage** (`src/pages/OverviewPage.tsx`) — Complete rewrite:
- Own tab bar at top: Overview | Performance | Lighting | Profiles | Buttons
- Tabs are capability-driven (only show when device supports the feature)
- **Overview tab**: Large device showcase (480×320, no border/background — blends with app bg) + Current Status grid + Device Information grid (all read-only)
- **Performance tab**: Existing `DevicePerformanceTab` (disabled for non-Logitech with notice)
- **Lighting/Profiles/Buttons**: Placeholder pages for future implementation
- Battery + Connection shown inline near "Connected" status dot: `● Connected · Wired · 🔋 Battery N/A`
- Battery shows icon + "N/A" for wired mice (not hidden)
- `useEffect` pauses auto-refresh when Performance tab is active

#### UI Polish

- **Device artwork**: Larger (480×320), no card borders/background — blends seamlessly with app background
- **Tabs centered**: `justify-content: center` on `.device-tabs-bar`
- **Version badge**: Changed from `var(--faint)` to `var(--bright)` for visibility
- **Empty top bar removed**: `AppHeader` no longer rendered
- **Green hover effect on game cards**: Border + subtle green glow on hover
- **Game card grid**: `auto-fill` with `minmax(190px, 1fr)` — fills left to right, wraps naturally

#### Games Page — Installed Game Scanner

**Problem**: Games page only showed a hardcoded list from `public/games.json`. User wants to see actually installed games.

**Rust backend** (`src-tauri/src/games.rs`):
- New `scan_installed_games` command — parses Steam's `libraryfolders.vdf` and `appmanifest_*.acf` files (the same approach NVIDIA/GeForce Experience uses)
- Scans all Steam library paths (supports multiple drives)
- Scans Epic Games Store manifests (`ProgramData\Epic\EpicGamesLauncher\Data\Manifests\`)
- Returns `installed_steam_ids` (matched against known games) and `install_paths`
- Built a minimal text-VDF parser (`vdf` module) for Steam's KeyValues format

**Frontend** (`src/hooks/use-game-watcher.ts`):
- Calls `scan_installed_games` with Steam AppIDs from `games.json`
- Marks games as `installed` based on scan results
- Games sorted: installed first, then not-installed
- `Game` type extended with `installed?: boolean` field

**GamesPage UI** (`src/pages/GamesPage.tsx`):
- Split into "Installed (N)" and "More Games (N)" sections
- Installed games show `📥 Installed` badge
- Not-installed games dimmed (0.5 opacity)
- Game cards have green hover effect
- Grid uses `auto-fill` for responsive left-to-right flow

#### Files Changed

| File | Change |
|---|---|
| `src/components/AppSidebar.tsx` | New: 56px icon sidebar with nav + settings/update |
| `src/components/AppHeader.tsx` | Gutted — no longer rendered |
| `src/layouts/FullDesktopView.tsx` | Removed AppHeader, clean layout |
| `src/pages/OverviewPage.tsx` | Complete rewrite: tab bar + showcase + status + info |
| `src/pages/GamesPage.tsx` | Installed/More Games sections, scan integration |
| `src/components/DevicePerformanceTab.tsx` | Added `readOnly` prop |
| `src/hooks/use-game-watcher.ts` | Calls `scan_installed_games`, marks installed games |
| `src/App.css` | Major: sidebar, showcase, tabs, info grid, game cards, hover effects |
| `src-tauri/src/games.rs` | New: `scan_installed_games`, VDF parser, Steam/Epic scanning |
| `src-tauri/src/lib.rs` | Registered `scan_installed_games` command |

---

### Release v0.0.11 — UI Refactor, Game Scanner, Sidebar

**~17:05 IST** — Commit `1b2123e`: "UI refactor: device dashboard, installed game scanner, sidebar navigation (v0.0.11)"

- Bumped version 0.0.10 → 0.0.11 in `package.json`, `Cargo.toml`, `tauri.conf.json`
- Updated `changelog.ts` with v0.0.11 entry (6 changes)
- Created backup tag `v0.0.10-backup` and branch `backup/v0.0.10` before release

#### CI Fixes

**~17:15 IST** — `build-windows.yml` was failing on every push because `tauri build` requires `TAURI_SIGNING_PRIVATE_KEY` (only available in release workflow). Fixed by switching to `cargo build --release` + `npm run build` for the sanity check — no bundler/signing needed.

#### Release

**17:09 IST** — Tag `v0.0.11` pushed, release workflow triggered and passed (3m40s).
- Release: https://github.com/OpenMouse-Project/Desktop/releases/tag/v0.0.11
- Assets: `.exe` installer, `.msi` installer, `.sig` files, `latest.json`

---

## Key Technical Notes

### Game Scanner
- Steam stores installs in `libraryfolders.vdf` (maps library paths) + `appmanifest_<appid>.acf` (per-game metadata)
- `StateFlags & 4` indicates fully installed
- Epic Games uses JSON manifests in `ProgramData\Epic\EpicGamesLauncher\Data\Manifests\`
- Games matched by Steam AppID (from `games.json`) against installed manifests
- VDF parser is minimal — just enough for Steam's KeyValues format, case-insensitive lookups

### Razer Protocol
- 90-byte packets (`RAZER_PACKET_LENGTH = 90`), report ID 0
- `encodeRazerRequest(command, transactionId)` → `sendFeatureReport(0, packet)` → wait → `receiveFeatureReport(0)` → `decodeRazerResponse()`
- Viper Mini uses transaction ID `0xff` (`VIPER_MINI_TRANSACTION_ID`), response delay 100ms, 6 retries
- Razer does **not** declare feature reports in its HID descriptor — this is the root cause of Windows HID minidriver rejection

### HID on Windows
- `IOCTL_HID_SET_FEATURE` / `IOCTL_HID_GET_FEATURE` go through the HID minidriver
- Minidriver validates report IDs against the HID descriptor
- `BytesReturned` from `IOCTL_HID_GET_FEATURE` includes the report ID byte (unlike WebHID)
- hidapi 2.6.6 already enumerates all sub-collection paths (`&colNN`) — no custom enumeration needed

### CI Workflows
- `build-windows.yml` — runs on every push to main, sanity-checks compilation
- `release.yml` — triggers on `v*.*.*` tags or `workflow_dispatch`, builds signed installers via `tauri-apps/tauri-action`, creates GitHub release as `github-actions[bot]`
- Assets: `.exe` installer, `.msi` installer, `.sig` files, `latest.json` (for auto-updater)

---

## 2026-08-26

### Device Control & Conflict Detection Session

**~Evening** — Razer Viper Mini full device control, critical conflict modal, UX fixes.

#### Razer Device Controls (wired to Production)

- **`src/native-hid/razer-actions.ts`** (new) — Write actions: `setDpi`, `setPollingRate`, `setLighting` wrapping `RazerViperMiniHidClient`
- **`src/components/DevicePerformanceTab.tsx`** — Brand-aware DPI (Razer 100–8500, Logitech 50–32000), staged-changes model, unified single DPI input
- **`src/components/DeviceLightingTab.tsx`** (new) — Effect picker, color pickers, speed picker, staged-changes model, Apply/Revert bar
- **`src/pages/OverviewPage.tsx`** — `canControl` includes `"Razer"`, imports/renders `DeviceLightingTab`, `ConflictingAppsModal`

#### Conflicting Apps Detection & Critical Modal

**Problem**: Razer Synapse (and other vendor software) locks HID access, causing OpenMouse to fail silently.

**Rust backend** (`src-tauri/src/conflicting_apps.rs`):
- `detect_conflicting_apps` command using `sysinfo` crate
- Scans running processes against known vendor substrings (`razerappengine`, `rzenginemon`, `razer_elevation_service`, etc.)
- Uses substring matching (`contains`) not exact match — vendor executables vary across versions
- Registered in `src-tauri/src/lib.rs`

**Frontend**:
- `use-conflicting-apps.ts` — Polls every 3s, returns list of detected apps
- `ConflictingAppsModal.tsx` — Full-screen critical modal (dark backdrop, can't click through)
  - Shows AlertTriangle icon, step-by-step instructions, detected process badges
  - On "I closed it" click → **immediate re-check** (no cooldown)
  - If app still running → modal stays with "Come on bruh, just close the process" message
  - Only disappears when process is actually gone

#### UX Fixes

**Back button redirect fix** (`src/hooks/use-mouse-connection.ts`):
- `connect()` now only calls `setView("device")` for **new** connections, not background re-reads
- Previously: auto-refresh interval fired `connect()` every 5s → unconditionally set `setView("device")` → yanked user back to device page after clicking "Devices" back button

#### Files Changed

| File | Change |
|---|---|
| `src/native-hid/razer-actions.ts` | New: Razer HID write actions (setDpi, setPollingRate, setLighting) |
| `src/components/DevicePerformanceTab.tsx` | Brand-aware DPI, staged-changes, unified input |
| `src/components/DeviceLightingTab.tsx` | New: Razer lighting controls with apply bar |
| `src/components/ConflictingAppsModal.tsx` | New: critical modal with strict re-check |
| `src/hooks/use-conflicting-apps.ts` | New: polls detect_conflicting_apps every 3s |
| `src/pages/OverviewPage.tsx` | Razer canControl, lighting tab, modal integration |
| `src/hooks/use-mouse-connection.ts` | Fix: auto-refresh no longer forces view to "device" |
| `src-tauri/src/conflicting_apps.rs` | New: detect_conflicting_apps command |
| `src-tauri/src/lib.rs` | Registered conflicting_apps command |
| `src/App.css` | Modal styles, apply bar, input focus glow |
