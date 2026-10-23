use std::rc::Rc;

use gpui::{
    AnyElement, ClickEvent, Context, Div, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement,
    Rgba, SharedString, Stateful, StatefulInteractiveElement, Styled, div, prelude::FluentBuilder as _, px,
};

use super::rows::{Listing, chevron, drop_target, hover_button, name_label, row_actions, row_base};
use super::{Target, cmd, draggable, forms, tip};
use crate::app::Root;
use crate::model::{AgentRow, PaneInfo, PullRequest, Shown, Workspace};
use crate::theme::colours;
use crate::ui::{self, badge};

pub(crate) fn pull_tag(pr: &PullRequest) -> (String, Rgba, bool) {
    let number = format!("#{}", pr.number);
    match pr.state.as_str() {
        "merged" => (number, forms::palette_colour(99), false),
        "closed" => (number, colours::faint(), true),
        _ => match pr.checks.as_deref() {
            Some("failing") => (format!("{number}✗"), colours::failed(), false),
            Some("pending") => (format!("{number}•"), colours::working(), false),
            Some("passing") => (format!("{number}✓"), colours::done(), false),
            _ => (number, colours::muted(), false),
        },
    }
}

fn pull_tip(pr: &PullRequest) -> String {
    let state = if pr.conflicts { format!("{}, conflicts", pr.state) } else { pr.state.clone() };
    let mut text = format!("#{} {}\n{state}", pr.number, pr.title);
    if let Some(checks) = &pr.checks {
        text.push_str(" · checks ");
        text.push_str(checks);
    }
    if !pr.failed.is_empty() {
        text.push_str("\nfailed: ");
        text.push_str(&pr.failed.join(", "));
    }
    text
}

fn agent_title(pane: &PaneInfo) -> Option<String> {
    let title = pane.title.as_deref()?.trim_start_matches(|c: char| !c.is_alphanumeric()).trim();
    (!title.is_empty()).then(|| title.to_string())
}

pub(super) fn plural(n: usize, one: &str) -> String {
    format!("{n} {one}{}", if n == 1 { "" } else { "s" })
}

fn workspace_detail(ws: &Workspace, collapsed: bool) -> Vec<AnyElement> {
    let mut detail: Vec<AnyElement> = Vec::new();
    if let Some(branch) = ws.branch.as_deref().filter(|b| *b != ws.name) {
        detail.push(div().overflow_hidden().text_ellipsis().child(branch.to_string()).into_any_element());
    }
    if let Some(ch) = ws.changes.as_ref().filter(|c| c.files > 0) {
        detail.push(
            div()
                .flex()
                .gap(px(3.))
                .child(div().text_color(colours::done()).child(format!("+{}", ch.added)))
                .child(div().text_color(colours::failed()).child(format!("−{}", ch.removed)))
                .into_any_element(),
        );
    }
    let agents = ws.tabs.iter().filter(|t| t.agent_pane().is_some()).count();
    detail.push(div().child(plural(ws.tabs.len(), "tab")).into_any_element());
    if agents > 0 && collapsed {
        detail.push(div().child(plural(agents, "agent")).into_any_element());
    }
    if let Some(n) = ws.closed_agents.filter(|n| *n > 0) {
        detail.push(div().child(format!("{n} closed")).into_any_element());
    }
    detail
}

fn pull_request_tag(ws: &Workspace) -> Option<Stateful<Div>> {
    let pr = ws.pull_request.as_ref()?;
    let (text, colour, crossed) = pull_tag(pr);
    let url = pr.url.clone();
    Some(
        div()
            .id(SharedString::from(format!("ws-pr-{}", ws.id)))
            .flex_none()
            .text_size(px(11.))
            .text_color(colour)
            .when(crossed, Styled::line_through)
            .cursor_pointer()
            .hover(Styled::underline)
            .child(text)
            .tooltip(tip(pull_tip(pr)))
            .on_click(move |_: &ClickEvent, _, cx| {
                cx.stop_propagation();
                cx.open_url(&url);
            }),
    )
}

fn detail_line(ws: &Workspace, collapsed: bool) -> Div {
    let setup = ws.setup.as_deref().map(|s| match s {
        "running" => ("setup…", colours::working()),
        _ => ("setup failed", colours::failed()),
    });
    div()
        .flex()
        .gap(px(6.))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_size(px(11.))
        .text_color(colours::faint())
        .children(setup.map(|(text, colour)| div().text_color(colour).child(text)))
        .children(workspace_detail(ws, collapsed))
}

struct WorkspaceLook {
    selected: bool,
    collapsed: bool,
    has_agents: bool,
    label: String,
}

fn selection_bar() -> Div {
    div().absolute().left_0().top(px(4.)).bottom(px(4.)).w(px(2.)).rounded_full().bg(colours::accent())
}

fn workspace_fold(id: u64, look: &WorkspaceLook, cx: &mut Context<Root>) -> Stateful<Div> {
    let fold_id = SharedString::from(format!("ws-fold-{id}"));
    if !look.has_agents {
        return div().id(fold_id).w(px(14.)).flex_none();
    }
    let collapsed = look.collapsed;
    chevron(fold_id, !collapsed).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
        cx.stop_propagation();
        this.set_folded(Target::Workspace(id), !collapsed);
    }))
}

impl Root {
    pub(super) fn render_workspace(
        &self,
        ws: &Workspace,
        listing: &Listing,
        siblings: Rc<Vec<u64>>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = ws.id;
        let target = Target::Workspace(id);
        let rows = ws.agent_rows();
        let look = WorkspaceLook {
            selected: listing.shown.workspace == Some(id),
            collapsed: ws.collapsed && !listing.filtering(),
            has_agents: !rows.is_empty(),
            label: if ws.worktree { format!("⎇ {}", ws.name) } else { ws.name.clone() },
        };
        let row = row_base(target)
            .gap(px(4.))
            .ml(px(6.))
            .pl(px(4.))
            .pr(px(4.))
            .py(px(3.))
            .when(look.selected, |d| d.bg(colours::selected()).child(selection_bar()))
            .child(workspace_fold(id, &look, cx))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .child(self.title_line(ws, &look))
                    .child(detail_line(ws, look.collapsed)),
            );
        let row = row_actions(row, target, move |this| this.focus_item("workspace", id), cx);
        let row = draggable(row, target, look.label.clone());
        let row = drop_target(row, target, siblings, cx);
        let agents = if look.collapsed { Vec::new() } else { rows };
        let mut col = div().flex().flex_col().child(row);
        for agent in &agents {
            col = col.child(self.render_agent(agent, listing.shown, cx));
        }
        col.into_any_element()
    }

    fn title_line(&self, ws: &Workspace, look: &WorkspaceLook) -> Div {
        let name = name_label()
            .min_w(px(0.))
            .text_color(if look.selected { colours::text() } else { colours::muted() })
            .child(look.label.clone());
        div()
            .flex()
            .items_center()
            .gap(px(5.))
            .child(self.name_or_rename(Target::Workspace(ws.id), name))
            .children(pull_request_tag(ws))
            .when(ws.behind > 0, |d| {
                d.child(
                    div().flex_none().text_size(px(11.)).text_color(colours::muted()).child(format!("↓{}", ws.behind)),
                )
            })
            .when(!look.has_agents || look.collapsed, |d| d.child(badge(ws.loudest())))
    }

    fn agent_tab_name(&self, tab: u64) -> String {
        self.session
            .as_ref()
            .and_then(|s| s.projects.iter().flat_map(|p| &p.workspaces).flat_map(|w| &w.tabs).find(|t| t.id == tab))
            .map(|t| t.name.clone())
            .unwrap_or_default()
    }

    fn render_agent(&self, row: &AgentRow<'_>, shown: Shown, cx: &mut Context<Self>) -> AnyElement {
        let pane = row.pane;
        let id = pane.id;
        let selected = shown.pane == Some(id);
        let title = agent_title(pane).unwrap_or_else(|| self.agent_tab_name(row.tab));
        let gid = SharedString::from(format!("agent-{id}"));
        div()
            .id(gid.clone())
            .group(gid.clone())
            .relative()
            .flex()
            .items_center()
            .gap(px(4.))
            .ml(px(22. + 12. * f32::from(u16::try_from(row.depth).unwrap_or(u16::MAX))))
            .px(px(4.))
            .py(px(2.))
            .rounded(px(5.))
            .cursor_pointer()
            .when(selected, |d| d.bg(colours::selected()))
            .hover(|s| s.bg(colours::hover()))
            .child(agent_fold(row, cx))
            .child(badge(pane.status.as_deref()))
            .child(agent_text(title, agent_details(pane), selected))
            .when(pane.collapsed && row.children > 0, |d| d.child(ui::dim(format!("+{}", row.children))))
            .child(hover_button(SharedString::from(format!("agent-menu-{id}")), gid, "⋯", 0).on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.open_agent_menu(id, e.position, window, cx);
                }),
            ))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    this.open_agent_menu(id, e.position, window, cx);
                }),
            )
            .on_click(cx.listener(move |this, _: &ClickEvent, _, _| this.focus_item("pane", id)))
            .into_any_element()
    }
}

fn agent_details(pane: &PaneInfo) -> String {
    let mut details = vec![pane.agent.clone().unwrap_or_default()];
    details.extend(pane.role.clone());
    details.extend(pane.model.clone());
    details.extend(pane.context.map(|c| format!("{c}%")));
    if pane.subagents > 0 {
        details.push(plural(pane.subagents as usize, "subagent"));
    }
    if pane.resuming {
        details.push("resuming".into());
    }
    details.join(" · ")
}

fn agent_fold(row: &AgentRow<'_>, cx: &mut Context<Root>) -> Stateful<Div> {
    let id = row.pane.id;
    let fold_id = SharedString::from(format!("agent-fold-{id}"));
    if row.children == 0 {
        return div().id(fold_id).w(px(10.)).flex_none();
    }
    let folded = row.pane.collapsed;
    chevron(fold_id, !folded).w(px(10.)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
        cx.stop_propagation();
        let verb = if folded { "expand" } else { "collapse" };
        this.queue.cmd(cmd(&[verb, "--pane", &id.to_string()]));
    }))
}

fn agent_text(title: String, details: String, selected: bool) -> Div {
    let line = || div().whitespace_nowrap().overflow_hidden().text_ellipsis();
    div()
        .flex_1()
        .min_w(px(0.))
        .flex()
        .flex_col()
        .child(
            line()
                .text_size(px(12.))
                .text_color(if selected { colours::text() } else { colours::muted() })
                .child(title),
        )
        .child(line().text_size(px(10.5)).text_color(colours::faint()).child(details))
}

#[cfg(test)]
mod tests;
