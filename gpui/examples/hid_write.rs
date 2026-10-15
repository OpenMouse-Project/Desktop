//! `cargo run --example hid_write -- <action> <value>` — one live write
//! through the embedded runtime, printing exactly what comes back. Actions:
//! dpi, pollingRate, liftOff, surface. Targets the first listed device.

use openmouse_gpui::hid::HidService;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (action, raw) = (args[1].as_str(), args[2].as_str());
    let value: serde_json::Value = serde_json::from_str(raw).unwrap_or(serde_json::json!(raw));
    let service = HidService::start();
    let list = futures::executor::block_on(service.list_candidates()).unwrap().unwrap();
    let target = &list[0];
    let device = futures::executor::block_on(service.connect(&target.info.key)).unwrap().unwrap();
    println!(
        "before: pollingRateHz={} supported={}",
        device.status["pollingRateHz"], device.status["supportedPollingRates"]
    );
    let started = std::time::Instant::now();
    let result = futures::executor::block_on(service.write(&target.info.key, &device.brand, action, value));
    println!("write {action}={raw} -> {result:?} in {:?}", started.elapsed());
    let after = futures::executor::block_on(service.connect(&target.info.key)).unwrap().unwrap();
    println!("after: pollingRateHz={}", after.status["pollingRateHz"]);
}
