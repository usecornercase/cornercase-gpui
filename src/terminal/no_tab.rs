use gpui::{
    ClickEvent, Context, Div, FontWeight, ParentElement as _, Stateful, StatefulInteractiveElement as _, Styled as _,
    div, px,
};

use super::mouse::mouse;
use crate::app::{Drag, Root};
use crate::dialogs;
use crate::keymap::{Command, with_keys};
use crate::model::Session;
use crate::render::moving::Moves;
use crate::sidebar::{cmd, reopening, tip};
use crate::theme::colours;
use crate::ui::{self, Icon, Tone};

const TAGLINE: &str = "every agent in its own corner";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Empty {
    Welcome,
    NoTab(NoTab),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NoTab {
    pub(super) name: String,
    pub(super) workspace: Option<u64>,
    pub(super) closed: usize,
    pub(super) removing: bool,
}

pub(super) fn empty_of(session: &Session, removing: impl Fn(u64) -> bool) -> Empty {
    if session.projects.is_empty() {
        return Empty::Welcome;
    }
    let shown = session.shown();
    if let Some((project, ws)) = shown.workspace.and_then(|id| session.workspace(id)) {
        let name = if ws.worktree { &ws.name } else { &project.name }.clone();
        let closed = ws.closed_agents.unwrap_or(0);
        return Empty::NoTab(NoTab { name, workspace: Some(ws.id), closed, removing: removing(ws.id) });
    }
    let project = shown.project.and_then(|id| session.projects.iter().find(|p| p.id == id));
    let name = project.map(|p| p.name.clone()).unwrap_or_default();
    Empty::NoTab(NoTab { name, workspace: None, closed: 0, removing: false })
}

pub(super) fn lead(no_tab: &NoTab) -> String {
    let place = if no_tab.name.is_empty() { "this project" } else { no_tab.name.as_str() };
    if no_tab.removing {
        return format!("{place} is being removed, with its worktree.");
    }
    match no_tab.closed {
        0 => format!("Nothing runs in {place}. Start an agent, or open a shell in its folder."),
        1 => format!("Nothing runs in {place}. Its closed agent can be reopened where it left off."),
        n => format!("Nothing runs in {place}. Its {n} closed agents can be reopened where they left off."),
    }
}

pub(super) fn new_shell_args(workspace: Option<u64>) -> Vec<String> {
    match workspace {
        Some(id) => cmd(&["new-tab", "--workspace", &id.to_string(), "--focus"]),
        None => cmd(&["new-tab", "--focus"]),
    }
}

fn reopen_label(closed: usize) -> String {
    if closed == 1 { "Reopen the agent".to_string() } else { format!("Reopen {closed} agents") }
}

fn page() -> Div {
    div().absolute().inset_0().flex().flex_col().items_center().justify_center().gap(px(10.)).p(px(24.))
}

fn heading(text: &str) -> Div {
    div().text_size(px(17.)).font_weight(FontWeight::SEMIBOLD).text_center().child(text.to_string())
}

fn lead_text(text: String) -> Div {
    div().max_w(px(540.)).text_center().text_color(colours::muted()).child(text)
}

fn tall(button: Stateful<Div>) -> Stateful<Div> {
    button.h(px(32.)).px(px(16.)).text_size(px(13.))
}

fn row() -> Div {
    div().flex().flex_wrap().justify_center().gap(px(10.)).pt(px(6.))
}

fn welcome(cx: &mut Context<Root>) -> Div {
    let open = tall(ui::icon_text_button("no-tab-open", Icon::FolderOpen, "Open a project…", Tone::Primary))
        .tooltip(tip(with_keys("Open a project", Command::OpenProject)))
        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_project(window, cx)));
    page()
        .child(ui::brand::mark(5., false).mb(px(12.)))
        .child(ui::brand::wordmark(22.))
        .child(div().text_color(colours::muted()).child(TAGLINE))
        .child(row().child(open))
}

fn reopen_button(no_tab: &NoTab, workspace: u64, busy: bool, cx: &mut Context<Root>) -> Stateful<Div> {
    if busy {
        return tall(ui::busy_button("no-tab-reopen", "Reopening…", Tone::Primary));
    }
    let (job, args) = reopening(workspace, &no_tab.name);
    tall(ui::icon_text_button("no-tab-reopen", Icon::Undo, reopen_label(no_tab.closed), Tone::Primary)).on_click(
        cx.listener(move |this, _: &ClickEvent, _, cx| {
            let (job, args) = (job.clone(), args.clone());
            this.store.update(cx, |s, cx| s.run_aside(job, args, cx));
        }),
    )
}

fn start_buttons(no_tab: &NoTab, cx: &mut Context<Root>) -> [Stateful<Div>; 2] {
    let tone = if no_tab.closed > 0 { Tone::Plain } else { Tone::Primary };
    let start = tall(ui::icon_text_button("no-tab-agent", Icon::Agent, "Start an agent…", tone))
        .tooltip(tip(with_keys("Start an agent", Command::StartAgent)))
        .on_click(cx.listener(|_, _: &ClickEvent, window, cx| dialogs::start_agent(window, cx)));
    let shell_args = new_shell_args(no_tab.workspace);
    let shell = tall(ui::icon_text_button("no-tab-shell", Icon::Terminal, "New shell", Tone::Plain))
        .tooltip(tip(with_keys("New shell tab", Command::NewTab)))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, _| this.act(shell_args.clone())));
    [start, shell]
}

fn actions(no_tab: &NoTab, reopen_busy: bool, cx: &mut Context<Root>) -> Div {
    if no_tab.removing {
        return row().child(ui::busy("no-tab-removing", format!("Removing {}…", no_tab.name)));
    }
    let reopen = no_tab.workspace.filter(|_| no_tab.closed > 0);
    row().children(reopen.map(|ws| reopen_button(no_tab, ws, reopen_busy, cx))).children(start_buttons(no_tab, cx))
}

fn no_tab_page(no_tab: &NoTab, reopen_busy: bool, cx: &mut Context<Root>) -> Div {
    page()
        .child(ui::emblem(Icon::Terminal))
        .child(heading("No tab open"))
        .child(lead_text(lead(no_tab)))
        .child(actions(no_tab, reopen_busy, cx))
}

impl Root {
    pub(super) fn shows_no_tab(&self) -> bool {
        let frame = self.has_frame.then_some(self.grid.projection).flatten();
        self.session.as_ref().is_some_and(|s| s.shows_no_tab(frame))
    }

    pub(crate) fn frame_shown(&self) -> bool {
        self.has_frame && !self.shows_no_tab()
    }

    pub(crate) fn drop_drags_without_tab(&mut self) {
        if !self.shows_no_tab() {
            return;
        }
        if let Some(Drag::Program { pane, cell, .. }) = self.drag.take() {
            self.queue.input(pane, mouse("up", "left", cell));
        }
        self.moves.replace(Moves::default());
    }

    pub(super) fn empty_page(&self, cx: &mut Context<Self>) -> Option<Div> {
        let session = self.session.as_ref()?;
        let store = self.store.read(cx);
        let empty = empty_of(session, |ws| store.removing(ws));
        Some(match empty {
            Empty::Welcome => welcome(cx),
            Empty::NoTab(no_tab) => {
                let reopen_busy = no_tab.workspace.is_some_and(|ws| store.busy(reopening(ws, "").0.key()));
                no_tab_page(&no_tab, reopen_busy, cx)
            }
        })
    }
}

#[cfg(test)]
mod tests;
