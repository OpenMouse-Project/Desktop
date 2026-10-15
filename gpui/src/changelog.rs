//! What's New: the changelog the Tauri app maintains by hand in
//! `src/lib/changelog.ts`. The build (`js/build.mjs`) evaluates that file to JSON,
//! so both apps show the same entries from one source.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Entry {
    pub version: String,
    /// ISO date, `YYYY-MM-DD`.
    pub date: String,
    pub changes: Vec<String>,
}

/// Newest first, as written. Parsed once: the page asks on every frame.
pub fn entries() -> &'static [Entry] {
    static ENTRIES: std::sync::OnceLock<Vec<Entry>> = std::sync::OnceLock::new();
    ENTRIES.get_or_init(|| serde_json::from_str(include_str!("../js/dist/changelog.json")).unwrap_or_default())
}

/// An entry's lines grouped by kind, in reading order (New, Improved, Fixed, then
/// lines with no prefix). Empty groups are left out.
pub fn grouped(entry: &Entry) -> Vec<(Option<Kind>, Vec<&str>)> {
    [Some(Kind::New), Some(Kind::Improved), Some(Kind::Fixed), None]
        .into_iter()
        .filter_map(|wanted| {
            let lines: Vec<&str> = entry
                .changes
                .iter()
                .map(|line| split_kind(line))
                .filter(|(kind, _)| *kind == wanted)
                .map(|(_, text)| text)
                .collect();
            (!lines.is_empty()).then_some((wanted, lines))
        })
        .collect()
}

/// What kind of change a line is, from its `New:` / `Improved:` / `Fixed:` prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    New,
    Improved,
    Fixed,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::New => "New",
            Kind::Improved => "Improved",
            Kind::Fixed => "Fixed",
        }
    }
}

/// Splits `New: the tray…` into (`Some(New)`, `the tray…`). Lines without a
/// recognised prefix are returned whole.
pub fn split_kind(line: &str) -> (Option<Kind>, &str) {
    for (prefix, kind) in [("New:", Kind::New), ("Improved:", Kind::Improved), ("Fixed:", Kind::Fixed)] {
        if let Some(rest) = line.strip_prefix(prefix) {
            return (Some(kind), rest.trim_start());
        }
    }
    (None, line)
}

/// `2026-09-16` → `Sep 16, 2026`; anything unparseable comes back unchanged.
pub fn display_date(iso: &str) -> String {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let mut parts = iso.split('-');
    match (parts.next(), parts.next().and_then(|m| m.parse::<usize>().ok()), parts.next().and_then(|d| d.parse::<u32>().ok())) {
        (Some(year), Some(month @ 1..=12), Some(day)) => format!("{} {day}, {year}", MONTHS[month - 1]),
        _ => iso.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_shared_changelog() {
        let all = entries();
        assert!(!all.is_empty(), "the build embeds changelog.ts");
        assert!(all.iter().all(|e| !e.version.is_empty() && !e.changes.is_empty()));
    }

    #[test]
    fn splits_the_kind_prefix() {
        assert_eq!(split_kind("New: the tray menu"), (Some(Kind::New), "the tray menu"));
        assert_eq!(split_kind("Fixed: a crash"), (Some(Kind::Fixed), "a crash"));
        assert_eq!(split_kind("Plain line"), (None, "Plain line"));
    }

    #[test]
    fn groups_lines_by_kind_in_reading_order() {
        let entry = Entry {
            version: "1".into(),
            date: "2026-01-01".into(),
            changes: vec!["Fixed: a".into(), "New: b".into(), "Plain".into(), "New: c".into()],
        };
        let groups = grouped(&entry);
        let kinds: Vec<_> = groups.iter().map(|(k, _)| *k).collect();
        assert_eq!(kinds, vec![Some(Kind::New), Some(Kind::Fixed), None]);
        assert_eq!(groups[0].1, vec!["b", "c"]);
    }

    #[test]
    fn formats_dates() {
        assert_eq!(display_date("2026-09-16"), "Sep 16, 2026");
        assert_eq!(display_date("2026-01-05"), "Jan 5, 2026");
        assert_eq!(display_date("soon"), "soon");
    }
}
