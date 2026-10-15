//! `cargo run --example hid_tabs -- <vvvv:pppp>` — what the Buttons / Lighting /
//! Advanced tabs would show for one device.
use openmouse_gpui::hid::HidService;

fn main() {
    let key = std::env::args().nth(1).expect("key like 31e3:1322");
    let service = HidService::start();
    let _ = futures::executor::block_on(service.list_candidates()).unwrap().unwrap();
    match futures::executor::block_on(service.connect(&key)).unwrap() {
        Ok(d) => {
            let t = d.tabs.unwrap_or_default();
            println!("{} / {}", d.brand, d.driver);
            println!("buttons sections: {:?}", t.buttons.iter().map(|s| (&s.title, s.controls.len())).collect::<Vec<_>>());
            println!("advanced sections: {:?}", t.advanced.iter().map(|s| (&s.title, s.controls.len())).collect::<Vec<_>>());
            println!("lighting: {:?}", t.lighting.as_ref().map(|l| (&l.modes, &l.mode, &l.color, l.write_only)));
        }
        Err(e) => println!("connect failed: {e}"),
    }
}
