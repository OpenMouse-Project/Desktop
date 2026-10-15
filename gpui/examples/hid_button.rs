//! `cargo run --example hid_button` — button-remap path check. Assigns button 0
//! the action it ALREADY has, which the protocol library detects as "no change"
//! and skips the flash write for, so it proves the call chain without altering
//! anything.
use openmouse_gpui::hid::HidService;

fn main() {
    let service = HidService::start();
    let list = futures::executor::block_on(service.list_candidates()).unwrap().unwrap();
    let key = list.iter().find(|c| c.brands.iter().any(|b| b == "Logitech")).expect("Logitech").info.key.clone();
    let d = futures::executor::block_on(service.connect(&key)).unwrap().unwrap();
    println!("mode before: {}", d.status["deviceMode"]);
    let _ = futures::executor::block_on(service.onboard_action(&key, "mode", serde_json::json!("Onboard"))).unwrap();
    let read = futures::executor::block_on(service.onboard_profiles(&key)).unwrap().unwrap();
    println!("actions: {}", read.actions.join(", "));
    let running = read.profiles.iter().find(|p| p.is_current).expect("a running profile");
    let current: Vec<String> = running.button_assignments.iter().map(|a| a["action"].as_str().unwrap_or("?").to_string()).collect();
    println!("running profile {} buttons: {current:?}", running.sector);
    let same = current[0].clone();
    let r = futures::executor::block_on(service.write(&key, "Logitech", "control:logi.button.primary.0", serde_json::json!(same)));
    println!("assign button 0 = {same:?} (unchanged) -> {r:?}");
    let after = futures::executor::block_on(service.onboard_profiles(&key)).unwrap().unwrap();
    let after_actions: Vec<String> = after.profiles.iter().find(|p| p.is_current).unwrap().button_assignments.iter().map(|a| a["action"].as_str().unwrap_or("?").to_string()).collect();
    println!("after: {after_actions:?} (identical: {})", after_actions == current);
}
