//! Pushes a few realistic statuses to the running Discord, then clears it, and
//! prints what Discord says it will display.
use openmouse_gpui::presence_text::{compose, Facts, Share};

fn main() {
    let presence = openmouse_presence::Presence::default();
    println!("enable -> {:?}", presence.enable());
    let mouse = Facts { running_game: None, device_name: Some("PRO X SUPERLIGHT 2c"), dpi: Some(800.0), polling_hz: Some(1000.0) };
    let cases = [
        ("no game, everything shared", Share { game: true, device: true, stats: true }, mouse.clone()),
        ("mouse only (one line)", Share { game: false, device: true, stats: false }, mouse.clone()),
        ("in a game", Share { game: true, device: true, stats: true }, Facts { running_game: Some("Counter-Strike 2"), ..mouse.clone() }),
    ];
    for (label, share, facts) in cases {
        let (details, state) = compose(share, &facts);
        println!("[{label}] details={details:?} state={state:?}");
        println!("  -> {:?}", presence.update_activity(details, state));
        std::thread::sleep(std::time::Duration::from_millis(1800));
    }
    println!("disable -> {:?}", presence.disable());
}
