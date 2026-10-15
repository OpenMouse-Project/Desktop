//! `cargo run --example hid_probe` — lists supported HID interfaces and reads
//! the status of the first (or `--key vvvv:pppp`) through the embedded
//! protocol runtime. Handy for checking device support without the UI.

use openmouse_gpui::hid::HidService;

fn main() {
    let service = HidService::start();
    let started = std::time::Instant::now();
    let candidates = futures::executor::block_on(service.list_candidates())
        .expect("runtime")
        .unwrap_or_else(|e| panic!("list failed: {e}"));
    println!("{} candidate(s) in {:?}", candidates.len(), started.elapsed());
    for c in &candidates {
        println!("  {} {} [{}]", c.info.key, c.info.product_string, c.brands.join(", "));
    }
    let wanted = std::env::args().skip_while(|a| a != "--key").nth(1);
    let Some(target) = candidates
        .iter()
        .find(|c| wanted.as_deref().map_or(true, |k| c.info.key == k))
    else {
        return;
    };
    let started = std::time::Instant::now();
    match futures::executor::block_on(service.connect(&target.info.key)).expect("runtime") {
        Ok(device) => println!(
            "connected {} via {} ({}) in {:?}\n{}",
            device.key,
            device.driver,
            device.brand,
            started.elapsed(),
            serde_json::to_string_pretty(&device.status).unwrap()
        ),
        Err(error) => println!("connect failed: {error}"),
    }
}
