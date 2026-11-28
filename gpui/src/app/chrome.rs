//! The window frame: sidebar, title bar and toast.

use super::*;

/// Release stage label shown in the titlebar badge: every 0.x build is a
/// Beta, 1.0 and later are Public.
pub(super) fn release_stage(version: &str) -> &'static str {
    let major = version
        .trim_start_matches('v')
        .split('.')
        .next()
        .unwrap_or("0")
        .parse::<u64>()
        .unwrap_or(0);
    if major == 0 {
        "Beta"
    } else {
        "Public"
    }
}

/// The community server, the same invite openmouse.app links to.
pub(super) const DISCORD_INVITE_URL: &str = "https://discord.gg/yxC9jzMdw6";

/// The Discord glyph (drawn in the theme colour by `themed_discord`), embedded so
/// there is no file to ship or find at runtime.
pub(super) static DISCORD_SVG: &str = include_str!("../../assets/discord.svg");

/// The Discord glyph drawn in `color`, so it follows the theme like the other
/// sidebar icons instead of being a fixed brand blue. One image per colour.
pub(super) fn themed_discord(color: Hsla) -> std::sync::Arc<Image> {
    static CACHE: std::sync::Mutex<Vec<(u32, std::sync::Arc<Image>)>> = std::sync::Mutex::new(Vec::new());
    let rgba: Rgba = color.into();
    let hex = ((rgba.r * 255.0).round() as u32) << 16 | ((rgba.g * 255.0).round() as u32) << 8 | (rgba.b * 255.0).round() as u32;
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, image)) = cache.iter().find(|(h, _)| *h == hex) {
        return image.clone();
    }
    let svg = DISCORD_SVG.replace("#5865F2", &format!("#{hex:06X}"));
    let image = std::sync::Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes()));
    cache.push((hex, image.clone()));
    image
}

impl AppView {
    /// One nav row: icon and label, a tinted fill and an accent bar when active.
    pub(super) fn nav_row(&self, id: &'static str, icon: IconName, label: &str, active: bool, collapsed: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let accent = cx.theme().primary;
        div()
            .id(id)
            .relative()
            .cursor_pointer()
            .flex()
            .flex_row()
            .items_center()
            .when(collapsed, |row| row.justify_center())
            .gap_3()
            .h(px(40.))
            .px_3()
            .rounded_lg()
            .text_sm()
            .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
            .text_color(if active { fg } else { muted })
            .when(active, |row| row.bg(Hsla { a: 0.16, ..accent }))
            .when(!active, |row| row.hover(|h| h.bg(Hsla { a: 0.07, ..fg }).text_color(fg)))
            .when(active, |row| {
                row.child(
                    div()
                        .absolute()
                        .left(px(-6.))
                        .top(px(10.))
                        .w(px(3.))
                        .h(px(20.))
                        .rounded_full()
                        .bg(accent),
                )
            })
            .child(Icon::new(icon).size(px(18.)).text_color(if active { accent } else { muted }))
            .when(!collapsed, |row| row.child(label.to_string()))
    }

    /// A footer row (Help & Feedback, Settings): an icon, a title with an optional
    /// subtitle, and a chevron.
    pub(super) fn footer_row(
        &self,
        id: &'static str,
        glyph: AnyElement,
        title: &str,
        subtitle: Option<&str>,
        active: bool,
        collapsed: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        div()
            .id(id)
            .cursor_pointer()
            .flex()
            .flex_row()
            .items_center()
            .when(collapsed, |row| row.justify_center())
            .gap_3()
            .px_3()
            .py_2p5()
            .when(active, |row| row.bg(Hsla { a: 0.14, ..cx.theme().primary }))
            .when(!active, |row| row.hover(|h| h.bg(Hsla { a: 0.06, ..fg })))
            .child(div().w(px(22.)).flex_none().flex().items_center().justify_center().child(glyph))
            .when(!collapsed, |row| {
                row.child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(if active { fg } else { muted }).child(title.to_string()))
                        .children(subtitle.map(|s| div().text_xs().text_color(muted).truncate().child(s.to_string()))),
                )
                .child(Icon::new(IconName::ChevronRight).size(px(14.)).text_color(muted))
            })
    }

    pub(super) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let collapsed = self.sidebar_collapsed;
        let fg = cx.theme().foreground;
        let border = cx.theme().border;
        let accent = cx.theme().primary;

        let mut nav = div().flex().flex_col().gap_1();
        for (id, page) in [("nav-overview", Page::Overview), ("nav-games", Page::Games), ("nav-supported", Page::Supported)] {
            nav = nav.child(self.nav_row(id, page.icon(), page.label(), self.page == page, collapsed, cx).on_click(
                cx.listener(move |this, _, _, cx| {
                    // Choosing the page you are on goes back to its top level.
                    match page {
                        Page::Overview => this.devices.open = None,
                        Page::Games => this.profile_editor = None,
                        _ => {}
                    }
                    this.page = page;
                    cx.notify();
                }),
            ));
        }

        let discord = img(themed_discord(cx.theme().muted_foreground)).w(px(22.)).h(px(17.)).into_any_element();
        let on_updates = self.page == Page::Updates;
        let on_settings = self.page == Page::Settings;
        let tone = |active: bool| if active { accent } else { cx.theme().muted_foreground };
        let updates_icon = Icon::new(Page::Updates.icon()).size(px(18.)).text_color(tone(on_updates)).into_any_element();
        let gear = Icon::new(Page::Settings.icon()).size(px(18.)).text_color(tone(on_settings)).into_any_element();
        let footer = div()
            .flex()
            .flex_col()
            .rounded_xl()
            .border_1()
            .border_color(Hsla { a: 0.8, ..border })
            .bg(Hsla { a: 0.04, ..fg })
            .overflow_hidden()
            .child(
                self.footer_row("footer-updates", updates_icon, "Updates", Some(&format!("v{APP_VERSION} · Release notes")), on_updates, collapsed, cx)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.page = Page::Updates;
                        cx.notify();
                    })),
            )
            .child(div().h(px(1.)).w_full().bg(Hsla { a: 0.8, ..border }))
            .child(
                self.footer_row("footer-discord", discord, "Help & Feedback", Some("Join the community server"), false, collapsed, cx)
                    .on_click(cx.listener(|_, _, _, cx| cx.open_url(DISCORD_INVITE_URL))),
            )
            .child(div().h(px(1.)).w_full().bg(Hsla { a: 0.8, ..border }))
            .child(
                self.footer_row("footer-settings", gear, Page::Settings.label(), None, self.page == Page::Settings, collapsed, cx)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.page = Page::Settings;
                        cx.notify();
                    })),
            );
        let _ = accent;

        div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(if collapsed { SIDEBAR_COLLAPSED_W } else { SIDEBAR_W }))
            .h_full()
            .px_3()
            .pt_3()
            .pb_3()
            .gap_6()
            .child(nav)
            .child(div().flex_1())
            .child(footer)
    }

    pub(super) fn render_titlebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Badge fade mirrors the theme-transition feel: 380ms in `App.css`
        // is the full-theme swap; a hover reveal reads better faster.
        const BADGE_FADE: Duration = Duration::from_millis(160);
        let hovered = self.badge_hovered;
        let collapsed = self.sidebar_collapsed;
        // The sidebar toggle lives here, just after the window buttons.
        let toggle = Button::new("toggle-sidebar")
            .ghost()
            .compact()
            .icon(IconName::Menu)
            .accessibility_label(if collapsed { "Expand sidebar" } else { "Collapse sidebar" })
            .tooltip(if collapsed { "Expand sidebar" } else { "Collapse sidebar" })
            .on_click(cx.listener(|this, _, _, cx| {
                this.sidebar_collapsed = !this.sidebar_collapsed;
                cx.notify();
            }));
        let brand = h_flex()
            .id("title-brand")
            .items_center()
            .gap_2()
            .px_1()
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.badge_hovered = *hovered;
                cx.notify();
            }))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().foreground)
                    .child("OpenMouse Desktop"),
            )
            .child(
                div()
                    .text_xs()
                    .px_1p5()
                    .py_0p5()
                    .rounded_md()
                    .bg(cx.theme().muted)
                    .text_color(cx.theme().muted_foreground)
                    .when(!hovered, |this| this.invisible())
                    .with_animation(
                        ElementId::NamedInteger("title-badge".into(), hovered as u64),
                        Animation::new(BADGE_FADE)
                            .with_easing(gpui_kit::base::animation::ease_out_cubic),
                        move |this, delta| {
                            // Fade + slight rise on enter, reversed on exit
                            // via the keyed id retargeting the animation.
                            let progress = if hovered { delta } else { 1.0 - delta };
                            this.opacity(progress).top(px(-4.0 * (1.0 - progress)))
                        },
                    )
                    .child(format!("v{APP_VERSION} · {}", release_stage(APP_VERSION))),
            );
        // The kit's bar is a Windows caption (drag) region, which turns clicks
        // into window moves; occluding it makes the toggle a normal client hit.
        // Collapsed, the sidebar strip is narrower than the name, so the text
        // would straddle its edge; only the toggle stays over the strip.
        h_flex().items_center().gap_1().child(
            // Collapsed, the bar's 12px left inset is mirrored on the right so
            // the button centres over the icon column below it.
            div()
                .occlude()
                .when(collapsed, |slot| slot.w(px(SIDEBAR_COLLAPSED_W - 24.)).flex().justify_center())
                .child(toggle),
        ).when(!collapsed, |row| row.child(brand))
    }

    /// The bottom-of-window message, if one is showing.
    pub(super) fn render_toast(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let toast = self.toast.as_ref()?;
        let accent = if toast.error { cx.theme().danger } else { cx.theme().primary };
        Some(
            div()
                .absolute()
                .bottom(px(24.))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(
                    div()
                        .max_w(px(560.))
                        .px_4()
                        .py_3()
                        .rounded_lg()
                        .border_1()
                        .border_color(accent)
                        .bg(cx.theme().sidebar)
                        .shadow_lg()
                        .text_sm()
                        .text_color(cx.theme().foreground)
                        .child(toast.text.clone()),
                )
                .into_any_element(),
        )
    }
}
