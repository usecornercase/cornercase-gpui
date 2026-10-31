use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, div, px,
};

use super::data::SourceInfo;
use super::{IssuesPanel, card, copy_text, state_dot};
use crate::notify::tip;
use crate::panels::PanelKind;
use crate::store::Store;
use crate::theme::{FONT_FAMILY, colours, radius};
use crate::ui::{self, Icon};

impl IssuesPanel {
    pub(super) fn render_connect(&self, only: Option<&SourceInfo>, cx: &mut Context<Self>) -> AnyElement {
        let sources: Vec<SourceInfo> = match only {
            Some(source) => vec![source.clone()],
            None => self.sources.as_ref().map(|s| s.list.clone()).unwrap_or_default(),
        };
        let (heading, how) = connect_text(only);
        div()
            .id("issues-connect")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(12.))
                    .px(px(24.))
                    .pt(px(44.))
                    .pb(px(24.))
                    .child(ui::emblem(Icon::Issues))
                    .child(div().text_size(px(17.)).font_weight(FontWeight::SEMIBOLD).child(heading))
                    .child(div().max_w(px(540.)).text_center().text_color(colours::muted()).child(how))
                    .child(trackers(&sources))
                    .child(connect_buttons(cx)),
            )
            .into_any_element()
    }
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

fn trackers(sources: &[SourceInfo]) -> Div {
    div()
        .w_full()
        .max_w(px(620.))
        .mt(px(8.))
        .flex()
        .flex_col()
        .gap(px(8.))
        .children(sources.iter().enumerate().map(|(index, source)| tracker(index, source)))
}

fn tracker(index: usize, source: &SourceInfo) -> Div {
    card()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(state_dot(source.state))
                .child(div().flex_none().font_weight(FontWeight::SEMIBOLD).child(source.name.clone()))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(12.))
                        .text_color(colours::muted())
                        .child(ui::one_line(source.status_line())),
                ),
        )
        .child(command(index, source.connect_hint()))
}

fn command(index: usize, hint: String) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .pl(px(10.))
        .pr(px(3.))
        .py(px(3.))
        .rounded(px(radius::CONTROL))
        .bg(colours::field())
        .border_1()
        .border_color(colours::border())
        .child(
            div()
                .flex_1()
                .min_w_0()
                .font_family(FONT_FAMILY)
                .text_size(px(11.5))
                .text_color(colours::soft())
                .child(ui::one_line(hint.clone())),
        )
        .child(
            ui::tool_button(("connect-copy", index), Icon::Copy, 24., false)
                .tooltip(tip("Copy the command"))
                .on_click(move |_: &ClickEvent, _, cx| copy_text(hint.clone(), "the command", cx)),
        )
}

fn connect_buttons(cx: &mut Context<IssuesPanel>) -> Div {
    div()
        .flex()
        .gap(px(8.))
        .pt(px(10.))
        .child(ui::primary_button("issues-settings", "Open Settings").on_click(|_: &ClickEvent, _, cx| {
            Store::get(cx).update(cx, |s, cx| s.open(PanelKind::Settings, cx));
        }))
        .child(
            ui::button("issues-check", "Check again")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.refresh(cx))),
        )
}
