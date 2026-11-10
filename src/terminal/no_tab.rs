use gpui::{
    ClickEvent, Context, Div, FontWeight, InteractiveElement as _, ParentElement as _, Stateful,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use super::mouse::{button_name, mouse};
use crate::app::{Drag, Root};
use crate::cli::{self, Said};
use crate::dialogs::{self, Question};
use crate::keymap::{Command, with_keys};
use crate::model::Session;
use crate::render::moving::Moves;
use crate::sidebar::{cmd, reopening, tip};
use crate::store::{self, Busy};
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

pub(super) fn column() -> Div {
    div().flex().flex_col().items_center().justify_center().gap(px(10.)).p(px(24.))
}

pub(super) fn page() -> Div {
    column().absolute().inset_0()
}

pub(super) fn scrolled(id: &'static str, content: Div) -> Stateful<Div> {
    div().id(id).absolute().inset_0().overflow_y_scroll().child(content.min_h_full())
}

pub(super) fn heading(text: &str) -> Div {
    div().text_size(px(17.)).font_weight(FontWeight::SEMIBOLD).text_center().child(text.to_string())
}

pub(super) fn lead_text(text: String) -> Div {
    div().max_w(px(540.)).text_center().text_color(colours::muted()).child(text)
}

pub(super) fn tall(button: Stateful<Div>) -> Stateful<Div> {
    button.h(px(32.)).px(px(16.)).text_size(px(13.))
}

pub(super) fn row() -> Div {
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

pub(super) fn answer_lines(said: &Said) -> (Vec<String>, bool) {
    let (text, failed) = match said {
        Said::Done(text) => (text, false),
        Said::Failed(text) => (text, true),
    };
    (text.lines().map(str::to_string).collect(), failed)
}

fn answer_line(said: Option<&Said>) -> Option<Div> {
    let (lines, failed) = answer_lines(said?);
    let colour = if failed { colours::failed() } else { colours::muted() };
    let lines = lines.into_iter().map(|line| div().child(line));
    Some(div().max_w(px(540.)).text_center().text_size(px(12.)).text_color(colour).children(lines))
}

fn answer_kept<'a>(answered: Option<&'a (String, Said)>, key: &str) -> Option<&'a Said> {
    answered.filter(|(kept, _)| kept == key).map(|(_, said)| said)
}

pub(super) fn answer_under(said: Option<&Said>) -> Div {
    div().children(answer_line(said))
}

pub(super) fn retry_button(id: &'static str, cx: &mut Context<Root>) -> Stateful<Div> {
    tall(ui::icon_text_button(id, Icon::Refresh, "Try again", Tone::Plain))
        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.start_again(cx)))
}

impl Root {
    pub(super) fn start_again(&mut self, cx: &mut Context<Self>) {
        self.start(cx);
        cx.notify();
    }

    pub(super) fn answer_for(&self, key: &str) -> Option<&Said> {
        answer_kept(self.answered.as_ref(), key)
    }

    pub(super) fn ask_then(
        (title, message, button): (String, String, &'static str),
        then: impl Fn(&mut Root, &mut Context<Root>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let root = cx.entity();
        let question = Question::new(title, message, button, Vec::new())
            .on_yes(move |cx| root.update(cx, |this, cx| then(this, cx)));
        dialogs::confirm(question, window, cx);
    }

    pub(super) fn run_answered(
        &mut self,
        busy: Busy,
        work: impl FnOnce() -> anyhow::Result<cli::Answer> + Send + 'static,
        then: impl FnOnce(&mut Root, bool, &mut Context<Root>) + 'static,
        cx: &mut Context<Self>,
    ) {
        self.answered = None;
        cx.notify();
        let key = busy.key().to_string();
        store::run_local_busy(cx, busy, work, move |this, result, cx| {
            let said = cli::said_of(result);
            let done = matches!(said, Said::Done(_));
            this.answered = Some((key, said));
            then(this, done, cx);
        });
    }

    pub(super) fn shows_no_tab(&self) -> bool {
        let shown = self.shown();
        let frame = shown.has_frame.then_some(shown.grid.projection).flatten();
        self.shown_session().is_some_and(|s| s.shows_no_tab(frame))
    }

    pub(crate) fn frame_shown(&self) -> bool {
        self.shown().has_frame && !self.shows_no_tab()
    }

    pub(crate) fn drop_drags_without_tab(&mut self) {
        if self.shows_no_tab() {
            self.drop_drags();
        }
    }

    pub(crate) fn drop_drags(&mut self) {
        if let Some(Drag::Program { pane, button, cell, .. }) = self.drag.take() {
            self.shown().queue.input(pane, mouse("up", button_name(button), cell));
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
