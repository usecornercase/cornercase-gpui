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
use super::model::{Member, RoleSpec, Run, plural};
use super::widgets::{panel_box, pill, show_pane};
use crate::store::{Store, args};
use crate::theme::colours;
use crate::ui::{self, Icon, Tone, glass::tint};

impl FlowsPanel {
    pub(super) fn run_top(&self, run: &Run, cx: &Context<Self>) -> Div {
        let key = format!("run-{}", run.id);
        let plan = Plan { flow: &run.flow, run: Some(run), focus: None, key: &key };
        div()
            .flex()
            .flex_col()
            .gap(px(22.))
            .child(run_hero(run, cx))
            .child(task(&run.task))
            .child(block(Icon::Accounts, "Team").child(team(run)))
            .child(block(Icon::Flows, "Plan").child(self.diagram(plan, cx)))
    }
}

fn run_hero(run: &Run, cx: &Context<FlowsPanel>) -> Div {
    let (state, colour) = run_status(run);
    let live = !run.closed() && !run.ended;
    let heading = title(&run.flow, Some(run), run.flow.name.clone().into(), facts(run_facts(run, cx)));
    hero(&run.flow)
        .child(heading.child(pill(state, colour, live)))
        .child(
            div()
                .relative()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(stages_bar(run).flex_1())
                .child(div().flex_none().text_size(px(11.)).text_color(colours::muted()).child(run.summary())),
        )
        .child(run_buttons(run, cx))
}

fn run_status(run: &Run) -> (&'static str, Rgba) {
    if run.closed() {
        ("agents closed", colours::faint())
    } else if run.ended {
        ("reached its end", colours::accent())
    } else {
        ("running", colours::working())
    }
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

fn run_buttons(run: &Run, cx: &Context<FlowsPanel>) -> Div {
    let buttons = div().relative().flex().items_center().gap(px(8.));
    if run.closed() {
        return closed_run_buttons(buttons, run.workspace, cx);
    }
    let Some(pane) = run.lead_pane() else { return buttons };
    let command = args(&["close", "--agent", "--pane", &pane.to_string()]);
    let stop = if running(&command, cx) {
        ui::busy_button("run-stop", "Stopping…", Tone::Danger)
    } else {
        let question = Question::new(
            "Stop this run?",
            "The orchestrator and its whole team close, and the run ends. Nothing is removed from disk: the worktree and its branch stay.",
            "Stop",
            command,
        )
        .doing("Stopping the run…");
        ui::icon_text_button("run-stop", Icon::Stop, "Stop", Tone::Danger).on_click(ask_on_click(question, cx))
    };
    buttons
        .child(
            ui::icon_text_button("run-show", Icon::Terminal, "Show orchestrator", Tone::Plain)
                .on_click(move |_: &ClickEvent, _, cx| show_pane(pane, cx)),
        )
        .child(stop)
}

fn reopen_button(workspace: &str, cx: &Context<FlowsPanel>) -> Stateful<Div> {
    let reopen = args(&["agents", "reopen", "--workspace", workspace]);
    if running(&reopen, cx) {
        return ui::busy_button("run-reopen", "Reopening…", Tone::Primary);
    }
    ui::icon_text_button("run-reopen", Icon::Undo, "Reopen agents", Tone::Primary).on_click(cx.listener(
        move |_, _: &ClickEvent, _, cx| {
            FlowsPanel::run_cmd(reopen.clone(), "Reopening the team…", "Reopened the team", cx);
        },
    ))
}

fn closed_run_buttons(buttons: Div, workspace: u64, cx: &Context<FlowsPanel>) -> Div {
    let workspace = workspace.to_string();
    let forget = Question::new(
        "Forget this run?",
        "Its closed agents are dropped for good and the run ends. The worktree and its branch stay.",
        "Forget",
        args(&["agents", "forget", "--workspace", &workspace, "--yes"]),
    )
    .doing("Forgetting the run…");
    buttons.child(reopen_button(&workspace, cx)).child(
        ui::icon_text_button("run-forget", Icon::Trash, "Forget", Tone::Danger).on_click(ask_on_click(forget, cx)),
    )
}

fn team(run: &Run) -> Div {
    div()
        .grid()
        .grid_cols(2)
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
