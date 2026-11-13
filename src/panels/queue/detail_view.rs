use std::time::Instant;

use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, div, list, prelude::FluentBuilder as _, px, relative,
};

use super::QueuePanel;
use super::data::{Entry, status_label};
use super::detail::{Detail, View};
use super::form_view::frame;
use super::read::refreshing_keys;
use super::result::{
    Answer, MESSAGE_CUT, NO_MESSAGE, Page, Spoken, Talk, can_copy, facts, notes as notes_of, said_heading,
};
use super::rows::{Shown, Snapshot, state_colour, state_icon};
use super::{open_pane, time};
use crate::dialogs::pull_tag;
use crate::model::PullRequest;
use crate::model::link::is_web;
use crate::notify::tip;
use crate::store::Store;
use crate::theme::{colours, radius};
use crate::ui::markdown::text::{self, OnLink, opens_web_addresses};
use crate::ui::{self, Icon, Tone};

const FACT_LABEL: f32 = 96.;
const PR_TIP: &str = "As it was when this result was kept";

fn fact_row(label: &'static str, value: impl IntoElement) -> Div {
    div()
        .flex()
        .gap(px(8.))
        .text_size(px(12.5))
        .child(div().w(px(FACT_LABEL)).flex_none().text_color(colours::muted()).child(label))
        .child(div().flex_1().min_w(px(0.)).text_color(colours::text()).child(value))
}

fn pull_row(pr: &PullRequest) -> Div {
    let (tag, colour, crossed) = pull_tag(pr);
    let tag = div().flex_none().text_color(colour).when(crossed, gpui::Styled::line_through).child(tag);
    let line = div().flex().gap(px(6.)).child(tag).child(div().flex_1().min_w(px(0.)).child(pr.title.clone()));
    if !is_web(&pr.url) {
        return fact_row("Pull request", line);
    }
    let url = pr.url.clone();
    let open = line
        .id("queue-detail-pr")
        .cursor_pointer()
        .tooltip(tip(format!("Open it on the web: {}. {PR_TIP}.", pr.url)))
        .on_click(move |_: &ClickEvent, _, cx| cx.open_url(&url));
    fact_row("Pull request", open)
}

fn fact_rows(facts: Vec<super::result::Fact>) -> Div {
    let rows = facts.into_iter().map(|fact| match fact.pull {
        Some(pr) => pull_row(&pr),
        None => fact_row(fact.label, fact.value),
    });
    div().flex().flex_col().gap(px(5.)).children(rows)
}

fn notes(detail: &Detail) -> Div {
    let lines = notes_of(detail.answer.as_ref(), detail.page);
    let pad = if lines.is_empty() { 0. } else { 6. };
    div()
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(3.))
        .px(px(20.))
        .pb(px(pad))
        .text_size(px(12.))
        .text_color(colours::muted())
        .children(lines)
}

fn title(label: &str) -> impl IntoElement + use<> {
    let text = ui::clamped(3, 20.).text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).child(label.to_string());
    div().id("queue-detail-title").flex_1().min_w(px(0.)).tooltip(tip(label.to_string())).child(text)
}

fn status_line(entry: &Entry, shown: &Shown, snapshot: &Snapshot, refreshing: bool) -> Div {
    let colour = state_colour(shown.state);
    let meta = format!("· {}", entry.meta(snapshot.session.as_ref()));
    div()
        .flex()
        .flex_wrap()
        .gap(px(6.))
        .text_size(px(12.))
        .child(state_icon(entry.id, shown.state))
        .child(div().text_color(colour).child(status_label(entry, shown.pane)))
        .child(div().flex_1().min_w(px(0.)).text_color(colours::faint()).child(meta))
        .when(refreshing, |d| d.child(ui::busy("queue-detail-busy", "Reading…").text_size(px(11.))))
}

impl QueuePanel {
    fn actions_row(shown: &Shown, can_copy: bool, cx: &mut Context<Self>) -> Div {
        let open = shown.pane.map(|pane| {
            ui::button("queue-detail-open", "Open")
                .tooltip(tip(format!("Show pane {pane}")))
                .on_click(move |_: &ClickEvent, _, cx| open_pane(pane, cx))
        });
        let copy = can_copy.then(|| {
            ui::icon_text_button("queue-detail-copy", Icon::Copy, "Copy", Tone::Plain)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.copy_shown(cx)))
        });
        let close = ui::tool_button("queue-detail-close", Icon::Close, 26., false)
            .tooltip(tip("Close"))
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.close_detail(window, cx)));
        div().flex_none().flex().items_center().gap(px(6.)).children(open).children(copy).child(close)
    }

    fn error_row(detail: &Detail, cx: &mut Context<Self>) -> Option<Div> {
        let text = detail.error.clone()?;
        let again = ui::button("queue-detail-again", "Try again")
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.try_again(cx)));
        let line = ui::error_line(text).flex_1().min_w(px(0.));
        Some(div().flex().flex_wrap().items_start().gap(px(8.)).child(line).child(again))
    }

    fn page_switch(detail: &Detail, cx: &mut Context<Self>) -> Div {
        let message = ui::segment("queue-page-message", "Last message", detail.page == Page::Message)
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.choose_page(Page::Message, cx)));
        let talk = detail.answer.as_ref().is_some_and(Answer::can_talk).then(|| {
            ui::segment("queue-page-talk", "Conversation", detail.page == Page::Conversation)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.choose_page(Page::Conversation, cx)))
                .into_any_element()
        });
        ui::segments(std::iter::once(message.into_any_element()).chain(talk).collect())
    }

    fn view_switch(detail: &Detail, cx: &mut Context<Self>) -> Div {
        let rendered = ui::segment("queue-view-rendered", "Rendered", detail.view == View::Rendered)
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.choose_view(View::Rendered, cx)));
        let plain = ui::segment("queue-view-text", "Text", detail.view == View::Text)
            .tooltip(tip("Plain text, to select and copy"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.choose_view(View::Text, cx)));
        ui::segments(vec![rendered.into_any_element(), plain.into_any_element()])
    }

    fn body_heading(detail: &Detail, cx: &mut Context<Self>) -> Div {
        div()
            .flex_none()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .px(px(20.))
            .py(px(8.))
            .border_t_1()
            .border_color(colours::border())
            .child(Self::page_switch(detail, cx))
            .child(Self::view_switch(detail, cx))
    }

    pub(super) fn detail_view(
        &self,
        entries: &[Entry],
        snapshot: &Snapshot,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let detail = self.detail.as_ref()?;
        let entry = entries.iter().find(|e| e.id == detail.id)?;
        let shown = self.shown(entry, snapshot, cx);
        let refreshing = detail.answer.is_some() && Self::reading(detail, cx);
        let answer = detail.answer.as_ref();
        let kept = answer.and_then(|a| a.result.as_ref());
        let can_copy = answer.is_some_and(|a| can_copy(a, detail.page));
        let facts = facts(entry, kept, snapshot.today, snapshot.now);
        let column = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .px(px(20.))
            .pb(px(12.))
            .child(status_line(entry, &shown, snapshot, refreshing))
            .child(fact_rows(facts))
            .children(Self::error_row(detail, cx));
        let head = div().id("queue-detail-head").flex_none().max_h(relative(0.4)).overflow_y_scroll().child(column);
        let top = div()
            .flex_none()
            .flex()
            .items_start()
            .gap(px(8.))
            .px(px(20.))
            .pt(px(16.))
            .pb(px(10.))
            .child(title(&entry.label))
            .child(Self::actions_row(&shown, can_copy, cx));
        let frame = frame(self.side_width.size()).track_focus(&self.focus);
        let body = self.detail_body(detail, cx);
        Some(
            frame
                .child(top)
                .child(head)
                .child(Self::body_heading(detail, cx))
                .child(notes(detail))
                .child(body)
                .into_any_element(),
        )
    }

    fn reading(detail: &Detail, cx: &Context<Self>) -> bool {
        let store = Store::get(cx).read(cx);
        let talk_shown = detail.answer.as_ref().is_some_and(|a| a.talk.is_some());
        refreshing_keys(detail.id, talk_shown).iter().any(|key| store.jobs.shown(key, Instant::now()))
    }

    fn detail_body(&self, detail: &Detail, cx: &mut Context<Self>) -> AnyElement {
        let Some(answer) = detail.answer.as_ref() else {
            return if detail.error.is_some() { div().into_any_element() } else { loading("Reading what it did…") };
        };
        let talking = detail.page == Page::Conversation;
        if talking && answer.talk.is_none() {
            return if detail.error.is_some() {
                div().into_any_element()
            } else {
                loading("Reading the conversation…")
            };
        }
        if detail.view == View::Text {
            return self.text_body(detail, cx);
        }
        if !talking && answer.blocks.is_empty() {
            let empty = answer.note.is_none().then(|| ui::dim(NO_MESSAGE).px(px(20.)).pt(px(8.)));
            return div().children(empty).into_any_element();
        }
        let state = detail.lists.of(detail.page, View::Rendered).clone();
        let items = list(state, cx.processor(|this, ix: usize, _, _| this.rendered_item(ix)));
        div().flex_1().min_h_0().child(items.size_full()).into_any_element()
    }

    fn rendered_item(&self, ix: usize) -> AnyElement {
        let link = opens_web_addresses();
        let Some(answer) = self.detail.as_ref().and_then(|d| d.answer.as_ref().map(|a| (d.page, a))) else {
            return div().into_any_element();
        };
        let (page, answer) = answer;
        let body = match (page, &answer.talk) {
            (Page::Conversation, Some(Talk::Said(said))) => message_card(ix, said, &link),
            _ => answer.blocks.get(ix).map(|b| text::linked_block(format!("last-{ix}").into(), b, &link)),
        };
        div().w_full().px(px(20.)).pt(px(8.)).pb(px(8.)).children(body).into_any_element()
    }
}

fn loading(text: &'static str) -> AnyElement {
    ui::loading(text).into_any_element()
}

fn message_card(ix: usize, said: &Spoken, link: &OnLink) -> Option<AnyElement> {
    let message = said.messages.get(ix)?;
    let blocks = said.blocks.get(ix)?;
    let ground = if message.role == "prompt" { colours::selected() } else { colours::panel() };
    let today = time::local(time::now()).unwrap_or(time::Civil::EPOCH);
    let byline = div()
        .flex()
        .items_center()
        .gap(px(8.))
        .text_size(px(12.))
        .text_color(colours::muted())
        .child(div().flex_1().min_w(px(0.)).child(said_heading(message, today)))
        .when(!message.text.is_empty(), |d| d.child(ui::copy_button(("queue-said-copy", ix), message.text.clone())));
    let card = div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .p(px(10.))
        .rounded(px(radius::card()))
        .bg(ground)
        .text_size(px(13.))
        .child(byline)
        .children(text::linked_blocks(&format!("said-{ix}"), blocks, link))
        .when(message.cut, |d| d.child(ui::dim(MESSAGE_CUT)));
    Some(card.into_any_element())
}
