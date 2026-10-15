//! `cargo run --example hid_rescan` — list/connect repeatedly in one process,
//! the path the Overview's Rescan button takes. Regression check for HidApi
//! being touched from more than one thread (aborts inside IOHID on macOS).

use openmouse_gpui::hid::HidService;

fn main() {
    let service = HidService::start();
    for round in 1..=6 {
        let list = futures::executor::block_on(service.list_candidates())
            .expect("runtime")
            .expect("list");
        let connected = list.first().map(|c| {
            futures::executor::block_on(service.connect(&c.info.key))
                .expect("runtime")
                .map(|d| d.driver)
        });
        println!("round {round}: {} device(s), connect = {connected:?}", list.len());
    }
}
