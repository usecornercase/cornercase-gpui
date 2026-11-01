use gpui::{
    App, AppContext as _, ClickEvent, ClipboardItem, Context, Div, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, KeyDownEvent, ParentElement as _, Render, Stateful, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, div, prelude::FluentBuilder as _, px,
};

use super::{body_height, close, frame, show};
use crate::model::{PullRequest, Session, Workspace};
use crate::store::{Store, ToastKind};
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, Tone};

mod signs;
mod stack;

use signs::{Sign, check_lines, checks_chip, state_chip};
pub(crate) use signs::{pull_tag, stack_tag};

const CONFLICT_NOTE: &str =
    "GitHub runs no pull_request workflows until the conflict is resolved, so checks may be missing.";
const WAITS_ON: &str = "Every check, with its workflow and link, needs";
const WAITS_ON_ISSUE: &str = "usecornercase/cornercase-terminal#567";
const WAITS_ON_URL: &str = "https://github.com/usecornercase/cornercase-terminal/issues/567";

pub(crate) struct PullRequestDialog {
    workspace: u64,
    focus: FocusHandle,
    _store: Subscription,
}

struct Shown {
    workspace: u64,
    pr: PullRequest,
    branch: Option<String>,
    behind: u64,
    holders: Vec<Option<(u64, String)>>,
}

impl Shown {
    fn take(session: &Session, workspace: u64) -> Option<Self> {
        let (_, ws) = session.workspace(workspace)?;
        let pr = ws.pull_request.clone()?;
        let holder = |url: &str| session.pull_request_holder(workspace, url).map(|w| (w.id, w.name.clone()));
        let holders = pr
            .stack
            .as_ref()
            .map(|stack| stack.rungs(pr.number).iter().map(|r| holder(&r.layer.url)).collect())
            .unwrap_or_default();
        Some(Self { workspace, branch: ws.branch.clone(), behind: ws.behind, holders, pr })
    }
}

pub(crate) fn pull_request(workspace: u64, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| {
        let store = Store::get(cx);
        let follow = cx.observe(&store, |this: &mut PullRequestDialog, store, cx| this.follow(&store, cx));
        PullRequestDialog { workspace, focus: cx.focus_handle(), _store: follow }
    });
    let focus = view.read(cx).focus.clone();
    show(view.into(), &focus, window, cx);
}

fn closed_or_covered(cx: &mut App) -> bool {
    let store = Store::get(cx);
    let Some(dialog) = store.read(cx).dialog.as_ref() else { return false };
    if dialog.entity_type() == std::any::TypeId::of::<PullRequestDialog>() {
        store.update(cx, Store::close_dialog);
    }
    true
}

pub(crate) fn shown_pull_request(window: &mut Window, cx: &mut App) {
    if closed_or_covered(cx) {
        return;
    }
    let store = Store::get(cx);
    let session = store.read(cx).session.as_ref();
    let found = session.and_then(|s| s.workspace(s.shown().workspace?)).map(|(_, ws)| (ws.id, missing(ws)));
    match found {
        Some((id, None)) => pull_request(id, window, cx),
        Some((_, Some(why))) => store.update(cx, |s, cx| s.toast(why, ToastKind::Info, cx)),
        None => {}
    }
}

fn missing(ws: &Workspace) -> Option<String> {
    if ws.pull_request.is_some() {
        return None;
    }
    Some(match ws.branch.as_deref() {
        Some(branch) => format!("No pull request for {branch}"),
        None => format!("{} is not on a branch", ws.name),
    })
}

fn pull_of(session: &Session, workspace: u64) -> Option<&PullRequest> {
    session.workspace(workspace)?.1.pull_request.as_ref()
}

impl PullRequestDialog {
    fn follow(&mut self, store: &Entity<Store>, cx: &mut Context<Self>) {
        let s = store.read(cx);
        let gone = s.session.as_ref().is_none_or(|session| pull_of(session, self.workspace).is_none());
        let mine = s.dialog.as_ref().is_some_and(|d| d.entity_id() == cx.entity_id());
        if gone && mine {
            store.update(cx, Store::close_dialog);
        }
    }
}

impl Render for PullRequestDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let shown = Store::get(cx).read(cx).session.as_ref().and_then(|s| Shown::take(s, self.workspace));
        let url = shown.as_ref().map(|s| s.pr.url.clone()).unwrap_or_default();
        let height = body_height(f32::from(window.viewport_size().height));
        div()
            .track_focus(&self.focus)
            .on_key_down(move |e: &KeyDownEvent, _, cx| {
                if e.keystroke.key == "enter" {
                    open(&url, cx);
                }
            })
            .children(shown.map(|shown| frame(shown.pr.heading(), body(&shown, height), buttons(&shown)).w(px(540.))))
    }
}

fn body(shown: &Shown, height: f32) -> impl IntoElement {
    div()
        .id("pr-body")
        .max_h(px(height))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .child(header(shown))
        .children(stack::section(shown))
        .child(checks(&shown.pr))
}

fn header(shown: &Shown) -> Div {
    let pr = &shown.pr;
    let chips = div()
        .flex()
        .flex_wrap()
        .gap(px(6.))
        .child(state_chip(pr))
        .children(pr.conflict_said().map(|said| ui::tinted_chip(said, colours::waiting())))
        .child(checks_chip(pr));
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .when(!pr.title.is_empty(), |d| d.child(div().text_size(px(14.)).child(pr.title.clone())))
        .child(chips)
        .children(branches(shown))
        .when(pr.conflicts && !pr.on_gitlab(), |d| d.child(muted(CONFLICT_NOTE)))
}

fn branches(shown: &Shown) -> Option<Div> {
    let branch = shown.branch.as_deref()?;
    let into = shown.pr.base.as_deref().map(|base| format!(" → {base}")).unwrap_or_default();
    Some(
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .text_size(px(12.))
            .text_color(colours::muted())
            .child(ui::icon(Icon::Branch, 12.).text_color(colours::faint()))
            .child(div().flex_1().min_w_0().child(ui::one_line(format!("{branch}{into}")))),
    )
}

fn muted(text: impl Into<gpui::SharedString>) -> Div {
    div().text_size(px(12.)).text_color(colours::muted()).child(text.into())
}

fn checks(pr: &PullRequest) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(ui::section(if pr.on_gitlab() { "Pipeline" } else { "Checks" }))
        .children(check_lines(pr).into_iter().map(|(sign, text)| check_line(sign, text)))
        .child(waits_on())
}

fn check_line(sign: Sign, text: String) -> Div {
    let glyph = sign.icon().unwrap_or(Icon::Idle);
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .px(px(8.))
        .text_size(px(12.5))
        .text_color(colours::soft())
        .child(ui::icon(glyph, 13.).text_color(if sign == Sign::Quiet { colours::faint() } else { sign.colour() }))
        .child(div().flex_1().min_w_0().child(ui::one_line(text)))
}

fn waits_on() -> Stateful<Div> {
    div()
        .id("pr-waits-on")
        .mt(px(6.))
        .px(px(8.))
        .py(px(4.))
        .flex()
        .items_start()
        .gap(px(8.))
        .rounded(px(radius::CONTROL))
        .text_size(px(11.5))
        .text_color(colours::faint())
        .cursor_pointer()
        .hover(|s| s.bg(colours::hover()))
        .child(div().pt(px(2.)).child(ui::icon(Icon::External, 12.).text_color(colours::faint())))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(WAITS_ON)
                .child(div().text_color(colours::accent()).child(WAITS_ON_ISSUE)),
        )
        .on_click(|_: &ClickEvent, _, cx| cx.open_url(WAITS_ON_URL))
}

fn buttons(shown: &Shown) -> Div {
    let pr = &shown.pr;
    div()
        .flex_1()
        .flex()
        .items_center()
        .gap(px(8.))
        .children(pr.problems().map(|text| ask_button(shown.workspace, text, pr.label())))
        .child(div().flex_1())
        .child(copy_button(pr.url.clone()))
        .child(ui::button("pr-done", "Done").on_click(|_: &ClickEvent, _, cx| close(cx)))
        .child(open_button(pr))
}

fn ask_button(workspace: u64, text: String, label: String) -> Stateful<Div> {
    ui::icon_text_button("pr-ask", Icon::Send, "Ask agent", Tone::Plain).on_click(move |_: &ClickEvent, _, cx| {
        close(cx);
        Store::get(cx).update(cx, |s, cx| s.ask_agent(workspace, &text, &label, cx));
    })
}

fn copy_button(url: String) -> Stateful<Div> {
    ui::icon_text_button("pr-copy", Icon::Copy, "Copy link", Tone::Plain).on_click(move |_: &ClickEvent, _, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string(url.clone()));
        Store::get(cx).update(cx, |s, cx| {
            s.close_dialog(cx);
            s.toast("Copied the link", ToastKind::Info, cx);
        });
    })
}

fn open_button(pr: &PullRequest) -> Stateful<Div> {
    let url = pr.url.clone();
    ui::icon_text_button("pr-open", Icon::External, format!("Open on {}", pr.site()), Tone::Primary)
        .on_click(move |_: &ClickEvent, _, cx| open(&url, cx))
}

fn open(url: &str, cx: &mut App) {
    if !url.is_empty() {
        cx.open_url(url);
    }
    close(cx);
}

#[cfg(test)]
mod tests;
