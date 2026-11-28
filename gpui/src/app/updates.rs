//! The update check and the Updates page.

use super::*;

/// Rows above the releases on the Updates page: heading, version card, notes title.
pub(super) const UPDATES_LEADING_ROWS: usize = 3;

/// Colours a list row needs, copied into the list's `'static` render closure.
#[derive(Clone, Copy)]
pub(super) struct PageColors {
    pub(super) fg: Hsla,
    pub(super) muted: Hsla,
    pub(super) accent: Hsla,
    pub(super) border: Hsla,
}

/// One release on the Updates page, as a card; the running version is tinted.
/// The list is virtualized, so only the rows in view are ever laid out.
pub(super) fn release_row(entry: &crate::changelog::Entry, current: bool, c: PageColors) -> AnyElement {
    use crate::changelog::{display_date, grouped, Kind};
    let mut changes = div().flex().flex_col().gap_4().flex_1().min_w_0();
    for (kind, lines) in grouped(entry) {
        let (label, tone) = match kind {
            Some(Kind::New) => ("New", c.accent),
            Some(Kind::Improved) => ("Improved", c.accent),
            Some(Kind::Fixed) => ("Fixed", c.muted),
            None => ("Changes", c.muted),
        };
        // The badge's dot and fill take the tone; its text stays readable.
        let mut group = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().flex().child(accent_badge(label, tone, c.fg)));
        for line in lines {
            group = group.child(
                h_flex()
                    .gap_3()
                    .items_start()
                    .child(div().mt(px(7.)).size(px(5.)).flex_none().rounded_full().bg(Hsla { a: 0.7, ..tone }))
                    .child(div().flex_1().min_w_0().text_sm().line_height(relative(1.5)).text_color(c.fg).child(line.to_string())),
            );
        }
        changes = changes.child(group);
    }
    h_flex()
        .gap_8()
        .items_start()
        .p_5()
        .rounded_xl()
        .border_1()
        .border_color(if current { Hsla { a: 0.5, ..c.accent } } else { c.border })
        .bg(if current { Hsla { a: 0.05, ..c.accent } } else { Hsla { a: 0.03, ..c.fg } })
        .child(
            div()
                .w(px(132.))
                .flex_none()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(if current { c.accent } else { c.fg })
                        .child(format!("v{}", entry.version)),
                )
                .child(div().text_xs().text_color(c.muted).child(display_date(&entry.date)))
                .children(current.then(|| div().text_xs().font_weight(FontWeight::MEDIUM).text_color(c.accent).child("Current"))),
        )
        .child(changes)
        .into_any_element()
}

impl AppView {
    /// Update watcher callback: a `latest.json` fetch landed, from either the
    /// background poll or the manual "Check for updates" button. Stores the
    /// result and, if it's newer than `APP_VERSION` and this version hasn't
    /// already been nagged about, toasts once (mirrors
    /// `runUpdateCheck`/`notifiedVersion` in `src/lib/update-check.ts`).
    pub fn note_update_available(&mut self, info: crate::update_check::UpdateInfo, cx: &mut Context<Self>) {
        let is_new = crate::update_check::is_newer(APP_VERSION, &info.version);
        let should_toast = is_new
            && self.settings.last_notified_update_version.as_deref() != Some(info.version.as_str());
        self.update_available = is_new.then(|| info.clone());
        if should_toast {
            self.settings.last_notified_update_version = Some(info.version.clone());
            self.settings.save();
            self.show_toast(format!("OpenMouse Desktop v{} is available — see Updates.", info.version), false, cx);
        }
        cx.notify();
    }

    /// "Check for updates" button: runs a check right now (not waiting for
    /// the next background-poll tick) and always surfaces *something* —
    /// "up to date" included — since the user explicitly asked.
    pub(super) fn check_for_updates_now(&mut self, cx: &mut Context<Self>) {
        if self.checking_for_updates {
            return;
        }
        self.checking_for_updates = true;
        cx.notify();
        cx.spawn(async move |view, cx| {
            let info = cx.background_executor().spawn(async { crate::update_check::fetch_latest() }).await;
            let _ = view.update(cx, |view, cx| {
                view.checking_for_updates = false;
                match info {
                    // A manual click always wants an answer, even "nothing to
                    // report" — `note_update_available` itself stays silent
                    // once a version's already been nagged about.
                    Some(info) if !crate::update_check::is_newer(APP_VERSION, &info.version) => {
                        view.note_update_available(info, cx);
                        view.show_toast("You're up to date.", false, cx);
                    }
                    Some(info) => view.note_update_available(info, cx),
                    None => view.show_toast("Couldn't check for updates — check your connection.", true, cx),
                }
            });
        })
        .detach();
    }

    /// Custom titlebar content mirroring `TitleBar.tsx`: app name plus a
    /// version·stage badge. Dragging, double-click zoom, and the
    /// min/max/close buttons stay owned by Kit's `TitleBar`; this only
    /// supplies the branded row inside it.
    /// Updates: this version, and the release notes as a timeline (version and
    /// date on the left, changes grouped under New / Improved / Fixed on the
    /// right). One virtualized list, so only the rows on screen are built — the
    /// page used to rebuild every release on each scroll tick and pinned a core.
    pub(super) fn render_updates(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = PageColors {
            fg: cx.theme().foreground,
            muted: cx.theme().muted_foreground,
            accent: cx.theme().primary,
            border: Hsla { a: 0.7, ..cx.theme().border },
        };
        let checking = self.checking_for_updates;
        let on_click = cx.listener(|this, _, _, cx| this.check_for_updates_now(cx));
        let check_button = Button::new("check-updates")
            .compact()
            .label("Check for updates")
            .loading(checking)
            .disabled(checking)
            .on_click(on_click)
            .into_any_element();
        // The click handler lives on the card's button, built here once per render
        // of the page (cheap); the list rows are built lazily below.
        let mut check_button = Some(check_button);
        let mut update_banner = self.update_available.as_ref().map(|info| {
            let notes = info.notes.clone().unwrap_or_default();
            h_flex()
                .gap_3()
                .items_center()
                .justify_between()
                .p_4()
                .mt_3()
                .rounded_lg()
                .border_1()
                .border_color(colors.accent)
                .bg(Hsla { a: 0.08, ..colors.accent })
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(colors.fg)
                                .child(format!("v{} is available", info.version)),
                        )
                        .children((!notes.is_empty()).then(|| div().text_xs().text_color(colors.muted).child(notes))),
                )
                .child(
                    Button::new("view-release")
                        .compact()
                        .label("View release")
                        .on_click(cx.listener(|_, _, _, _| {
                            // Opens the GitHub release page for a manual download —
                            // this pass is check + notify only, no in-app download/install.
                            let _ = crate::proc::quiet(
                                std::process::Command::new("cmd")
                                    .args(["/C", "start", ""])
                                    .arg("https://github.com/OpenMouse-Project/Desktop/releases/latest")
                                    .stdin(std::process::Stdio::null())
                                    .stdout(std::process::Stdio::null())
                                    .stderr(std::process::Stdio::null()),
                            )
                            .spawn();
                        }))
                        .into_any_element(),
                )
                .into_any_element()
        });

        let entries = crate::changelog::entries();
        let list_state = self.updates_list.clone();
        let stage = release_stage(APP_VERSION);

        list(list_state, move |ix, _window, _cx| {
            let c = colors;
            let body: AnyElement = match ix {
                0 => div()
                    .flex()
                    .flex_col()
                    .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).text_color(c.fg).child("Updates"))
                    .child(div().text_sm().text_color(c.muted).child("Your version and what's changed"))
                    .children(update_banner.take())
                    .into_any_element(),
                1 => h_flex()
                    .gap_4()
                    .items_center()
                    .justify_between()
                    .p_6()
                    .mt_5()
                    .rounded_xl()
                    .border_1()
                    .border_color(Hsla { a: 0.35, ..c.accent })
                    .bg(Hsla { a: 0.06, ..c.accent })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1p5()
                            .child(div().text_xs().text_color(c.muted).child("OpenMouse Desktop"))
                            .child(
                                h_flex()
                                    .gap_3()
                                    .items_center()
                                    .child(div().text_2xl().font_weight(FontWeight::BOLD).text_color(c.fg).child(format!("v{APP_VERSION}")))
                                    .child(accent_badge(stage.to_string(), c.accent, c.fg)),
                            )
                            .child(div().text_sm().text_color(c.muted).child("You're running this version.")),
                    )
                    .children(check_button.take())
                    .into_any_element(),
                2 => div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .pt_8()
                    .pb_1()
                    .child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(c.fg).child("Release notes"))
                    .children(entries.is_empty().then(|| div().text_sm().text_color(c.muted).child("No release notes yet.")))
                    .into_any_element(),
                n => match entries.get(n - UPDATES_LEADING_ROWS) {
                    Some(entry) => release_row(entry, entry.version == APP_VERSION, c),
                    None => div().into_any_element(),
                },
            };
            div().w_full().px_4().pt_3().child(div().max_w(px(960.)).child(body)).into_any_element()
        })
        .size_full()
        .into_any_element()
    }
}
