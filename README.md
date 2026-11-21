# OpenMouse Desktop

A native desktop app for configuring gaming mice, built with
[GPUI](https://www.gpui.rs/). It talks to mice through the drivers in
[`@openmouse/protocol`](https://github.com/OpenMouse-Project/mouse-protocol),
which run inside an embedded QuickJS runtime on top of a native HID layer.

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
