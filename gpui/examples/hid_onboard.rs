//! `cargo run --example hid_onboard` — read-only: the Logitech mouse's stored profiles.
use openmouse_gpui::hid::HidService;

fn main() {
    let service = HidService::start();
    let list = futures::executor::block_on(service.list_candidates()).unwrap().unwrap();
    let key = list.iter().find(|c| c.brands.iter().any(|b| b == "Logitech")).expect("a Logitech device").info.key.clone();
    let d = futures::executor::block_on(service.connect(&key)).unwrap().unwrap();
    println!("mode={} format={}", d.status["deviceMode"], d.status["onboardProfileFormat"]);
    if let Some(mode) = std::env::args().nth(1) {
        let r = futures::executor::block_on(service.onboard_action(&key, "mode", serde_json::json!(mode))).unwrap();
        println!("set mode {mode} -> {r:?}");
    }
    let started = std::time::Instant::now();
    match futures::executor::block_on(service.onboard_profiles(&key)).unwrap() {
        Ok(read) => {
            let profiles = read.profiles;
            println!("{} profile(s), {} button actions, in {:?}", profiles.len(), read.actions.len(), started.elapsed());
            for p in &profiles {
                let dpis: Vec<String> = p.dpi_stages.iter().map(|s| s["x"].to_string()).collect();
                println!(
                    "  sector {} name={:?} enabled={} current={} dpi=[{}] default={:?} wl={:?} wired={:?} buttons={} crc={}",
                    p.sector, p.name, p.enabled, p.is_current, dpis.join(","), p.default_dpi_index,
                    p.report_rate_wireless, p.report_rate_wired, p.button_assignments.len(), p.crc_valid
                );
            }
        }
        Err(e) => println!("read failed: {e}"),
    }
}
