use std::rc::Rc;

use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, IntoElement, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, WeakEntity, div, list, prelude::FluentBuilder as _, px,
};

use super::data::{self, Comment, Issue};
use super::page::{Item, Page, Read};
use super::{BodyView, IssuesPanel, blank, card, copy_text, error_note, label_chip, state_icon, text};
use crate::notify::tip;
use crate::theme::{FONT_FAMILY, colours};
use crate::ui::{self, Icon, Tone, glass};

struct Detail {
    page: Rc<Page>,
    view: BodyView,
    panel: WeakEntity<IssuesPanel>,
    now: i64,
}

impl IssuesPanel {
    pub(super) fn render_detail(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(issue) = self.selected.clone() else {
            return blank("Choose an issue to read it and start work on it.").size_full().into_any_element();
        };
        let read = self.reads.get(&issue.id());
        let info = issue.info(read.and_then(|r| r.as_ref().ok()).map(|page| &page.read));
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(self.detail_header(&issue, &info, cx))
            .child(self.detail_text(read, cx))
            .into_any_element()
    }

    fn detail_header(&self, issue: &Issue, info: &[String], cx: &mut Context<Self>) -> Div {
        div()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(10.))
            .px(px(20.))
            .pt(px(16.))
            .pb(px(14.))
            .border_b_1()
            .border_color(colours::border())
            .child(self.detail_heading(issue))
            .child(
                div()
                    .text_size(px(18.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .line_clamp(3)
                    .text_ellipsis()
                    .child(issue.title.clone()),
            )
            .child(info_line(issue, info))
            .child(detail_actions(&issue.url, cx))
    }

    fn detail_heading(&self, issue: &Issue) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .text_size(px(12.))
            .child(state_pill(issue))
            .child(div().font_family(FONT_FAMILY).text_color(colours::accent()).child(issue.key.clone()))
            .child(div().text_color(colours::faint()).child("·"))
            .child(div().text_color(colours::muted()).child(self.source_name(&issue.source)))
    }

    fn detail_text(&self, read: Option<&Read>, cx: &mut Context<Self>) -> AnyElement {
        let (view, panel) = (self.body_view, cx.entity().downgrade());
        match read {
            Some(Ok(page)) => Detail { page: page.clone(), view, panel, now: data::now() }.render(),
            Some(Err(e)) => unread(view, &panel, error_note(e.clone())),
            None => unread(view, &panel, ui::loading("Reading the issue…").p_0()),
        }
    }
}

impl Detail {
    fn render(self) -> AnyElement {
        let state = self.page.list(self.view);
        list(state, move |index, _, _| self.item(index)).flex_1().into_any_element()
    }

    fn item(&self, index: usize) -> AnyElement {
        let items = self.page.items(self.view);
        let Some(&item) = items.get(index) else { return nothing() };
        let top = match index {
            0 => 14.,
            1 => 10.,
            _ => 8.,
        };
        div()
            .w_full()
            .px(px(20.))
            .pt(px(top))
            .when(index + 1 == items.len(), |d| d.pb(px(24.)))
            .text_size(px(13.))
            .text_color(colours::text())
            .child(self.content(item))
            .into_any_element()
    }

    fn content(&self, item: Item) -> AnyElement {
        let page = &self.page;
        match item {
            Item::Heading => body_heading(self.view, &self.panel).into_any_element(),
            Item::Empty => ui::dim("No description.").text_size(px(12.)).into_any_element(),
            Item::Block(i) => page.body.get(i).map_or_else(nothing, |b| text::block(format!("body-{i}").into(), b)),
            Item::Source => text::raw(&page.read.body),
            Item::Comments => ui::section(format!("Comments · {}", page.read.comments.len())).into_any_element(),
            Item::Comment(i) => self.comment(i),
        }
    }

    fn comment(&self, index: usize) -> AnyElement {
        let (Some(comment), Some(blocks)) = (self.page.read.comments.get(index), self.page.comments.get(index)) else {
            return nothing();
        };
        let body = match self.view {
            BodyView::Rendered => text::blocks(&format!("comment-{index}"), blocks),
            BodyView::Markdown => vec![text::raw(&comment.body)],
        };
        card().child(byline(comment, self.now)).children(body).into_any_element()
    }
}

fn nothing() -> AnyElement {
    div().into_any_element()
}

fn unread(view: BodyView, panel: &WeakEntity<IssuesPanel>, note: Div) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .px(px(20.))
        .pt(px(14.))
        .child(body_heading(view, panel))
        .child(note)
        .into_any_element()
}

fn body_heading(view: BodyView, panel: &WeakEntity<IssuesPanel>) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(8.))
        .child(ui::section("Description").p_0())
        .child(view_switch(view, panel))
}

fn view_switch(view: BodyView, panel: &WeakEntity<IssuesPanel>) -> Div {
    ui::segments(vec![
        view_segment(BodyView::Rendered, view, panel.clone()),
        view_segment(BodyView::Markdown, view, panel.clone()),
    ])
}

fn view_segment(view: BodyView, shown: BodyView, panel: WeakEntity<IssuesPanel>) -> AnyElement {
    let (id, label) = match view {
        BodyView::Rendered => ("issue-rendered", "Rendered"),
        BodyView::Markdown => ("issue-raw", "Markdown"),
    };
    ui::segment(id, label, view == shown)
        .on_click(move |_: &ClickEvent, _, cx| {
            let _ = panel.update(cx, |this, cx| this.set_body_view(view, cx));
        })
        .into_any_element()
}

fn state_pill(issue: &Issue) -> Div {
    let colour = if issue.closed { colours::muted() } else { colours::done() };
    div()
        .flex()
        .items_center()
        .gap(px(5.))
        .px(px(8.))
        .py(px(2.))
        .rounded_full()
        .bg(glass::tint(colour, 0.14))
        .text_size(px(11.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(colour)
        .child(state_icon(issue, 12.).text_color(colour))
        .child(issue.state_label())
}

fn info_line(issue: &Issue, info: &[String]) -> Div {
    let mut parts = issue.info_extras(info);
    if !issue.updated_at.is_empty() {
        parts.push(format!("updated {}", data::ago(&issue.updated_at, data::now())));
    }
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(6.))
        .text_size(px(12.))
        .text_color(colours::muted())
        .child(parts.join(" · "))
        .children(issue.labels.iter().cloned().map(label_chip))
}

fn detail_actions(url: &str, cx: &mut Context<IssuesPanel>) -> Div {
    let (open, copy) = (url.to_string(), url.to_string());
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(6.))
        .child(
            ui::icon_text_button("issue-start", Icon::Play, "Start…", Tone::Primary)
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_start(false, window, cx))),
        )
        .child(
            ui::icon_text_button("issue-schedule", Icon::Queue, "Schedule…", Tone::Plain)
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_start(true, window, cx))),
        )
        .child(
            ui::icon_text_button("issue-flow", Icon::Flows, "Start a flow…", Tone::Plain)
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.start_flow(window, cx))),
        )
        .child(div().flex_1())
        .when(!url.is_empty(), |d| {
            d.child(
                ui::tool_button("issue-open", Icon::External, 28., false)
                    .tooltip(tip("Open in the browser"))
                    .on_click(move |_: &ClickEvent, _, cx| cx.open_url(&open)),
            )
            .child(
                ui::tool_button("issue-copy", Icon::Copy, 28., false)
                    .tooltip(tip("Copy the issue's link"))
                    .on_click(move |_: &ClickEvent, _, cx| copy_text(copy.clone(), "the issue's link", cx)),
            )
        })
}

fn byline(comment: &Comment, now: i64) -> Div {
    let initial: String = comment.author.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .text_size(px(12.))
        .child(
            div()
                .size(px(20.))
                .flex_none()
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(glass::tint(colours::brand(), 0.35))
                .text_size(px(10.))
                .font_weight(FontWeight::BOLD)
                .text_color(colours::on_brand())
                .child(initial),
        )
        .child(div().font_weight(FontWeight::SEMIBOLD).child(comment.author.clone()))
        .child(div().text_color(colours::faint()).child(comment.when(now)))
}
