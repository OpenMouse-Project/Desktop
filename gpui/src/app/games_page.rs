//! The Games page.

use super::*;

impl AppView {
    /// Games page: a header with live counts, then the installed/running
    /// games and the rest of the catalog as grids of poster cards. Cached art
    /// falls back to a name-initial tile until prefetch completes.
    pub(super) fn render_games(&self, cx: &mut Context<Self>) -> AnyElement {
        let (installed, rest): (Vec<&Game>, Vec<&Game>) =
            self.games.iter().partition(|g| g.installed || g.running);
        let running = self.games.iter().filter(|g| g.running).count();
        let profiles = self.games.iter().filter(|g| self.profiles.has_any(&g.id)).count();
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let subtitle = {
            let mut parts = vec![format!("{} installed", installed.len())];
            if running > 0 {
                parts.push(format!("{running} running"));
            }
            parts.push(format!("{profiles} with a profile"));
            parts.join(" · ")
        };
        div()
            .flex()
            .flex_col()
            .size_full()
            .px_6()
            .py_5()
            .gap_6()
            .overflow_y_scrollbar()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).text_color(fg).child("Games"))
                    .child(div().text_sm().text_color(muted).child(subtitle)),
            )
            .child(self.render_game_section(cx, "Installed & running", "Pick a game to give it its own mouse profile.", &installed, false))
            .child(self.render_game_section(cx, "More games", "Not found on this PC. Profiles you set up still apply once they are installed.", &rest, true))
            .into_any_element()
    }

    pub(super) fn render_game_section(
        &self,
        cx: &mut Context<Self>,
        title: &str,
        blurb: &str,
        games: &[&Game],
        muted: bool,
    ) -> AnyElement {
        if games.is_empty() {
            return div().into_any_element();
        }
        let fg = cx.theme().foreground;
        let subtle = cx.theme().muted_foreground;
        let border = cx.theme().border;
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_baseline()
                            .gap_2()
                            .child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(fg).child(title.to_string()))
                            .child(
                                div()
                                    .text_xs()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_full()
                                    .bg(Hsla { a: 0.08, ..fg })
                                    .text_color(subtle)
                                    .child(games.len().to_string()),
                            ),
                    )
                    .child(div().text_sm().text_color(subtle).child(blurb.to_string())),
            )
            .child(div().h(px(1.)).w_full().bg(Hsla { a: 0.7, ..border }))
            .child(
                div().flex().flex_row().flex_wrap().gap_4().children(
                    games
                        .iter()
                        .map(|game| self.render_game_card(cx, game, muted)),
                ),
            )
            .into_any_element()
    }

    /// One poster card: art on top, then the name and status badges. A running
    /// game gets an accent outline; games not installed are dimmed.
    pub(super) fn render_game_card(&self, cx: &mut Context<Self>, game: &Game, muted: bool) -> AnyElement {
        const CARD_W: f32 = 148.;
        const ART_H: f32 = 200.;
        let fg = cx.theme().foreground;
        let accent = cx.theme().primary;
        let border = cx.theme().border;
        // Cached file art when prefetched, else the name-initial tile.
        // Remote `img(url)` is not used: desktop GPUI ships no HTTP client
        // (`with_http_client` is web-backend-only), so URLs would stay blank.
        // `card_art_path` prefers the 400 px copy prefetch caches: GPUI never
        // evicts decoded images, so full-size covers cost ~58 MB for the page.
        let art = match crate::games::card_art_path(game) {
            Some(path) => div()
                .w_full()
                .h(px(ART_H))
                .overflow_hidden()
                .child(
                    // PathBuf (not String): String routes to the embedded
                    // asset bundle and misses; PathBuf reads the file.
                    img(path).w_full().h_full().object_fit(ObjectFit::Cover),
                )
                .into_any_element(),
            None => div()
                .w_full()
                .h(px(ART_H))
                .flex()
                .items_center()
                .justify_center()
                .bg(Hsla { a: 0.10, ..accent })
                .text_size(px(40.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(Hsla { a: 0.7, ..accent })
                .child(game.mark().to_string())
                .into_any_element(),
        };
        let game_id = game.id.clone();
        let has_profile = self.profiles.has_any(&game.id);
        div()
            .id(SharedString::from(format!("game-{}", game.id)))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.profile_editor = Some(game_id.clone());
                this.profile_device = None;
                cx.notify();
            }))
            .flex()
            .flex_col()
            .w(px(CARD_W))
            .flex_shrink_0()
            .rounded_xl()
            .overflow_hidden()
            .border_1()
            .border_color(if game.running { Hsla { a: 0.8, ..accent } } else { Hsla { a: 0.7, ..border } })
            .bg(Hsla { a: 0.04, ..fg })
            .hover(move |card| card.bg(Hsla { a: 0.08, ..fg }).border_color(Hsla { a: 0.55, ..accent }))
            .opacity(if muted && !game.running { 0.6 } else { 1.0 })
            .child(art)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .p_2p5()
                    .child(
                        div()
                            .w_full()
                            .min_w_0()
                            .h(px(32.))
                            .overflow_hidden()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(fg)
                            .child(game.name.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap_1()
                            .h(px(20.))
                            .when(game.running, |row| row.child(accent_badge("Running", accent, fg)))
                            .when(game.installed && !game.running, |row| {
                                row.child(Tag::secondary().small().child("Installed"))
                            })
                            .when(has_profile, |row| row.child(accent_badge("Profile", accent, fg))),
                    ),
            )
            .into_any_element()
    }
}
