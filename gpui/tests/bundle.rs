//! The driver bundle runs before the first device can be listed, so anything
//! slow at its top level is time the user spends looking at an empty window.

use std::time::{Duration, Instant};

use rquickjs::{Context, Runtime};

const BUNDLE: &str = include_str!("../js/dist/bundle.js");

/// Takes well under a second; the limit leaves room for a slow build machine
/// while still catching the 55-second regression this guards against.
const LIMIT: Duration = Duration::from_secs(8);

#[test]
fn the_driver_bundle_loads_quickly() {
    let runtime = Runtime::new().unwrap();
    let context = Context::full(&runtime).unwrap();
    context.with(|ctx| {
        // Just enough of the host for the top level to run.
        ctx.eval::<(), _>(
            "globalThis.__host = { platform: 'windows', log() {}, \
             sleep() { return new Promise(() => {}); }, invoke() { return new Promise(() => {}); } };",
        )
        .unwrap();
        let started = Instant::now();
        ctx.eval::<(), _>(BUNDLE).expect("the bundle runs");
        let took = started.elapsed();
        assert!(took < LIMIT, "the driver bundle took {took:.1?} to load");
    });
}
