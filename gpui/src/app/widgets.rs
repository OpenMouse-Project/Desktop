//! Small elements shared between pages.

use super::*;

/// A small status badge: a faint accent-tinted pill with an accent dot and the
/// theme's own text colour. (The toolkit's `Tag::success()` is a fixed green
/// that ignores the palette and reads poorly on the Dynamic theme; coloured
/// text on a tint of itself falls under 4.5:1 on some wallpapers.)
pub(super) fn accent_badge(text: impl Into<SharedString>, accent: Hsla, ink: Hsla) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_1p5()
        .px_2()
        .py_0p5()
        .rounded_full()
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .bg(Hsla { a: 0.20, ..accent })
        .text_color(ink)
        .child(div().size(px(6.)).flex_none().rounded_full().bg(accent))
        .child(text.into())
}

impl AppView {
    /// Mouse artwork in a `side` x `side` box. Placement is in fractions of
    /// the box (the Tauri app's `deviceArtStyle` normalisation, so every
    /// model's subject comes out a comparable size); `zoom` scales that up
    /// where the art should dominate.
    pub(super) fn device_art(&self, tile: &Tile, side: f32, zoom: f32) -> Div {
        let frame = div().relative().flex_none().w(px(side)).h(px(side)).overflow_hidden();
        let Some(art) = &tile.art else { return frame };
        let image = img(art.path.clone());
        match &art.placement {
            Some(p) => {
                let h = p.height / 100.0 * zoom;
                let w = h * p.aspect;
                let left = 0.5 - w / 2.0 + p.offset_x / 100.0 * w;
                let top = 0.5 - h / 2.0 + p.offset_y / 100.0 * h;
                frame.child(
                    image
                        .absolute()
                        .left(relative(left))
                        .top(relative(top))
                        .w(relative(w))
                        .h(relative(h))
                        .object_fit(ObjectFit::Fill),
                )
            }
            None => frame.child(image.size_full().object_fit(ObjectFit::Contain)),
        }
    }

    /// Selectable pill used for the page's option lists.
    pub(super) fn chip(
        &self,
        id: String,
        label: String,
        selected: bool,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        let primary = cx.theme().primary;
        div()
            .id(SharedString::from(id))
            .cursor_pointer()
            .px_3()
            .py_1p5()
            .rounded_md()
            .border_1()
            .text_sm()
            .border_color(if selected { primary } else { cx.theme().border })
            .when(selected, |this| this.bg(Hsla { a: 0.14, ..primary }))
            .text_color(if selected { cx.theme().foreground } else { cx.theme().muted_foreground })
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| on_click(this, cx)))
    }

    /// A slider for a Buttons/Advanced/Performance control: accent-filled track
    /// and thumb, with its value in a fixed-width pill so rows line up.
    pub(super) fn control_slider_row(slider: Option<Entity<SliderState>>, value: String, cx: &mut Context<Self>) -> AnyElement {
        let accent = cx.theme().primary;
        let fg = cx.theme().foreground;
        let border = cx.theme().border;
        h_flex()
            .gap_3()
            .items_center()
            .children(slider.map(|s| div().w(px(220.)).child(Slider::new(&s).bg(accent).text_color(fg))))
            .child(
                div()
                    .min_w(px(84.))
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(Hsla { a: 0.7, ..border })
                    .bg(Hsla { a: 0.05, ..fg })
                    .text_sm()
                    .text_center()
                    .text_color(fg)
                    .child(value),
            )
            .into_any_element()
    }
}
