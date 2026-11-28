# OpenMouse Desktop

A native desktop app for configuring gaming mice, with keyboard support
planned. It is built with [GPUI](https://www.gpui.rs/) and talks to mice through
the drivers in
[`@openmouse/protocol`](https://github.com/OpenMouse-Project/mouse-protocol),
which run inside an embedded QuickJS runtime on top of a native HID layer.
Keyboard drivers will come from
[`@openmouse/keyboard-protocol`](https://github.com/OpenMouse-Project/keyboard-protocol).

## Checklist

The goal is to support every device in the mouse and keyboard protocols with
its controls, and to let the app update itself.

### Mice

`@openmouse/protocol` 0.62.0 has 109 driver entries. The app picks them up
automatically, but each control has to be wired in by hand.

- [ ] Wire up the missing controls. 95 of 618 driver setting methods are never
  referenced by the app. The counts come from matching method names, so some may
  be aliases or internal methods; check each before building it.

  | Driver | Missing | Examples |
  |---|---|---|
  | SteelSeries | 19 | zone colour, LED effects, sleep and dim timers |
  | Finalmouse | 8 | profile names and data, LED settings |
  | Keychron | 8 | keycodes, encoder, lift-off scale |
  | Hitscan | 7 | tracking mode, SPDT, click timings |
  | Endgame Gear | 6 | sensor angle, max sensor FPS |
  | Logitech | 6 | DPI stages per profile, bunny hopping |
  | ATK | 5 | button assignments, macros |

- [ ] Cover the smaller gaps (1–3 each), mostly DPI stage colour, per-axis DPI
  and macros
- [ ] Enable Wooting mice, which are hidden in `gpui/js/native-hid/brands.ts`
- [ ] Check each brand on real hardware or a mock, and track which are verified

### Keyboards

`@openmouse/keyboard-protocol` 1.8.0 is installed, but the app does not import
it yet.

- [ ] Detect keyboards using its driver registry
- [ ] Add keyboard cards and pages to the UI
- [ ] Build shared controls: actuation, rapid trigger, key remapping, layers,
  lighting, profiles and macros, as each driver allows
- [ ] Support each driver:
  - [ ] Wooting
  - [ ] Razer
  - [ ] Logitech
  - [ ] SteelSeries
  - [ ] Keychron
  - [ ] ATK
  - [ ] MCHOSE
  - [ ] Glorious
  - [ ] Arbiter Studio
  - [ ] Wuque Studio
  - [ ] Kochi
  - [ ] VIA
- [ ] Add keyboard mocks and tests

### Desktop app updates

The app already checks for a new version at startup and every 6 hours, has a
manual check button and a notification, and a Windows release workflow publishes
the installer with its SHA-256.

- [ ] Publish a first release, so the check has something to find
- [ ] Download the installer inside the app, with progress
- [ ] Verify its SHA-256 before running it
- [ ] Close the app, install and relaunch (confirm the installer copes with a
  running app)
- [ ] Handle failed or interrupted updates
- [ ] Code-sign the Windows build; a checksum from the same release does not
  prove who published it
- [ ] Add macOS to releases, with signing and notarization (the DMG is only
  ad-hoc signed)
- [x] Fix "View release" on macOS, which launches Windows `cmd`
- [ ] Add an automatic updates setting

### Logging and crash reporting

- [ ] Write log files with a size limit, including device connections, failed
  reads and writes, and update checks
- [ ] Capture errors from the embedded JavaScript drivers
- [ ] Add a panic handler that saves a crash report with the app version, OS and
  backtrace
- [ ] Offer to show or export the report on the next launch; send nothing
  without the user's consent
- [ ] Add an "Open logs folder" button
- [ ] Keep serial numbers and personal file paths out of logs and reports

### Safe device writes

- [ ] Show an error when a write fails or is unsupported
- [ ] Re-read the setting after a write, so the UI shows what the device has
- [ ] Ask for confirmation before resets

### Report my device

- [ ] Add a "Report my device" button for hardware we do not own
- [ ] Export the vendor and product IDs, HID descriptors and app version,
  without serial numbers
- [ ] Show the report to the user before anything is shared

### Linux

- [ ] Build and run the app on Linux
- [ ] Get HID access without root, with udev rules
- [ ] Check the platform features: tray, autostart, overlay, wallpaper theme,
  game detection and opening links
- [ ] Add a Linux build workflow
- [ ] Package it and include it in releases and app updates

## Layout

- `gpui/` — the app. `gpui/js/` is the TypeScript bundled into the app
  (control layouts, device scanning, game profiles); `gpui/installer/` builds
  the Windows installer.
- `crates/openmouse-hid` — native HID transport. `crates/openmouse-presence` —
  Discord Rich Presence.
- `public/` — `games.json` (served from the CDN) and device artwork.

## Building

Needs Rust, Node.js and the repo's npm dependencies.

```sh
npm ci                 # esbuild + the protocol drivers (used by gpui/build.rs)
cd gpui
cargo run              # debug build
cargo test
cargo build --release
```

Memory and CPU figures from `cargo run` are not representative: the debug
profile keeps roughly three times the resident memory of an optimized build.
For profiling, build the CI profile and run that binary:

```sh
cargo build --profile ci
./target/ci/openmouse-gpui
```

`OPENMOUSE_MOCK=slx` (or `ulx`, `x3`, comma separated; a trailing `!` hides
real devices) lists fake devices for looking at their UI without hardware.

Releases are cut by pushing a `v*.*.*` tag; see `.github/workflows/release.yml`.
