use std::rc::Rc;

use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, Rgba, SharedString, Stateful, StatefulInteractiveElement, Styled, div, prelude::FluentBuilder as _,
    px,
};

use super::rows::{Listing, chevron, drop_target, hover_button, name_label, nested, row_actions, row_base};
use super::{Fold, Target, draggable, forms, tip};
use crate::app::Root;
use crate::dialogs;
use crate::model::{AgentRow, PaneInfo, PullRequest, Shown, Workspace};
use crate::theme::colours;
use crate::ui::{self, badge};

fn pull_tip(pr: &PullRequest) -> String {
    let mut text = format!("{} {}\n{}", pr.reference(), pr.title, pr.summary());
    if !pr.failed.is_empty() {
        text.push_str("\nfailed: ");
        text.push_str(&pr.failed.join(", "));
    }
    text
}

pub(super) fn plural(n: usize, one: &str) -> String {
    format!("{n} {one}{}", if n == 1 { "" } else { "s" })
}

fn workspace_detail(ws: &Workspace, look: &WorkspaceLook) -> Vec<AnyElement> {
    let mut detail: Vec<AnyElement> = Vec::new();
    if let Some(branch) = ws.branch.as_deref().filter(|b| *b != ws.name) {
        detail.push(div().child(ui::one_line(branch.to_string())).into_any_element());
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
    if look.collapsed || !look.lists_tabs {
        detail.push(div().child(plural(ws.tabs.len(), "tab")).into_any_element());
    }
    if agents > 0 && look.collapsed {
        detail.push(div().child(plural(agents, "agent")).into_any_element());
    }
    if let Some(n) = ws.closed_agents.filter(|n| *n > 0) {
        detail.push(div().child(format!("{n} closed")).into_any_element());
    }
    detail
}

fn pull_request_tags(ws: &Workspace) -> Option<Div> {
    let pr = ws.pull_request.as_ref()?;
    let id = ws.id;
    let tag = |key: &str, text: String, colour: Rgba| {
        div()
            .id(SharedString::from(format!("ws-{key}-{id}")))
            .flex_none()
            .text_color(colour)
            .cursor_pointer()
            .child(text)
            .tooltip(tip(pull_tip(pr)))
            .on_click(move |_: &ClickEvent, window, cx| {
                cx.stop_propagation();
                dialogs::pull_request(id, window, cx);
            })
    };
    let (text, colour, crossed) = dialogs::pull_tag(pr);
    let stack = dialogs::stack_tag(pr).map(|(text, colour)| tag("stack", text, colour));
    Some(
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(4.))
            .text_size(px(11.))
            .child(tag("pr", text, colour).when(crossed, Styled::line_through))
            .children(stack),
    )
}

fn detail_line(ws: &Workspace, look: &WorkspaceLook, removing: bool) -> Option<Div> {
    let setup = ws.setup.as_deref().map(|s| match s {
        "running" => ("setup…", colours::working()),
        _ => ("setup failed", colours::failed()),
    });
    let detail = workspace_detail(ws, look);
    if !removing && setup.is_none() && detail.is_empty() {
        return None;
    }
    let line = div()
        .flex()
        .gap(px(6.))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_size(px(11.))
        .text_color(colours::faint())
        .when(removing, |d| d.child(ui::busy(("ws-removing", ws.id), "removing…").text_size(px(11.))))
        .children(setup.map(|(text, colour)| div().text_color(colour).child(text)))
        .children(detail);
    Some(line)
}

#[derive(PartialEq, Eq)]
enum Chosen {
    No,
    Here,
    Below,
}

struct WorkspaceLook {
    chosen: Chosen,
    collapsed: bool,
    lists_tabs: bool,
    has_children: bool,
    label: String,
}

fn workspace_look(ws: &Workspace, listing: &Listing, rows: &[AgentRow<'_>]) -> WorkspaceLook {
    let collapsed = ws.collapsed;
    let child_lit = listing.tabs || rows.iter().any(|r| listing.shown.pane == Some(r.pane.id));
    let chosen = match listing.shown.workspace == Some(ws.id) {
        false => Chosen::No,
        true if collapsed || !child_lit => Chosen::Here,
        true => Chosen::Below,
    };
    WorkspaceLook {
        chosen,
        collapsed,
        lists_tabs: listing.tabs,
        has_children: listing.tabs || !rows.is_empty(),
        label: if ws.worktree { format!("⎇ {}", ws.name) } else { ws.name.clone() },
    }
}

fn workspace_fold(id: u64, look: &WorkspaceLook, cx: &mut Context<Root>) -> Stateful<Div> {
    let fold_id = SharedString::from(format!("ws-fold-{id}"));
    if !look.has_children {
        return div().id(fold_id).w(px(14.)).flex_none();
    }
    let collapsed = look.collapsed;
    chevron(fold_id, !collapsed).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
        cx.stop_propagation();
        this.set_folded(Fold::Workspace(id), !collapsed);
        cx.notify();
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
        let look = workspace_look(ws, listing, &rows);
        let removing = self.store.read(cx).removing(id);
        let row = row_base(target, look.chosen == Chosen::Here)
            .gap(px(4.))
            .pl(px(6.))
            .pr(px(8.))
            .py(px(3.))
            .child(workspace_fold(id, &look, cx))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .child(self.title_line(ws, &look))
                    .children(detail_line(ws, &look, removing)),
            );
        let row = if removing {
            row.opacity(0.55).cursor_default()
        } else {
            let row = row_actions(row, target, move |this| this.focus_item("workspace", id), cx);
            drop_target(draggable(row, target, look.label.clone()), target, siblings, cx)
        };
        let children = if look.collapsed { Vec::new() } else { self.workspace_children(ws, listing, &rows, cx) };
        div()
            .flex()
            .flex_col()
            .gap(px(1.))
            .child(row)
            .when(!children.is_empty(), |d| d.child(nested(children)))
            .into_any_element()
    }

    fn workspace_children(
        &self,
        ws: &Workspace,
        listing: &Listing,
        rows: &[AgentRow<'_>],
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        if listing.tabs {
            return self.render_tab_rows(ws, listing.shown, cx);
        }
        let place = AgentPlace { scope: "agent", place: None, closable: false };
        rows.iter().map(|agent| self.render_agent(agent, listing.shown, &place, cx)).collect()
    }

    fn title_line(&self, ws: &Workspace, look: &WorkspaceLook) -> Div {
        let chosen = look.chosen != Chosen::No;
        let name = name_label(look.label.clone())
            .min_w(px(0.))
            .text_color(if chosen { colours::text() } else { colours::soft() })
            .when(chosen, |d| d.font_weight(FontWeight::MEDIUM));
        div()
            .flex()
            .items_center()
            .gap(px(5.))
            .child(self.name_or_rename(Target::Workspace(ws.id), name))
            .children(pull_request_tags(ws))
            .when(ws.behind > 0, |d| {
                d.child(
                    div().flex_none().text_size(px(11.)).text_color(colours::muted()).child(format!("↓{}", ws.behind)),
                )
            })
            .when(!look.has_children || look.collapsed, |d| d.child(badge(ws.loudest())))
    }

    fn agent_tab_name(&self, tab: u64) -> String {
        self.session
            .as_ref()
            .and_then(|s| s.projects.iter().flat_map(|p| &p.workspaces).flat_map(|w| &w.tabs).find(|t| t.id == tab))
            .map(|t| t.name.clone())
            .unwrap_or_default()
    }

    pub(super) fn render_agent(
        &self,
        row: &AgentRow<'_>,
        shown: Shown,
        place: &AgentPlace,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let pane = row.pane;
        let id = pane.id;
        let selected = shown.pane == Some(id);
        let title = pane.agent_title().unwrap_or_else(|| self.agent_tab_name(row.tab));
        let details = place.place.iter().cloned().chain([agent_details(pane)]).collect::<Vec<_>>().join(" · ");
        let gid = SharedString::from(format!("{}-{id}", place.scope));
        ui::row(gid.clone(), selected)
            .group(gid.clone())
            .gap(px(4.))
            .ml(px(12. * f32::from(u16::try_from(row.depth).unwrap_or(u16::MAX))))
            .px(px(6.))
            .py(px(3.))
            .child(agent_fold(row, place.scope, cx))
            .child(badge(pane.status.as_deref()))
            .child(agent_text(title, details, selected))
            .when(pane.collapsed && row.children > 0, |d| d.child(ui::dim(format!("+{}", row.children))))
            .when(place.closable, |d| d.child(agent_close_button(id, place.scope, gid.clone(), cx)))
            .child(agent_menu_button(id, place.scope, gid, cx))
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

pub(super) struct AgentPlace {
    pub scope: &'static str,
    pub place: Option<String>,
    pub closable: bool,
}

fn agent_close_button(id: u64, scope: &str, group: SharedString, cx: &mut Context<Root>) -> Stateful<Div> {
    hover_button(SharedString::from(format!("{scope}-close-{id}")), group, "✕", 1)
        .tooltip(tip("Close agent…"))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |_, _: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                forms::close_agent(id, window, cx);
            }),
        )
}

fn agent_menu_button(id: u64, scope: &str, group: SharedString, cx: &mut Context<Root>) -> Stateful<Div> {
    hover_button(SharedString::from(format!("{scope}-menu-{id}")), group, "⋯", 0).on_mouse_down(
        MouseButton::Left,
        cx.listener(move |this, e: &MouseDownEvent, window, cx| {
            cx.stop_propagation();
            this.open_agent_menu(id, e.position, window, cx);
        }),
    )
}

pub(super) fn agent_details(pane: &PaneInfo) -> String {
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

fn agent_fold(row: &AgentRow<'_>, scope: &str, cx: &mut Context<Root>) -> Stateful<Div> {
    let id = row.pane.id;
    let fold_id = SharedString::from(format!("{scope}-fold-{id}"));
    if row.children == 0 {
        return div().id(fold_id).w(px(10.)).flex_none();
    }
    let folded = row.pane.collapsed;
    chevron(fold_id, !folded).w(px(10.)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
        cx.stop_propagation();
        this.set_folded(Fold::Pane(id), !folded);
        cx.notify();
    }))
}

fn agent_text(title: String, details: String, selected: bool) -> Div {
    let line = |text: String| div().child(ui::one_line(text));
    div()
        .flex_1()
        .min_w(px(0.))
        .flex()
        .flex_col()
        .child(line(title).text_size(px(12.)).text_color(if selected { colours::text() } else { colours::soft() }))
        .child(line(details).text_size(px(10.5)).text_color(colours::faint()))
}

#[cfg(test)]
mod tests;
