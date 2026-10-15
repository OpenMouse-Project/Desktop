//! The Buttons / Lighting / Advanced descriptors are produced by
//! `js/controls.ts`. `fixtures/tabs.json` is its real output for mock Razer,
//! Endgame Gear, Ninjutso, lit and bare statuses; this checks the Rust types
//! read it, so a schema change on either side fails here.

use std::collections::BTreeMap;

use openmouse_gpui::hid::Tabs;

#[test]
fn js_descriptors_deserialize() {
    let text = include_str!("fixtures/tabs.json");
    let all: BTreeMap<String, Tabs> = serde_json::from_str(text).expect("fixture parses");

    let razer = &all["razer"];
    assert_eq!(razer.buttons.len(), 2);
    assert!(razer.buttons[0].controls.iter().all(|c| c.kind == "choice" && c.options.len() == 5));
    assert!(razer.buttons[1].controls.iter().all(|c| c.kind == "info"));

    let egg = &all["egg"];
    assert!(egg.buttons[0].controls.iter().all(|c| c.options.len() == 9));
    let kinds: Vec<&str> = egg.advanced[0].controls.iter().map(|c| c.kind.as_str()).collect();
    assert!(kinds.contains(&"toggle") && kinds.contains(&"choice") && kinds.contains(&"number"));
    let divider = egg.advanced[0].controls.iter().find(|c| c.id == "egg.pollingDivider").unwrap();
    assert_eq!((divider.min, divider.max), (Some(1.0), Some(255.0)));

    assert_eq!(all["nin"].advanced[0].controls.len(), 4);

    let lit = all["lit"].lighting.as_ref().expect("lighting passes through");
    assert_eq!(lit.mode.as_deref(), Some("Static"));
    assert_eq!(lit.speeds, vec![1.0, 2.0, 3.0]);

    let none = &all["none"];
    assert!(none.buttons.is_empty() && none.advanced.is_empty() && none.lighting.is_none());
}

/// The Performance tab's extra controls, as `js/controls.ts` produces them for mock
/// Razer / Attack Shark / Zaunkoenig / hint-hidden / bare devices.
#[test]
fn js_performance_descriptors_deserialize() {
    let text = include_str!("fixtures/performance.json");
    let all: BTreeMap<String, Tabs> = serde_json::from_str(text).expect("fixture parses");
    let ids = |name: &str| -> Vec<String> {
        all[name].performance.iter().flat_map(|s| s.controls.iter().map(|c| c.id.clone())).collect()
    };

    // Razer: sleep, the three processing switches, and the low-power list.
    assert_eq!(ids("razer"), ["perf.sleep", "perf.motionSync", "perf.angleSnapping", "perf.rippleControl", "perf.lowPower"]);
    let low_power = all["razer"].performance[0].controls.iter().find(|c| c.id == "perf.lowPower").unwrap();
    // The device's own off-list 12% is kept as an option, so the page shows the truth.
    assert!(low_power.options.iter().any(|o| o.value == 12));
    assert_eq!(low_power.current, 12);

    // Attack Shark: its driver hides ripple control; on-device stages and debounce show.
    assert_eq!(ids("atk"), ["perf.motionSync", "perf.performanceMode", "perf.dpiStage", "perf.debounce"]);
    // Zaunkoenig: the driver hides the sleep card; USB speed and primary button remain.
    assert_eq!(ids("zaun"), ["perf.usbSpeed", "perf.primaryButton"]);
    // A driver that hides the whole processing card shows none of those switches.
    assert!(ids("hidden").is_empty());
    // A mouse reporting nothing extra gets nothing extra.
    assert!(ids("superlight").is_empty());
}

/// The shared remapper, profile selector and power-mode controls that newer driver
/// families report through generic status fields (`buttonMappings` + `buttonOptions`,
/// `profileCount`, `powerModes`), as `js/controls.ts` produces them for mock devices.
#[test]
fn js_generic_descriptors_deserialize() {
    let text = include_str!("fixtures/generic.json");
    let all: BTreeMap<String, Tabs> = serde_json::from_str(text).expect("fixture parses");
    let controls = |sections: &[openmouse_gpui::hid::Section]| -> Vec<(String, String)> {
        sections.iter().flat_map(|s| s.controls.iter().map(|c| (c.id.clone(), c.kind.clone()))).collect()
    };

    // Ajazz-style remapper: editable buttons, and a macro slot shown but not editable.
    let buttons = controls(&all["ajazz"].buttons);
    assert_eq!(buttons.iter().filter(|(_, k)| k == "choice").count(), 3);
    assert!(buttons.contains(&("gen.fixed/Macro slot".to_string(), "info".to_string())));
    assert!(all["ajazz"].buttons[0].controls[0].options.iter().any(|o| o.label == "Disable"));

    // Dareu-style profiles: exactly the driver's count, its names where it has them.
    let profile = &all["dareu"].advanced[0].controls[0];
    assert_eq!(profile.id, "gen.profile");
    assert_eq!(profile.options.len(), 5);
    assert_eq!(profile.options[0].label, "FPS");
    assert_eq!(profile.options[1].label, "Profile 2", "a blank name falls back to the number");

    // Pulsar reports no profile count, so it keeps its own fixed 1-6 selector.
    let pulsar = controls(&all["pulsar"].advanced);
    assert!(pulsar.iter().any(|(id, _)| id == "pulsar.profile") && !pulsar.iter().any(|(id, _)| id == "gen.profile"));

    // Regression: a Logitech mouse reports its running onboard profile as
    // `activeProfile` too, and must not be offered Pulsar's fixed 1-6 selector.
    assert!(all["logitech"].advanced.is_empty());

    // MCHOSE-style power modes live with the Performance extras.
    let power = controls(&all["mchose"].performance);
    assert_eq!(power, vec![("gen.powerMode".to_string(), "choice".to_string())]);

    // Mappings without an options list aren't remappable.
    assert!(all["noOptions"].buttons.is_empty());
}

/// Dongle / receiver controls, as `js/controls.ts` produces them for mock R5 Ultra
/// (wireless and wired), Incott and ATK statuses.
#[test]
fn js_dongle_descriptors_deserialize() {
    let text = include_str!("fixtures/dongle.json");
    let all: BTreeMap<String, Tabs> = serde_json::from_str(text).expect("fixture parses");
    let ids = |name: &str| -> Vec<String> {
        all[name].advanced.iter().flat_map(|s| s.controls.iter().map(|c| c.id.clone())).collect()
    };

    // The wireless R5 Ultra reports its dongle LED; the wired one has no dongle to control.
    assert_eq!(ids("r5Wireless"), ["dongle.led"]);
    assert_eq!(all["r5Wireless"].advanced[0].controls[0].kind, "toggle");
    assert!(ids("r5Wired").is_empty());

    assert_eq!(ids("incott"), ["dongle.incott"]);
    assert_eq!(all["incott"].advanced[0].controls[0].options.len(), 3);

    // ATK: the dongle light, plus the receiver's own state as read-only rows.
    assert_eq!(ids("atk"), ["dongle.atk", "dongle.info.state", "dongle.info.rf", "dongle.info.pairing"]);
    assert!(all["atk"].advanced[0].controls[1..].iter().all(|c| c.kind == "info"));
    // Write-only and never written: still offered, with nothing selected.
    assert_eq!(ids("atkOff"), ["dongle.atk"]);
    assert!(all["atkOff"].advanced[0].controls[0].current.is_null());
}

/// Table-driven generic settings (scroll wheel, switches, haptics, sensor modes) and
/// the read-only Device info group, as `js/controls.ts` produces them for the real
/// Superlight's fields, an MX-style mock, a "flags say unsupported" mock and a
/// Wallhack-style mock.
#[test]
fn js_generic_feature_descriptors_deserialize() {
    let text = include_str!("fixtures/features.json");
    let all: BTreeMap<String, Tabs> = serde_json::from_str(text).expect("fixture parses");
    let titles = |name: &str| -> Vec<String> { all[name].advanced.iter().map(|s| s.title.clone()).collect() };
    let find = |name: &str, id_suffix: &str| {
        all[name].advanced.iter().flat_map(|s| &s.controls).find(|c| c.id.ends_with(id_suffix)).cloned()
    };

    // The real Superlight: its Lightforce switch mode, and what it reports about itself.
    assert_eq!(titles("superlight"), ["Switches", "Device info"]);
    assert_eq!(find("superlight", "setLightforceSwitchMode").unwrap().current, "Optical");
    assert_eq!(find("superlight", "info.unit").unwrap().current, "3EC2C515");

    // MX-style: wheel and haptics are editable, Easy-Switch is read-only.
    assert_eq!(titles("mx"), ["Scroll wheel", "Haptics", "Device info"]);
    let smart = find("mx", "setSmartShiftThreshold").unwrap();
    assert_eq!((smart.kind.as_str(), smart.min, smart.max, smart.step), ("number", Some(1.0), Some(255.0), Some(5.0)));
    assert_eq!(find("mx", "info.hosts").unwrap().kind, "info");

    // A device whose capability flags say it can't invert scrolling is not offered it.
    assert!(titles("unsupported").is_empty());
    // A null field (turbo mode) is "not reported", not "off".
    assert!(find("wallhack", "setTurboMode").is_none() && find("wallhack", "setHyperMode").is_some());
}

#[test]
fn js_brand_extras_descriptors_deserialize() {
    let text = include_str!("fixtures/extras.json");
    let all: BTreeMap<String, Tabs> = serde_json::from_str(text).expect("fixture parses");
    let find = |name: &str, id: &str| all[name].advanced.iter().flat_map(|s| &s.controls).find(|c| c.id == id).cloned();

    // Starlight X: actuation is fractional millimetres, shown to two places.
    let thr = find("slx", "slx.thrL").unwrap();
    assert_eq!((thr.kind.as_str(), thr.decimals, thr.current.as_f64()), ("number", Some(2), Some(0.15)));
    // Only enabled profiles (mask 0b0111) are offered: 3 of 4.
    assert_eq!(find("slx", "slx.profile").unwrap().options.len(), 3);
    assert_eq!(find("atk", "atk.sensorMode").unwrap().options.len(), 3);
    assert_eq!(find("incott", "incott.fireTimes").unwrap().current, 2);
    assert_eq!(find("endgame", "egg.glass").unwrap().kind, "toggle");
    assert!(find("asus", "omni.pair").is_some());
    assert_eq!(find("luna", "lf.guardMs").unwrap().current.as_f64(), Some(100.0));
    // A device that doesn't support glass mode isn't offered it.
    assert!(all["plain"].advanced.is_empty());

    // Superstrike: unlinked (the buttons differ), so two cards; rapid-trigger
    // sensitivity is offered only for the button that has it on.
    assert!(find("x3", "hits.act.1").is_some() && find("x3", "hits.hap.0").is_some());
    // Rapid trigger is one slider whose lowest stop is Off.
    let rt = find("x3", "hits.rt.0").unwrap();
    assert_eq!((rt.min, rt.zero_label.as_deref(), rt.current.as_f64()), (Some(0.0), Some("Off"), Some(0.0)));
    assert_eq!(find("x3", "hits.rt.1").unwrap().current.as_f64(), Some(2.0));
}
