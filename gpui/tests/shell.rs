//! Shell navigation contract, harness-free.
//!
//! `gpui-kit/test-support` is not resolvable in this lockfile (Kit 0.7.3's
//! `test-support` pulls `gpui-fast 0.1.7`, whose `cocoa =0.26.0` conflicts
//! with the locked `cocoa 0.26.1` from `gpui-pre-macos 0.3.9`). Until that
//! upstream conflict clears, this suite covers the pure navigation contract:
//! every page has a distinct label, and the default page is Overview.

use openmouse_gpui::app::{AppView, Page};

#[test]
fn pages_have_distinct_labels() {
    let labels = [
        Page::Overview.label(),
        Page::Games.label(),
        Page::Supported.label(),
        Page::Settings.label(),
    ];
    let mut sorted = labels.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 4, "page labels must be distinct");
}

#[test]
fn default_page_is_overview() {
    assert_eq!(AppView::new().page(), Page::Overview);
}
