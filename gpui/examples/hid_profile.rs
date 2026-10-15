//! `cargo run --example hid_profile` — applies a game profile through the shared
//! `applyGameProfile` (one device session) and prints what the mouse applied.
use openmouse_gpui::{hid::HidService, profiles::GameProfile};

fn main() {
    let service = HidService::start();
    let list = futures::executor::block_on(service.list_candidates()).unwrap().unwrap();
    let key = list[0].info.key.clone();
    let d = futures::executor::block_on(service.connect(&key)).unwrap().unwrap();
    println!("before: dpi={} poll={} surface={}", d.status["dpi"], d.status["pollingRateHz"], d.status["gamingSurfaceMode"]);
    // Same values as now, so a test run changes nothing.
    let profile = GameProfile {
        dpi: d.status["dpi"].as_f64(),
        polling: d.status["pollingRateHz"].as_u64().map(|v| v as u32),
        surface: d.status["gamingSurfaceMode"].as_str().map(str::to_string),
        ..Default::default()
    };
    println!("profile: {}", profile.describe());
    let started = std::time::Instant::now();
    let r = futures::executor::block_on(service.apply_profile(&key, profile.to_js()));
    println!("apply -> {r:?} in {:?}", started.elapsed());
}
