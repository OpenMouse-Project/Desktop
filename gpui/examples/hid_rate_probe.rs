//! Diagnostic: write a polling rate, then re-read it over time, to tell
//! "the mouse never took it" apart from "it took it and reverted".
use openmouse_gpui::hid::HidService;

fn main() {
    let rate: u64 = std::env::args().nth(1).and_then(|v| v.parse().ok()).expect("rate");
    let service = HidService::start();
    let list = futures::executor::block_on(service.list_candidates()).unwrap().unwrap();
    let key = list[0].info.key.clone();
    let read = |label: &str| {
        let d = futures::executor::block_on(service.connect(&key)).unwrap().unwrap();
        println!("{label}: pollingRateHz={} mode={}", d.status["pollingRateHz"], d.status["deviceMode"]);
        d
    };
    let d = read("before");
    let r = futures::executor::block_on(service.write(&key, &d.brand, "pollingRate", serde_json::json!(rate)));
    println!("write {rate} -> {r:?}");
    for (i, wait) in [0u64, 1, 3, 6].iter().enumerate() {
        std::thread::sleep(std::time::Duration::from_secs(*wait));
        read(&format!("read {i} (+{wait}s)"));
    }
}
