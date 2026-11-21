use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    Stateful, StatefulInteractiveElement as _, Styled as _, div, px,
};

use super::QueuePanel;
use super::rows::Snapshot;
use crate::panels::PanelKind;
use crate::panels::bot_chat::EXAMPLES;
use crate::store::Store;
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, Tone, glass, press};

pub(super) const FIRST_STOP: &str = "queue-first";
const ISSUES_STOP: &str = "queue-issues";

const HEADING: &str = "Start agents later, on their own";
const LEAD: &str = "Nothing is queued yet. Queue a prompt or an issue, and cornercase starts an agent on it when you \
                    say, in any of your projects, even while you are away.";

const POINTS: [(Icon, &str, &str); 5] = [
    (Icon::Queue, "When you say", "At a time, in a while, or when your Claude or Codex limits reset."),
    (
        Icon::Repeat,
        "Again and again",
        "A routine repeats on the schedule you choose. It only runs while cornercase's server runs, each run gets a \
         fresh worktree from the up-to-date default branch, and only git repositories can have routines.",
    ),
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
        .rounded(px(radius::control()))
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
        .rounded(px(radius::card()))
        .children(POINTS.into_iter().map(|(glyph, title, text)| point(glyph, title, text)))
}

fn tall(button: Stateful<Div>) -> Stateful<Div> {
    button.h(px(32.)).px(px(16.)).text_size(px(13.))
}

fn row_of_buttons() -> Div {
    div().flex().flex_wrap().justify_center().gap(px(10.)).pt(px(6.))
}

fn page_line(line: &str) -> Div {
    ui::dim(line.to_string()).max_w(px(470.)).text_center()
}

fn issues_button(stops: bool) -> Stateful<Div> {
    let button = tall(ui::icon_text_button(ISSUES_STOP, Icon::Issues, "Open Issues", Tone::Plain));
    press::stop(button, ISSUES_STOP, stops).on_click(|event: &ClickEvent, _, cx| {
        if press::pressed(event, ISSUES_STOP) {
            Store::get(cx).update(cx, |s, cx| s.open(PanelKind::Issues, cx));
        }
    })
}

pub(super) fn no_routines_line(name: &str, here: bool) -> String {
    let example = EXAMPLES[EXAMPLES.len() - 1];
    let ask = if here { "Schedule one here, or ask it" } else { "Ask it" };
    format!("{name} has no routines yet. {ask} in its chat, such as “{example}”")
}

impl QueuePanel {
    fn first_button(&self, label: &'static str, stops: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let button = tall(ui::icon_text_button(FIRST_STOP, Icon::Plus, label, Tone::Primary));
        press::stop_at(button, &self.first, FIRST_STOP, stops).on_click(cx.listener(
            |this, event: &ClickEvent, window, cx| {
                if press::pressed(event, FIRST_STOP) {
                    this.open_form(None, window, cx);
                }
            },
        ))
    }

    pub(super) fn empty_page(&self, snapshot: &Snapshot, cx: &mut Context<Self>) -> AnyElement {
        let buttons = self.form.is_none().then(|| {
            row_of_buttons()
                .child(self.first_button("Schedule an agent…", snapshot.stops, cx))
                .child(issues_button(snapshot.stops))
        });
        let page = ui::emblem_page(Icon::Queue, HEADING)
            .child(ui::emblem_line(LEAD))
            .children(buttons)
            .children(snapshot.elsewhere.as_deref().map(page_line))
            .child(div().w_full().mt(px(8.)).flex().justify_center().child(points()));
        div().id("queue-empty").flex_1().min_h_0().overflow_y_scroll().child(page).into_any_element()
    }

    pub(super) fn no_routines(&self, name: &str, snapshot: &Snapshot, cx: &mut Context<Self>) -> AnyElement {
        let gate = snapshot.bot_add_note().map(page_line);
        let line = no_routines_line(name, gate.is_none());
        let button = (gate.is_none() && self.form.is_none())
            .then(|| row_of_buttons().child(self.first_button("Schedule a routine…", snapshot.stops, cx)));
        let page = ui::emblem_page(Icon::Repeat, "No routines yet")
            .child(ui::emblem_line(line))
            .children(button)
            .children(gate);
        div().id("queue-no-routines").flex_1().min_h_0().overflow_y_scroll().child(page).into_any_element()
    }
}
