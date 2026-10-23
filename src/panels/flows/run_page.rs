use gpui::{
    App, ClickEvent, ClipboardItem, Context, Div, FontWeight, InteractiveElement as _, ParentElement as _, Rgba,
    Stateful, StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px,
};

use super::actions::{Question, ask_on_click};
use super::model::{self, Edge, FlowSpec, Member, Message, Run, RunFile};
use super::widgets::{error_small, open_path, role_label, show_pane, tag_owned, text_box};
use super::{FlowsPanel, diagram};
use crate::store::{Store, args};
use crate::theme::colours;
use crate::ui;

impl FlowsPanel {
    pub(super) fn run_page(&self, run: &Run, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(run_title(run, cx))
            .child(ui::dim(run_facts(run, cx)))
            .child(text_box().mt(px(8.)).line_clamp(8).child(run.task.clone()))
            .child(ui::section("Team"))
            .child(team(run))
            .child(ui::section("Plan"))
            .child(diagram::diagram(&run.flow, Some(run)))
            .child(ui::section(format!("Messages ({})", self.feed.messages.len())))
            .child(self.feed_list(run, cx))
            .child(ui::section("Files"))
            .child(self.files_list(run))
    }

    fn feed_list(&self, run: &Run, cx: &Context<Self>) -> Div {
        let feed = div().flex().flex_col().gap(px(4.));
        if run.closed() {
            return feed
                .child(ui::dim("Messages are kept while the run is live; reopen the agents to follow it again."));
        }
        let now = model::now();
        feed.when(self.feed.messages.is_empty(), |d| {
            d.child(ui::dim(
                "Nothing sent yet. The orchestrator's sends, keys, reads and waits appear here as they happen.",
            ))
        })
        .children(self.feed.messages.iter().rev().map(|message| self.message_card(run, message, now, cx)))
        .when_some(self.feed.ended.as_ref(), |d, ended| d.child(ui::dim(format!("Not following: {ended}"))))
    }

    fn message_card(&self, run: &Run, message: &Message, now: i64, cx: &Context<Self>) -> Stateful<Div> {
        let seq = message.seq;
        let whole = self.expanded.contains(&seq);
        let text = message.text.clone().unwrap_or_default();
        let shown: String = if whole { text.clone() } else { text.chars().take(1200).collect() };
        let cut = u64::try_from(text.len()).is_ok_and(|len| message.bytes > len);
        div()
            .id(("message", seq))
            .p(px(8.))
            .rounded(px(6.))
            .bg(colours::terminal())
            .border_1()
            .border_color(colours::border())
            .cursor_pointer()
            .hover(|s| s.border_color(colours::muted()))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_message(seq, cx)))
            .child(message_header(&run.flow, message, text, now))
            .when(!shown.is_empty(), |d| d.child(message_body(shown, whole)))
            .when(cut && whole, |d| d.child(ui::dim(format!("cut at 32 KiB of {}", model::size(message.bytes)))))
    }

    fn toggle_message(&mut self, seq: u64, cx: &mut Context<Self>) {
        if !self.expanded.remove(&seq) {
            self.expanded.insert(seq);
        }
        cx.notify();
    }

    fn files_list(&self, run: &Run) -> Div {
        let list = div().flex().flex_col().gap(px(1.)).children(run.dir.clone().map(folder_line));
        match &self.files {
            None => list.child(ui::dim("Reading the folder…")),
            Some(Err(e)) => list.child(error_small(e.clone())),
            Some(Ok(files)) if files.entries.is_empty() => {
                list.child(ui::dim("No files yet: the roles write their answers here."))
            }
            Some(Ok(files)) => {
                let now = model::now();
                list.children(files.entries.iter().enumerate().map(|(i, file)| file_row(i, file, now)))
                    .when(files.more, |d| d.child(ui::dim("Only the newest 200 files are listed.")))
            }
        }
    }
}

fn run_title(run: &Run, cx: &Context<FlowsPanel>) -> Div {
    let (state, colour) = run_status(run);
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .child(ui::title(run.flow.name.clone()).text_size(px(16.)))
        .child(div().text_size(px(12.)).text_color(colour).child(state))
        .child(div().flex_1())
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

fn run_facts(run: &Run, cx: &App) -> String {
    let session = Store::get(cx).read(cx).session.clone();
    let place = session.as_ref().and_then(|s| s.workspace(run.workspace)).map(|(p, w)| {
        let branch = w.branch.as_ref().filter(|b| **b != w.name).map(|b| format!(" · {b}")).unwrap_or_default();
        format!("{} › {}{branch}", p.name, w.name)
    });
    format!(
        "{} · {} · {} message{}",
        place.unwrap_or_else(|| format!("workspace {}", run.workspace)),
        run.summary(),
        run.messages,
        if run.messages == 1 { "" } else { "s" }
    )
}

fn run_buttons(run: &Run, cx: &Context<FlowsPanel>) -> Div {
    if run.closed() {
        return closed_run_buttons(run.workspace, cx);
    }
    let buttons = div().flex().gap(px(6.));
    let Some(pane) = run.lead_pane() else { return buttons };
    let stop = Question::new(
        "Stop this run?",
        "The orchestrator and its whole team close, and the run ends. Nothing is removed from disk: the worktree and its branch stay.",
        "Stop",
        args(&["close", "--agent", "--pane", &pane.to_string()]),
    );
    buttons
        .child(ui::button("run-show", "Show orchestrator").on_click(move |_: &ClickEvent, _, cx| show_pane(pane, cx)))
        .child(ui::danger_button("run-stop", "Stop").on_click(ask_on_click(stop, cx)))
}

fn closed_run_buttons(workspace: u64, cx: &Context<FlowsPanel>) -> Div {
    let workspace = workspace.to_string();
    let reopen = args(&["agents", "reopen", "--workspace", &workspace]);
    let forget = Question::new(
        "Forget this run?",
        "Its closed agents are dropped for good and the run ends. The worktree and its branch stay.",
        "Forget",
        args(&["agents", "forget", "--workspace", &workspace, "--yes"]),
    );
    div()
        .flex()
        .gap(px(6.))
        .child(ui::primary_button("run-reopen", "Reopen agents").on_click(
            cx.listener(move |_, _: &ClickEvent, _, cx| FlowsPanel::run_cmd(reopen.clone(), "Reopening the team", cx)),
        ))
        .child(ui::danger_button("run-forget", "Forget").on_click(ask_on_click(forget, cx)))
}

fn team(run: &Run) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(1.))
        .children(run.roles.iter().enumerate().map(|(i, member)| member_row(run, i, member)))
}

fn member_row(run: &Run, i: usize, member: &Member) -> Stateful<Div> {
    let pane = member.pane.filter(|_| !member.closed);
    let agent = run.flow.role(&member.role).map(model::RoleSpec::agent_facts).unwrap_or_default().join(" · ");
    let status =
        if member.closed { "closed".to_string() } else { member.status.clone().unwrap_or_else(|| "—".into()) };
    let row = ui::row(("member", i), false)
        .when(member.closed, |d| d.opacity(0.55))
        .child(ui::badge(if member.closed { None } else { member.status.as_deref() }))
        .child(role_label(diagram::role_colour(&run.flow, &member.role), member.role.clone()))
        .child(div().w(px(200.)).flex_none().truncate().text_size(px(12.)).text_color(colours::muted()).child(agent))
        .child(div().w(px(80.)).flex_none().text_size(px(12.)).child(status))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .gap(px(6.))
                .children(member.awaited.as_ref().map(|a| tag_owned(format!("work {a}"))))
                .children(member.branch.as_ref().map(|b| ui::dim(format!("⎇ {b}")).truncate())),
        )
        .child(ui::dim(pane.map(|p| format!("pane {p}")).unwrap_or_default()).flex_none());
    match pane {
        Some(pane) => row.on_click(move |_: &ClickEvent, _, cx| show_pane(pane, cx)),
        None => row.cursor_default(),
    }
}

fn edge_label(flow: &FlowSpec, edge: Option<Edge>) -> Option<String> {
    match edge? {
        Edge::Forward { stage } => Some(format!("stage {} → {}", stage + 1, stage + 2)),
        Edge::Loop { stage } => flow.stages.get(stage)?.back.as_ref().map(|b| format!("↺ {}", b.when)),
        Edge::Outside { .. } => Some("outside the plan".into()),
    }
}

fn role_name(flow: &FlowSpec, role: &str) -> Div {
    div().text_color(diagram::role_colour(flow, role)).font_weight(FontWeight::SEMIBOLD).child(role.to_string())
}

fn message_header(flow: &FlowSpec, message: &Message, text: String, now: i64) -> Div {
    let seq = message.seq;
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .text_size(px(12.))
        .child(role_name(flow, &message.from))
        .child(div().text_color(colours::faint()).child("→"))
        .child(role_name(flow, &message.to))
        .child(div().text_color(colours::muted()).child(message.verb().to_string()))
        .children(edge_label(flow, message.edge).map(tag_owned))
        .child(div().flex_1())
        .when(!text.is_empty(), |d| d.child(copy_button(seq, text)))
        .child(ui::dim(format!("#{seq} · {}", model::ago(&message.time, now))))
}

fn copy_button(seq: u64, text: String) -> Stateful<Div> {
    ui::icon_button(("copy-message", seq), "⧉").text_size(px(12.)).on_click(move |_: &ClickEvent, _, cx| {
        cx.stop_propagation();
        cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
    })
}

fn message_body(shown: String, whole: bool) -> Div {
    div().pt(px(4.)).text_size(px(12.)).text_color(colours::text()).when(!whole, |d| d.line_clamp(4)).child(shown)
}

fn folder_line(dir: String) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .pb(px(4.))
        .child(ui::dim(dir.clone()).truncate())
        .child(ui::button("run-folder", "Open folder").on_click(open_path(dir)))
}

fn file_row(i: usize, file: &RunFile, now: i64) -> Stateful<Div> {
    ui::row(("run-file", i), false)
        .on_click(open_path(file.path.clone()))
        .child(div().flex_1().truncate().text_size(px(12.)).child(file.name.clone()))
        .child(ui::dim(model::size(file.bytes)).w(px(70.)).flex_none())
        .child(ui::dim(file.modified.as_deref().map(|m| model::ago(m, now)).unwrap_or_default()).w(px(40.)).flex_none())
}
