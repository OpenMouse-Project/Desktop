//! The two lines Discord Rich Presence shows, from what the user chose to share.

/// What the user has opted to show.
#[derive(Clone, Copy, Debug)]
pub struct Share {
    pub game: bool,
    pub device: bool,
    pub stats: bool,
}

/// What is true right now.
#[derive(Clone, Debug, Default)]
pub struct Facts<'a> {
    pub running_game: Option<&'a str>,
    pub device_name: Option<&'a str>,
    pub dpi: Option<f64>,
    pub polling_hz: Option<f64>,
}

/// `(details, state)`: the two lines under Discord's title, which is already the
/// app's name (from the application registration), so neither line repeats it.
///
/// Whatever is shared and known (the game, the mouse, its stats) fills the lines
/// in that order: the first becomes the top line and the rest join the second.
/// With nothing to say the top line is "Managing mouse settings" and the second
/// is empty (the caller omits an empty line; Discord rejects blank ones).
pub fn compose(share: Share, facts: &Facts) -> (String, String) {
    let mut lines: Vec<String> = Vec::new();
    if share.game {
        if let Some(game) = facts.running_game {
            lines.push(format!("Playing {game}"));
        }
    }
    if share.device {
        if let Some(name) = facts.device_name.filter(|n| !n.is_empty()) {
            lines.push(name.to_string());
        }
    }
    if share.stats {
        let mut stats = Vec::new();
        if let Some(dpi) = facts.dpi {
            stats.push(format!("{dpi:.0} DPI"));
        }
        if let Some(hz) = facts.polling_hz {
            stats.push(format!("{hz:.0} Hz"));
        }
        if !stats.is_empty() {
            lines.push(stats.join(" · "));
        }
    }
    match lines.len() {
        0 => ("Managing mouse settings".to_string(), String::new()),
        1 => (lines.remove(0), String::new()),
        _ => {
            let top = lines.remove(0);
            (top, lines.join(" · "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: Share = Share { game: true, device: true, stats: true };
    fn facts() -> Facts<'static> {
        Facts { running_game: Some("Counter-Strike 2"), device_name: Some("PRO X SUPERLIGHT 2c"), dpi: Some(800.0), polling_hz: Some(1000.0) }
    }

    #[test]
    fn shares_everything_when_asked() {
        let (d, s) = compose(ALL, &facts());
        assert_eq!(d, "Playing Counter-Strike 2");
        assert_eq!(s, "PRO X SUPERLIGHT 2c · 800 DPI · 1000 Hz");
    }

    #[test]
    fn never_repeats_the_app_name() {
        // No game running: the mouse leads and its stats follow.
        let (d, s) = compose(ALL, &Facts { running_game: None, ..facts() });
        assert_eq!((d.as_str(), s.as_str()), ("PRO X SUPERLIGHT 2c", "800 DPI · 1000 Hz"));
        for share in [ALL, Share { game: false, device: true, stats: false }, Share { game: false, device: false, stats: false }] {
            for f in [facts(), Facts::default()] {
                let (d, s) = compose(share, &f);
                assert!(!d.contains("OpenMouse Desktop") && !s.contains("OpenMouse Desktop"));
            }
        }
    }

    #[test]
    fn leaves_out_what_is_not_shared() {
        let (d, s) = compose(Share { game: false, device: true, stats: false }, &facts());
        assert_eq!((d.as_str(), s.as_str()), ("PRO X SUPERLIGHT 2c", ""));
    }

    #[test]
    fn falls_back_when_nothing_to_say() {
        let (d, s) = compose(ALL, &Facts::default());
        assert_eq!((d.as_str(), s.as_str()), ("Managing mouse settings", ""));
        let (d, s) = compose(Share { game: true, device: false, stats: false }, &Facts { running_game: None, ..facts() });
        assert_eq!((d.as_str(), s.as_str()), ("Managing mouse settings", ""));
    }
}
