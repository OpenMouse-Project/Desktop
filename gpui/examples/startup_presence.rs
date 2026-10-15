//! Smoke test for launch-on-startup and Discord Rich Presence. Leaves both off.
use openmouse_gpui::autostart;

fn main() {
    println!("autostart before: {}", autostart::is_enabled());
    println!("enable -> {:?}", autostart::enable());
    println!("autostart after enable: {}", autostart::is_enabled());
    println!("disable -> {:?}", autostart::disable());
    println!("autostart after disable: {}", autostart::is_enabled());

    let presence = openmouse_presence::Presence::default();
    println!("discord enable -> {:?}", presence.enable());
    std::thread::sleep(std::time::Duration::from_secs(2));
    println!("discord disable -> {:?}", presence.disable());
}
