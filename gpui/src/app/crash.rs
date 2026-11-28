//! Telling the user about a crash on the next launch (see `crate::logging`).

use super::*;

impl AppView {
    /// A card offering the last crash report. Showing it only opens the folder;
    /// sharing it is up to the user.
    pub(super) fn render_crash_notice(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.crash_report.as_ref()?;
        Some(
            div()
                .absolute()
                .bottom(px(80.))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(
                    h_flex()
                        .occlude()
                        .max_w(px(720.))
                        .mx_4()
                        .gap_3()
                        .items_center()
                        .px_4()
                        .py_3()
                        .rounded_lg()
                        .border_1()
                        .border_color(cx.theme().danger)
                        .bg(cx.theme().sidebar)
                        .shadow_lg()
                        // `min_w_0` lets the message wrap; without it the text
                        // keeps its full width and pushes the buttons out.
                        .child(div().flex_1().min_w_0().text_sm().text_color(cx.theme().foreground).child(
                            "OpenMouse crashed. A report was saved on this computer; nothing was sent.",
                        ))
                        .child(Button::new("crash-show").compact().label("Show report").on_click(cx.listener(
                            |this, _, _, cx| {
                                if let Some(report) = this.crash_report.take() {
                                    cx.reveal_path(&crate::logging::mark_seen(&report));
                                }
                                cx.notify();
                            },
                        )))
                        .child(Button::new("crash-dismiss").compact().label("Dismiss").on_click(cx.listener(
                            |this, _, _, cx| {
                                if let Some(report) = this.crash_report.take() {
                                    crate::logging::mark_seen(&report);
                                }
                                cx.notify();
                            },
                        ))),
                )
                .into_any_element(),
        )
    }
}
