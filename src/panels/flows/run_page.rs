use gpui::{
    App, ClickEvent, Context, Div, FontWeight, InteractiveElement as _, ParentElement as _, Rgba, Stateful,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px,
};

use super::FlowsPanel;
use super::actions::{Question, ask_on_click, running};
use super::cards::{avatar, conductor, status_word, with_status};
use super::diagram::{Plan, role_colour};
use super::flow_page::block;
use super::hero::{facts, hero, stages_bar, task, title};
use super::merged_view;
use super::model::{Member, RoleSpec, Run, Standing, plural};
use super::widgets::{loop_words, panel_box, pill, show_pane, standing_colour};
use crate::model::Reopen;
use crate::store::{Store, args, reopen_key};
use crate::theme::colours;
use crate::ui::{self, Icon, Tone, glass::tint};

const TWO_COLUMNS: f32 = 480.;

impl FlowsPanel {
    pub(super) fn run_top(&self, run: &Run, cx: &Context<Self>) -> Div {
        let key = format!("run-{}", run.id);
        let plan = Plan { flow: &run.flow, run: Some(run), focus: None, key: &key, on_role: None };
        div()
            .flex()
            .flex_col()
            .gap(px(22.))
            .child(run_hero(run, cx))
            .child(task(&run.task))
            .child(block(Icon::Accounts, "Team").child(team(run, team_columns(self.board.room()))))
            .child(self.diagram(plan, cx))
    }
}

fn run_hero(run: &Run, cx: &Context<FlowsPanel>) -> Div {
    let standing = run.standing();
    let (state, colour) = (hero_word(standing), standing_colour(standing));
    let live = glows(standing);
    let offer = offer(run, cx);
    let closed_first = offer.as_ref().map_or(0, |(reopen, _)| reopen.closed_first());
    let heading = title(&run.flow, Some(run), run.flow.name.clone().into(), facts(run_facts(run, cx)));
    hero(&run.flow)
        .child(heading.child(pill(state, colour, live)))
        .child(
            div().relative().flex().items_center().gap(px(12.)).child(stages_bar(run).flex_1()).child(run_summary(run)),
        )
        .children(run_notes(run, standing, closed_first).into_iter().map(note))
        .child(run_buttons(run, offer, cx))
        .children(merged_view::run_switch(run, cx))
}

fn run_summary(run: &Run) -> Div {
    div()
        .min_w_0()
        .flex()
        .items_center()
        .text_size(px(11.))
        .text_color(colours::muted())
        .whitespace_nowrap()
        .child(run.position())
        .children(run.looped().into_iter().map(|(stage, back, rounds)| {
            div().min_w_0().flex().items_center().child(" · ").child(loop_words(
                ("run-loop", stage),
                back,
                Some((Some(rounds), true)),
            ))
        }))
}

fn hero_word(standing: Standing) -> &'static str {
    match standing {
        Standing::Closed => "agents closed",
        Standing::Ended => "reached its end",
        Standing::Stopped(_) => "stopped moving",
        Standing::Running => "running",
    }
}

fn glows(standing: Standing) -> bool {
    matches!(standing, Standing::Running)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Note {
    Stopped,
    Owed,
    Exited,
}

fn run_notes(run: &Run, standing: Standing, closed_first: usize) -> Vec<(String, Note)> {
    let stopped = match standing {
        Standing::Stopped(secs) => Some((run.stopped_line(secs), Note::Stopped)),
        _ => None,
    };
    stopped
        .into_iter()
        .chain(run.not_run_line().map(|line| (line, Note::Owed)))
        .chain(run.exited_line(closed_first).map(|line| (line, Note::Exited)))
        .collect()
}

fn note_colour(kind: Note) -> Rgba {
    match kind {
        Note::Stopped => colours::muted(),
        Note::Owed | Note::Exited => colours::waiting(),
    }
}

fn note((text, kind): (String, Note)) -> Div {
    div().relative().text_size(px(12.)).text_color(note_colour(kind)).child(text)
}

fn run_facts(run: &Run, cx: &App) -> Vec<String> {
    let session = Store::get(cx).read(cx).session.clone();
    let place = session.as_ref().and_then(|s| s.workspace(run.workspace)).map(|(p, w)| {
        let branch = w.branch.as_ref().filter(|b| **b != w.name).map(|b| format!(" · {b}")).unwrap_or_default();
        format!("{} › {}{branch}", p.name, w.name)
    });
    vec![
        place.unwrap_or_else(|| format!("workspace {}", run.workspace)),
        format!("run {}", run.id),
        plural(usize::try_from(run.messages).unwrap_or(usize::MAX), "message"),
    ]
}

fn offer(run: &Run, cx: &App) -> Option<(Reopen, String)> {
    let store = Store::get(cx).read(cx);
    let (_, workspace) = store.session.as_ref()?.workspace(run.workspace)?;
    Some((workspace.reopen_for_run(run.closed())?, workspace.name.clone()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Action {
    Show(u64),
    Reopen(String),
    Stop(u64),
    Forget,
}

fn actions(run: &Run, reopen: Option<Reopen>) -> Vec<Action> {
    let reopen = reopen.map(|reopen| Action::Reopen(reopen.label()));
    if run.closed() {
        return reopen.into_iter().chain([Action::Forget]).collect();
    }
    run.lead_pane().map(Action::Show).into_iter().chain(reopen).chain(run.stop_pane().map(Action::Stop)).collect()
}

fn run_buttons(run: &Run, offer: Option<(Reopen, String)>, cx: &Context<FlowsPanel>) -> Div {
    let name = offer.as_ref().map(|(_, name)| name.clone()).unwrap_or_default();
    let buttons = div().relative().flex().flex_wrap().items_center().gap(px(8.));
    buttons.children(actions(run, offer.map(|(reopen, _)| reopen)).into_iter().map(|action| match action {
        Action::Show(pane) => show_button(pane),
        Action::Reopen(label) => reopen_button(run.workspace, label, name.clone(), cx),
        Action::Stop(pane) => stop_button(pane, cx),
        Action::Forget => forget_button(run.workspace, cx),
    }))
}

fn show_button(pane: u64) -> Stateful<Div> {
    ui::icon_text_button("run-show", Icon::Terminal, "Show orchestrator", Tone::Plain)
        .on_click(move |_: &ClickEvent, _, cx| show_pane(pane, cx))
}

fn stop_button(pane: u64, cx: &Context<FlowsPanel>) -> Stateful<Div> {
    let command = args(&["close", "--agent", "--pane", &pane.to_string()]);
    if running(&command, cx) {
        return ui::busy_button("run-stop", "Stopping…", Tone::Danger);
    }
    let question = Question::new(
        "Stop this run?",
        "The orchestrator and its whole team close, and the run ends. Nothing is removed from disk: the worktree and its branch stay.",
        "Stop",
        command,
    )
    .doing("Stopping the run…");
    ui::icon_text_button("run-stop", Icon::Stop, "Stop", Tone::Danger).on_click(ask_on_click(question, cx))
}

fn reopen_button(workspace: u64, label: String, name: String, cx: &Context<FlowsPanel>) -> Stateful<Div> {
    if Store::get(cx).read(cx).busy(&reopen_key(workspace)) {
        return ui::busy_button("run-reopen", "Reopening…", Tone::Primary);
    }
    ui::icon_text_button("run-reopen", Icon::Undo, label, Tone::Primary).on_click(move |_: &ClickEvent, _, cx| {
        Store::get(cx).update(cx, |store, cx| store.reopen_agents(workspace, &name, cx));
    })
}

fn forget_button(workspace: u64, cx: &Context<FlowsPanel>) -> Stateful<Div> {
    let forget = Question::new(
        "Forget this run?",
        "Its closed agents are dropped for good and the run ends. The worktree and its branch stay.",
        "Forget",
        args(&["agents", "forget", "--workspace", &workspace.to_string(), "--yes"]),
    )
    .doing("Forgetting the run…");
    ui::icon_text_button("run-forget", Icon::Trash, "Forget", Tone::Danger).on_click(ask_on_click(forget, cx))
}

fn team_columns(width: f32) -> u16 {
    if width >= TWO_COLUMNS { 2 } else { 1 }
}

fn team(run: &Run, columns: u16) -> Div {
    div()
        .grid()
        .grid_cols(columns)
        .gap(px(8.))
        .children(run.roles.iter().enumerate().map(|(i, member)| member_card(run, i, member)))
}

fn member_card(run: &Run, i: usize, member: &Member) -> Stateful<Div> {
    let colour = role_colour(&run.flow, &member.role);
    let face = if run.flow.is_lead(&member.role) { conductor(32.) } else { avatar(&member.role, colour, 32.) };
    let card = panel_box()
        .id(("member", i))
        .flex()
        .items_center()
        .gap(px(12.))
        .p(px(12.))
        .when(member.closed, |d| d.opacity(0.5))
        .when(member.agent_exited(), |d| d.border_dashed().border_color(tint(colours::waiting(), 0.6)))
        .child(with_status(face, Some(member)))
        .child(member_text(run, member))
        .child(member_side(member));
    match member.pane.filter(|_| !member.closed) {
        Some(pane) => card
            .cursor_pointer()
            .hover(move |s| s.border_color(tint(colour, 0.6)).bg(colours::hover()))
            .on_click(move |_: &ClickEvent, _, cx| show_pane(pane, cx)),
        None => card,
    }
}

fn member_text(run: &Run, member: &Member) -> Div {
    let agent = run.flow.role(&member.role).map(RoleSpec::agent_facts).unwrap_or_default().join(" · ");
    let small = |glyph, text: String| {
        div()
            .flex()
            .min_w_0()
            .items_center()
            .gap(px(4.))
            .text_size(px(11.))
            .text_color(colours::faint())
            .child(ui::icon(glyph, 11.).text_color(colours::faint()))
            .child(div().child(ui::one_line(text)))
    };
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(3.))
        .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child(ui::one_line(member.role.clone())))
        .child(div().text_size(px(11.)).text_color(colours::muted()).child(ui::one_line(agent)))
        .children(member.branch.clone().map(|branch| small(Icon::Branch, branch)))
        .children(member.awaited.clone().map(|awaited| small(Icon::Repeat, format!("work {awaited}"))))
}

fn member_side(member: &Member) -> Div {
    let state = status_word(Some(member));
    let pane = member.pane.filter(|_| !member.closed);
    div()
        .flex()
        .flex_none()
        .flex_col()
        .items_end()
        .gap(px(4.))
        .children(state.map(|(word, colour)| div().text_size(px(11.)).text_color(colour).child(word)))
        .children(pane.map(|pane| {
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .text_size(px(10.))
                .text_color(colours::faint())
                .child(ui::icon(Icon::Terminal, 11.).text_color(colours::faint()))
                .child(format!("pane {pane}"))
        }))
}

#[cfg(test)]
mod tests;
