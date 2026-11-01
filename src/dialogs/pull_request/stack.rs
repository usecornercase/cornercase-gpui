use gpui::{
    ClickEvent, Div, FontWeight, InteractiveElement as _, ParentElement as _, Rgba, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled, div, prelude::FluentBuilder as _, px,
};

use super::signs::layer_sign;
use super::{Shown, muted};
use crate::model::{Block, Layer, PullRequest, Rung, Stack};
use crate::store::Store;
use crate::theme::{colours, radius};
use crate::ui::{self, Icon};

const MERGED_BELOW: &str = "was merged and this branch is behind its remote: pull it before pushing again.";

pub(super) fn section(shown: &Shown) -> Option<Div> {
    let pr = &shown.pr;
    let stack = pr.stack.as_ref()?;
    let rows = stack.rungs(pr.number).into_iter().zip(&shown.holders).map(|(rung, held)| row(pr, rung, held.as_ref()));
    Some(
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(ui::section("Stack"))
            .child(muted(heading(stack)).pb(px(4.)))
            .children(rows)
            .children(blocker(pr))
            .children(merged_below(pr, stack, shown.behind).map(|text| muted(text).pt(px(4.)))),
    )
}

pub(super) fn heading(stack: &Stack) -> String {
    format!("#{} on {} · layer {} of {}", stack.number, stack.base, stack.position, stack.size)
}

fn tone(layer: &Layer, own: bool) -> Rgba {
    match layer.state.as_str() {
        "merged" => colours::merged(),
        "closed" => colours::faint(),
        _ if own => colours::text(),
        _ => colours::soft(),
    }
}

fn row(pr: &PullRequest, rung: Rung<'_>, held: Option<&(u64, String)>) -> Stateful<Div> {
    let layer = rung.layer;
    let sign = layer_sign(pr, layer, rung.own);
    let line = div()
        .id(SharedString::from(format!("pr-layer-{}", layer.number)))
        .flex()
        .items_center()
        .gap(px(8.))
        .px(px(8.))
        .py(px(4.))
        .rounded(px(radius::CONTROL))
        .text_size(px(12.5))
        .text_color(tone(layer, rung.own))
        .child(div().w(px(16.)).flex_none().text_right().text_color(colours::faint()).child(rung.position.to_string()))
        .child(div().flex_none().child(format!("#{}", layer.number)))
        .child(div().w(px(13.)).flex_none().children(sign.icon().map(|g| ui::icon(g, 13.).text_color(sign.colour()))))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .when(layer.state == "closed", Styled::line_through)
                .child(ui::one_line(layer.title.clone())),
        )
        .children(held.map(|(_, name)| holder(name)));
    if rung.own {
        return line.bg(colours::selected()).font_weight(FontWeight::MEDIUM);
    }
    match held {
        Some((workspace, _)) => line.cursor_pointer().hover(|s| s.bg(colours::hover())).on_click(go(*workspace)),
        None => line,
    }
}

fn holder(name: &str) -> Div {
    div()
        .flex()
        .flex_none()
        .max_w(px(150.))
        .items_center()
        .gap(px(4.))
        .text_size(px(11.5))
        .text_color(colours::muted())
        .child(ui::icon(Icon::Branch, 11.).text_color(colours::faint()))
        .child(ui::one_line(name.to_string()))
}

fn go(workspace: u64) -> impl Fn(&ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static {
    move |_, _, cx| {
        Store::get(cx).update(cx, |s, cx| {
            s.focus("workspace", workspace);
            s.close_dialog(cx);
        });
    }
}

pub(super) fn blocker_said(pr: &PullRequest) -> Option<(String, Block)> {
    let (layer, block) = pr.blocked()?;
    let what = match block {
        Block::Checks => "passes its checks",
        Block::Conflicts => "has no merge conflicts",
        Block::Review => "is approved",
    };
    Some((format!("Cannot merge until #{} below {what}", layer.number), block))
}

fn blocker(pr: &PullRequest) -> Option<Div> {
    let (text, block) = blocker_said(pr)?;
    let line = match block {
        Block::Checks => ui::error_line(text),
        Block::Conflicts => ui::warning_line(text),
        Block::Review => muted(text),
    };
    Some(line.pt(px(4.)))
}

pub(super) fn merged_below(pr: &PullRequest, stack: &Stack, behind: u64) -> Option<String> {
    if pr.settled() || behind == 0 {
        return None;
    }
    let layer = stack.merged_below(pr.number)?;
    Some(format!("#{} below {MERGED_BELOW}", layer.number))
}

#[cfg(test)]
mod tests;
