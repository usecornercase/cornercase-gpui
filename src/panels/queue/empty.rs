use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px,
};

use super::QueuePanel;
use crate::panels::PanelKind;
use crate::store::Store;
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, Tone, glass};

const HEADING: &str = "Start agents later, on their own";
const LEAD: &str = "Nothing is queued yet. Queue a prompt or an issue, and cornercase starts an agent on it when you \
                    say, in any of your projects, even while you are away.";

const POINTS: [(Icon, &str, &str); 4] = [
    (Icon::Queue, "When you say", "At a time, in a while, or when your Claude or Codex limits reset."),
    (
        Icon::Message,
        "From a prompt or an issue",
        "Write what the agent should do, paste an issue's URL or key, or press Schedule… on an issue in Issues.",
    ),
    (
        Icon::Branch,
        "Where you choose",
        "A new tab in one of your workspaces, or a new worktree on a branch of its own.",
    ),
    (Icon::Agent, "Unattended", "It accepts the folder's trust prompt and submits the prompt by itself."),
];

fn point(glyph: Icon, title: &'static str, text: &'static str) -> Div {
    let square = div()
        .size(px(30.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::CONTROL))
        .bg(glass::tint(colours::brand(), 0.14))
        .child(ui::icon(glyph, 15.).text_color(colours::accent()));
    div().flex().items_start().gap(px(12.)).child(square).child(
        div()
            .flex_1()
            .min_w(px(0.))
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(title))
            .child(div().text_size(px(12.)).text_color(colours::muted()).child(text)),
    )
}

fn points() -> Div {
    div()
        .flex_1()
        .min_w(px(0.))
        .max_w(px(520.))
        .p(px(16.))
        .flex()
        .flex_col()
        .gap(px(14.))
        .bg(colours::panel())
        .border_1()
        .border_color(colours::border())
        .rounded(px(radius::CARD))
        .children(POINTS.into_iter().map(|(glyph, title, text)| point(glyph, title, text)))
}

fn buttons(cx: &mut Context<QueuePanel>) -> Div {
    let tall = |button: gpui::Stateful<Div>| button.h(px(32.)).px(px(16.)).text_size(px(13.));
    div()
        .flex()
        .flex_wrap()
        .justify_center()
        .gap(px(10.))
        .pt(px(6.))
        .child(
            tall(ui::icon_text_button("queue-first", Icon::Plus, "Schedule an agent…", Tone::Primary))
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_form(None, window, cx))),
        )
        .child(
            tall(ui::icon_text_button("queue-issues", Icon::Issues, "Open Issues", Tone::Plain))
                .on_click(|_: &ClickEvent, _, cx| Store::get(cx).update(cx, |s, cx| s.open(PanelKind::Issues, cx))),
        )
}

impl QueuePanel {
    pub(super) fn empty_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let page = div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(10.))
            .px(px(28.))
            .pt(px(40.))
            .pb(px(28.))
            .child(ui::emblem(Icon::Queue))
            .child(div().text_size(px(17.)).font_weight(FontWeight::SEMIBOLD).text_center().child(HEADING))
            .child(div().max_w(px(470.)).text_center().text_color(colours::muted()).child(LEAD))
            .when(self.form.is_none(), |d| d.child(buttons(cx)))
            .child(div().w_full().mt(px(8.)).flex().justify_center().child(points()));
        div().id("queue-empty").flex_1().min_h_0().overflow_y_scroll().child(page).into_any_element()
    }
}
