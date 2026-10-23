use gpui::{
    AnyElement, App, ClickEvent, ClipboardItem, Context, Div, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px,
};

use super::data::{self, Comment, Issue, ReadIssue, SourceInfo};
use super::{BodyView, IssuesPanel, error_note, label_chip, state_chip, text};
use crate::panels::PanelKind;
use crate::store::{Store, ToastKind};
use crate::theme::{FONT_FAMILY, colours};
use crate::ui;

impl IssuesPanel {
    pub(super) fn render_detail(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(issue) = self.selected.clone() else {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(ui::note("Choose an issue to read it and start work on it."))
                .into_any_element();
        };
        let read = self.reads.get(&issue.id());
        let info = issue.info(read.and_then(|r| r.as_ref().ok()));
        div()
            .id("issue-detail")
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .p(px(18.))
                    .child(self.detail_heading(&issue))
                    .child(div().text_size(px(18.)).font_weight(FontWeight::SEMIBOLD).child(issue.title.clone()))
                    .child(info_line(&issue, &info))
                    .child(self.detail_buttons(&issue.url, cx))
                    .child(self.detail_body(read)),
            )
            .into_any_element()
    }

    fn detail_heading(&self, issue: &Issue) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .text_size(px(12.))
            .child(div().font_family(FONT_FAMILY).text_color(colours::accent()).child(issue.key.clone()))
            .child(div().text_color(colours::faint()).child(self.source_name(&issue.source)))
            .children((!issue.state.is_empty()).then(|| state_chip(&issue.state, issue.closed)))
    }

    fn detail_buttons(&self, url: &str, cx: &mut Context<Self>) -> Div {
        let (open, copy) = (url.to_string(), url.to_string());
        div()
            .flex()
            .flex_wrap()
            .gap(px(6.))
            .pb(px(6.))
            .border_b_1()
            .border_color(colours::border())
            .child(
                ui::primary_button("issue-start", "Start…")
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_start(false, window, cx))),
            )
            .child(
                ui::button("issue-schedule", "Schedule…")
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_start(true, window, cx))),
            )
            .when(!url.is_empty(), |d| {
                d.child(
                    ui::button("issue-open", "Open in browser")
                        .on_click(move |_: &ClickEvent, _, cx| cx.open_url(&open)),
                )
                .child(ui::button("issue-copy", "Copy URL").on_click(move |_: &ClickEvent, _, cx| copy_url(&copy, cx)))
            })
            .child(div().flex_1())
            .child(self.view_switch(cx))
    }

    fn view_switch(&self, cx: &mut Context<Self>) -> Div {
        let raw = self.body_view == BodyView::Markdown;
        ui::segments(vec![
            ui::segment("issue-rendered", "Rendered", !raw)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_body_view(BodyView::Rendered, cx)))
                .into_any_element(),
            ui::segment("issue-raw", "Markdown", raw)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_body_view(BodyView::Markdown, cx)))
                .into_any_element(),
        ])
    }

    fn detail_body(&self, read: Option<&Result<ReadIssue, String>>) -> Div {
        let body = div().flex().flex_col().gap(px(8.)).text_size(px(13.)).text_color(colours::text());
        match read {
            None => body.child(ui::note("Reading the issue…").p_0()),
            Some(Err(e)) => body.child(error_note(e.clone())),
            Some(Ok(read)) => body.children(self.description(&read.body)).children(self.comments(&read.comments)),
        }
    }

    fn formatted(&self, id: &str, source: &str) -> Vec<AnyElement> {
        match self.body_view {
            BodyView::Markdown => vec![text::raw(source)],
            BodyView::Rendered => text::markdown(id, source),
        }
    }

    fn description(&self, body: &str) -> Vec<AnyElement> {
        if body.trim().is_empty() {
            return vec![ui::dim("No description.").into_any_element()];
        }
        self.formatted("body", body)
    }

    fn comments(&self, comments: &[Comment]) -> Vec<AnyElement> {
        if comments.is_empty() {
            return Vec::new();
        }
        let now = data::now();
        let heading = ui::section(format!("Comments · {}", comments.len())).into_any_element();
        let each = comments.iter().enumerate().map(|(i, comment)| self.comment(i, comment, now));
        std::iter::once(heading).chain(each).collect()
    }

    fn comment(&self, index: usize, comment: &Comment, now: i64) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .p(px(10.))
            .rounded(px(6.))
            .bg(colours::sidebar())
            .border_1()
            .border_color(colours::border())
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .text_size(px(12.))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(comment.author.clone()))
                    .child(div().text_color(colours::faint()).child(comment.when(now))),
            )
            .children(self.formatted(&format!("comment-{index}"), &comment.body))
            .into_any_element()
    }

    pub(super) fn render_connect(&self, only: Option<&SourceInfo>, cx: &mut Context<Self>) -> AnyElement {
        let sources: Vec<SourceInfo> = match only {
            Some(source) => vec![source.clone()],
            None => self.sources.as_ref().map(|s| s.list.clone()).unwrap_or_default(),
        };
        let (heading, how) = connect_text(only);
        div()
            .size_full()
            .flex()
            .justify_center()
            .pt(px(40.))
            .child(
                div()
                    .w(px(720.))
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(div().text_size(px(17.)).font_weight(FontWeight::SEMIBOLD).child(heading))
                    .child(div().text_color(colours::muted()).child(how))
                    .child(div().flex().flex_col().text_size(px(12.5)).children(sources.iter().map(connect_row)))
                    .child(connect_buttons(cx)),
            )
            .into_any_element()
    }
}

fn info_line(issue: &Issue, info: &[String]) -> Div {
    div()
        .flex()
        .flex_wrap()
        .gap(px(6.))
        .items_center()
        .children(issue.info_extras(info).into_iter().map(|i| ui::dim(i).text_size(px(12.))))
        .children(issue.labels.iter().cloned().map(label_chip))
}

fn copy_url(url: &str, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string(url.to_string()));
    Store::get(cx).update(cx, |s, cx| s.toast("Copied the issue's URL", ToastKind::Info, cx));
}

fn connect_text(only: Option<&SourceInfo>) -> (String, String) {
    match only {
        Some(source) if source.source == "github" => (
            format!("{} is not connected", source.name),
            "GitHub goes through the gh command line in the project's folder: sign in with gh, or open a \
             project whose repository is on GitHub."
                .to_string(),
        ),
        Some(source) => (
            format!("{} is not connected", source.name),
            format!(
                "Connect it in Settings → Issues → {}, or from a shell. Its token is checked and kept by \
                 cornercase, never by this window.",
                source.name
            ),
        ),
        None => (
            "No issue tracker is connected".to_string(),
            "Connect GitHub with the gh command line, and the other trackers with a token in Settings → \
             Issues. Tokens are checked and kept by cornercase, never by this window."
                .to_string(),
        ),
    }
}

fn connect_row(source: &SourceInfo) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .py(px(5.))
        .border_b_1()
        .border_color(colours::border())
        .child(div().w(px(80.)).flex_none().font_weight(FontWeight::SEMIBOLD).child(source.name.clone()))
        .child(div().w(px(260.)).flex_none().text_color(colours::muted()).child(source.missing_line()))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .font_family(FONT_FAMILY)
                .text_size(px(11.))
                .text_color(colours::faint())
                .child(source.connect_hint()),
        )
}

fn connect_buttons(cx: &mut Context<IssuesPanel>) -> Div {
    div()
        .flex()
        .gap(px(8.))
        .pt(px(6.))
        .child(ui::primary_button("issues-settings", "Open Settings").on_click(|_: &ClickEvent, _, cx| {
            Store::get(cx).update(cx, |s, cx| s.open(PanelKind::Settings, cx));
        }))
        .child(
            ui::button("issues-check", "Check again")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.refresh(cx))),
        )
}
