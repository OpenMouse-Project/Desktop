//! `cargo run --example chime_demo` — plays the success chime, then the error chime.
use openmouse_gpui::audio::{play, Chime};

fn main() {
    println!("success chime accepted: {}", play(Chime::Success));
    std::thread::sleep(std::time::Duration::from_millis(900));
    println!("error chime accepted:   {}", play(Chime::Error));
    std::thread::sleep(std::time::Duration::from_millis(900));
}
