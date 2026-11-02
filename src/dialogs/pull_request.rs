use gpui::{
    App, AppContext as _, ClickEvent, ClipboardItem, Context, Div, Entity, FocusHandle, HighlightStyle,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, Subscription, Task, Window, div, prelude::FluentBuilder as _, px,
};

use super::{body_height, close, frame, show};
use crate::model::{PullRequest, Session, Workspace};
use crate::store::{self, Busy, Store, ToastKind};
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, Tone};

mod read;
mod signs;
mod stack;

use read::{Note, REFRESH_AFTER, Reads, answer_of};
use signs::{CheckLine, Sign, check_lines, checks_chip, state_chip};
pub(crate) use signs::{pull_tag, stack_tag};

const CONFLICT_NOTE: &str =
    "GitHub runs no pull_request workflows until the conflict is resolved, so checks may be missing.";

pub(crate) struct PullRequestDialog {
    workspace: u64,
    focus: FocusHandle,
    reads: Reads,
    refresh: Option<Task<()>>,
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
        PullRequestDialog {
            workspace,
            focus: cx.focus_handle(),
            reads: Reads::default(),
            refresh: None,
            _store: follow,
        }
    });
    let focus = view.read(cx).focus.clone();
    show(view.clone().into(), &focus, window, cx);
    view.update(cx, |dialog, cx| {
        let pr = Store::get(cx).read(cx).session.as_ref().and_then(|s| pull_of(s, workspace)).cloned();
        dialog.ask(pr, cx);
    });
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
    fn seen(&self, cx: &Context<Self>) -> (bool, Option<PullRequest>) {
        let store = Store::get(cx);
        let s = store.read(cx);
        let mine = s.dialog.as_ref().is_some_and(|d| d.entity_id() == cx.entity_id());
        (mine, s.session.as_ref().and_then(|session| pull_of(session, self.workspace)).cloned())
    }

    fn follow(&mut self, store: &Entity<Store>, cx: &mut Context<Self>) {
        let (mine, pr) = self.seen(cx);
        if !mine {
            return;
        }
        if pr.is_none() {
            store.update(cx, Store::close_dialog);
        }
        self.ask(pr, cx);
    }

    fn ask(&mut self, pr: Option<PullRequest>, cx: &mut Context<Self>) {
        if let Some(pr) = pr.filter(|pr| self.reads.due(pr)) {
            self.send(&pr, cx);
        }
    }

    fn send(&mut self, pr: &PullRequest, cx: &mut Context<Self>) {
        let generation = self.reads.start(pr);
        let workspace = self.workspace;
        let busy =
            Busy::new(format!("pull-request:{workspace}"), format!("Asking {} about {}…", pr.site(), pr.reference()));
        store::run_busy(cx, busy, read::args(workspace), move |this, result, cx| {
            if this.reads.settle(generation, answer_of(workspace, result)) {
                this.schedule(cx);
                cx.notify();
            }
        });
    }

    fn schedule(&mut self, cx: &mut Context<Self>) {
        self.refresh = self.reads.refresh_due().then(|| {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(REFRESH_AFTER).await;
                let _ = this.update(cx, PullRequestDialog::refresh);
            })
        });
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let (mine, pr) = self.seen(cx);
        if let Some(pr) = pr.filter(|_| mine && self.reads.refresh_due()) {
            self.send(&pr, cx);
        }
    }
}

impl Render for PullRequestDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut shown = Store::get(cx).read(cx).session.as_ref().and_then(|s| Shown::take(s, self.workspace));
        if let Some(shown) = shown.as_mut() {
            self.reads.fill(&mut shown.pr);
        }
        let url = shown.as_ref().map(|s| s.pr.url.clone()).unwrap_or_default();
        let height = body_height(f32::from(window.viewport_size().height));
        let note = self.reads.note();
        div()
            .track_focus(&self.focus)
            .on_key_down(move |e: &KeyDownEvent, _, cx| {
                if e.keystroke.key == "enter" {
                    open(&url, cx);
                }
            })
            .children(
                shown.map(|shown| frame(shown.pr.heading(), body(&shown, note, height), buttons(&shown)).w(px(540.))),
            )
    }
}

fn body(shown: &Shown, note: Option<Note<'_>>, height: f32) -> impl IntoElement {
    div()
        .id("pr-body")
        .max_h(px(height))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .child(header(shown))
        .children(stack::section(shown))
        .child(checks(&shown.pr, note))
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

fn muted(text: impl Into<SharedString>) -> Div {
    div().text_size(px(12.)).text_color(colours::muted()).child(text.into())
}

fn checks(pr: &PullRequest, note: Option<Note<'_>>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(ui::section(if pr.on_gitlab() { "Pipeline" } else { "Checks" }))
        .children(check_lines(pr).into_iter().enumerate().map(|(at, line)| check_line(at, line)))
        .children(note.map(|note| note_line(note, pr.site())))
}

fn check_line(at: usize, line: CheckLine) -> Stateful<Div> {
    let glyph = line.sign.icon().unwrap_or(Icon::Idle);
    let ink = if line.sign == Sign::Quiet { colours::faint() } else { line.sign.colour() };
    let (text, workflow) = line.text();
    let muted = HighlightStyle { color: Some(colours::muted().into()), ..Default::default() };
    let row = div()
        .id(SharedString::from(format!("pr-check-{at}")))
        .flex()
        .items_center()
        .gap(px(8.))
        .px(px(8.))
        .py(px(3.))
        .rounded(px(radius::CONTROL))
        .text_size(px(12.5))
        .text_color(colours::soft())
        .child(ui::icon(glyph, 13.).text_color(ink))
        .child(div().flex_1().min_w_0().child(ui::one_line(text).highlights(workflow.map(|range| (range, muted)))));
    if line.url.is_empty() {
        return row;
    }
    let url = line.url;
    row.cursor_pointer()
        .hover(|s| s.bg(colours::hover()))
        .child(ui::icon(Icon::External, 12.).text_color(colours::faint()))
        .on_click(move |_: &ClickEvent, _, cx| cx.open_url(&url))
}

fn note_line(note: Note<'_>, site: &str) -> Div {
    let line = match note {
        Note::Asking => return ui::busy("pr-asking", format!("Asking {site}…")).px(px(8.)).pt(px(4.)),
        Note::Failed(why) => div()
            .text_color(colours::failed())
            .child(ui::icon(Icon::Alert, 12.).text_color(colours::failed()))
            .child(div().flex_1().min_w_0().child(ui::one_line(why.to_string()))),
        Note::Said(text) => {
            div().text_color(colours::muted()).child(div().flex_1().min_w_0().child(ui::one_line(text.to_string())))
        }
    };
    line.flex().items_center().gap(px(6.)).px(px(8.)).pt(px(4.)).text_size(px(12.))
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
