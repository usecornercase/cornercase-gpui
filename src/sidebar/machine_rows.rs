use std::collections::HashSet;

use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, SharedString, Stateful, StatefulInteractiveElement, Styled, div, prelude::FluentBuilder as _, px,
    relative,
};

use super::Renaming;
use super::look::Look;
use super::rows::{End, Listing, chevron, hover_button, hover_listener, name_label, nested, row_frame, trailing};
use super::tab_rows::{tab_details, tab_text, tab_title};
use super::workspace::{detail_row, plural};
use crate::app::Root;
use crate::model::{Aim, MachinePlace, MachineSeen, Session, Tab, ThereId, ThereProject, ThereWorkspace};
use crate::theme::colours;
use crate::ui::{self, badge};

const DOWN_OPACITY: f32 = 0.55;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct FoldThere {
    machine: String,
    path: String,
    name: Option<String>,
}

impl FoldThere {
    fn workspace(seen: &MachineSeen, ws: &ThereWorkspace) -> Self {
        Self { machine: seen.name.clone(), path: ws.path.clone(), name: Some(ws.name.clone()) }
    }

    fn project(seen: &MachineSeen, there: &ThereProject) -> Self {
        Self { machine: seen.name.clone(), path: there.path.clone(), name: None }
    }
}

#[derive(Debug, Default)]
pub(crate) struct ThereFolds(HashSet<FoldThere>);

impl ThereFolds {
    fn folded(&self, fold: &FoldThere) -> bool {
        self.0.contains(fold)
    }

    fn toggle(&mut self, fold: FoldThere) {
        if !self.0.remove(&fold) {
            self.0.insert(fold);
        }
    }
}

fn there_id(kind: &str, seen: &MachineSeen, id: ThereId) -> SharedString {
    SharedString::from(format!("machine-{}-{kind}-{}", seen.name, id.key()))
}

fn there_detail(ws: &ThereWorkspace, folded: bool) -> Vec<String> {
    let mut detail: Vec<String> = ws.branch.iter().filter(|branch| **branch != ws.name).cloned().collect();
    if folded {
        detail.push(plural(ws.tabs.len(), "tab"));
        let agents = ws.tabs.iter().filter(|tab| tab.agent_pane().is_some()).count();
        detail.extend((agents > 0).then(|| plural(agents, "agent")));
    }
    detail.extend(ws.closed_agents.filter(|n| *n > 0).map(|n| format!("{n} closed")));
    detail
}

fn there_details_line(ws: &ThereWorkspace, folded: bool, leaving: bool) -> Option<Div> {
    let detail = there_detail(ws, folded);
    if detail.is_empty() && !leaving {
        return None;
    }
    let busy = SharedString::from(format!("machine-removing-{}", ws.id.key()));
    Some(
        detail_row()
            .when(leaving, |d| d.child(ui::busy(busy, "removing…").text_size(px(11.))))
            .children(detail.into_iter().map(|text| div().child(ui::one_line(text)))),
    )
}

fn there_tab_details(tab: &Tab<ThereId>, title: &str, look: &Look) -> Option<String> {
    Some(tab_details(tab, None, look)).filter(|details| !details.is_empty() && details != title)
}

fn there_tab_text(name: Div, note: Option<String>, details: Option<String>) -> Div {
    let Some(note) = note else { return tab_text(name.into_any_element(), details) };
    div()
        .flex_1()
        .min_w(px(0.))
        .flex()
        .flex_col()
        .child(tab_text(name.into_any_element(), None))
        .child(div().text_size(px(11.)).text_color(colours::faint()).child(ui::one_line(note)))
}

fn status_of(seen: &MachineSeen, status: Option<&str>) -> Div {
    if seen.linked() { badge(status) } else { ui::still_badge(status) }
}

fn machine_tag(seen: &MachineSeen) -> Div {
    div()
        .flex()
        .min_w_0()
        .items_center()
        .gap(px(3.))
        .text_size(px(11.))
        .text_color(colours::muted())
        .child(ui::icon(seen.icon.icon(), 11.).text_color(colours::muted()))
        .child(div().min_w_0().child(ui::one_line(seen.name.clone())))
}

fn row_end(end: End, hovered: bool) -> Div {
    trailing(end, hovered).max_w(relative(0.5)).min_w_0()
}

fn fold_lead(id: SharedString, fold: Option<(FoldThere, bool)>, cx: &mut Context<Root>) -> Stateful<Div> {
    let Some((fold, folded)) = fold else { return div().id(id).w(px(14.)).flex_none() };
    chevron(id, !folded).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
        cx.stop_propagation();
        this.side.there_folds.toggle(fold.clone());
        cx.notify();
    }))
}

fn pick_on_click(row: Stateful<Div>, place: MachinePlace, cx: &mut Context<Root>) -> Stateful<Div> {
    row.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
        let place = place.clone();
        this.store.update(cx, |s, cx| s.pick_machine_place(place, cx));
    }))
}

fn workspace_buttons(aim: &Aim, gid: &SharedString, cx: &mut Context<Root>) -> [Stateful<Div>; 2] {
    let key = aim.key();
    let (close, menu) = (aim.clone(), aim.clone());
    [
        hover_button(SharedString::from(format!("machine-close-{key}")), gid.clone(), "✕", 1).on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                this.close_there(&close, window, cx);
            }),
        ),
        hover_button(SharedString::from(format!("machine-menu-{key}")), gid.clone(), "⋯", 0).on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                this.open_there_menu(&menu, e.position, window, cx);
            }),
        ),
    ]
}

struct ThereRow<'a> {
    seen: &'a MachineSeen,
    ws: &'a ThereWorkspace,
    aim: Aim,
    chosen: bool,
    folded: bool,
    leaving: bool,
    look: &'a Look,
}

impl Root {
    pub(super) fn there_rows_under(&self, project: u64, listing: &Listing, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(session) = &self.session else { return Vec::new() };
        session.paired_with(project).map(|(seen, ws)| self.there_workspace(seen, ws, listing, cx)).collect()
    }

    pub(super) fn there_workspace(
        &self,
        seen: &MachineSeen,
        ws: &ThereWorkspace,
        listing: &Listing,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let aim = seen.aim_at(ws);
        let leaving = self.store.read(cx).machine_leaving(&aim);
        let chosen = listing.place.as_ref().is_some_and(|place| place.is_at(seen, ws));
        let folded = self.side.there_folds.folded(&FoldThere::workspace(seen, ws));
        let look = &self.side.look.current;
        let row = ThereRow { seen, ws, aim, chosen, folded, leaving, look };
        let children =
            if folded { Vec::new() } else { ws.tabs.iter().map(|tab| there_tab(&row, tab, listing, cx)).collect() };
        div()
            .flex()
            .flex_col()
            .gap(px(1.))
            .when(!seen.linked() || leaving, |d| d.opacity(DOWN_OPACITY))
            .child(self.there_workspace_row(&row, cx))
            .when(!children.is_empty(), |d| d.child(nested(children)))
            .into_any_element()
    }

    fn there_workspace_row(&self, row: &ThereRow<'_>, cx: &mut Context<Self>) -> Stateful<Div> {
        let (seen, ws) = (row.seen, row.ws);
        let gid = there_id("ws", seen, ws.id);
        let lit = row.chosen && (row.folded || ws.tabs.is_empty());
        let fold = (!ws.tabs.is_empty()).then(|| (FoldThere::workspace(seen, ws), row.folded));
        let frame = row_frame(gid.clone(), lit)
            .gap(px(4.))
            .pl(px(6.))
            .pr(px(8.))
            .py(px(3.))
            .child(fold_lead(there_id("ws-fold", seen, ws.id), fold, cx))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .child(self.there_title(row, &gid, cx))
                    .children(there_details_line(ws, row.folded, row.leaving)),
            );
        if row.leaving {
            return frame.cursor_default();
        }
        let menu = row.aim.clone();
        workspace_clicks(frame, MachinePlace::at(seen, ws, None), row.aim.clone(), cx)
            .on_hover(hover_listener(gid, cx))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    this.open_there_menu(&menu, e.position, window, cx);
                }),
            )
    }

    fn there_title(&self, row: &ThereRow<'_>, gid: &SharedString, cx: &mut Context<Self>) -> Div {
        let ws = row.ws;
        let label = if ws.worktree { format!("⎇ {}", ws.name) } else { ws.name.clone() };
        let name = name_label(label)
            .min_w(px(0.))
            .text_color(if row.chosen { colours::text() } else { colours::soft() })
            .when(row.chosen, |d| d.font_weight(FontWeight::MEDIUM));
        let what = Renaming::There(row.aim.clone());
        div()
            .flex()
            .items_center()
            .gap(px(5.))
            .child(self.name_or_field(&what, name))
            .child(self.there_workspace_end(row, gid, cx))
    }

    fn there_workspace_end(&self, row: &ThereRow<'_>, gid: &SharedString, cx: &mut Context<Self>) -> Div {
        let status = row.folded || row.ws.tabs.is_empty();
        let end = End { gap: 5., buttons: if row.leaving { 0 } else { 2 }, figures: true };
        let hovered = !row.leaving && self.row_hovered(gid);
        row_end(end, hovered)
            .child(machine_tag(row.seen))
            .when(status, |d| d.child(status_of(row.seen, row.ws.loudest())))
            .when(!row.leaving, |d| d.children(workspace_buttons(&row.aim, gid, cx)))
    }

    pub(super) fn alone_project_rows(
        &self,
        session: &Session,
        listing: &Listing,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        session.alone().map(|(seen, there)| self.alone_project(seen, there, listing, cx)).collect()
    }

    fn alone_project(
        &self,
        seen: &MachineSeen,
        there: &ThereProject,
        listing: &Listing,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let fold = FoldThere::project(seen, there);
        let folded = !listing.tree || self.side.there_folds.folded(&fold);
        let row = alone_project_row(seen, there, listing, (fold, folded), cx);
        let col = div()
            .flex()
            .flex_col()
            .gap(px(1.))
            .when(listing.tree, |d| d.pt(px(4.)))
            .when(!seen.linked(), |d| d.opacity(DOWN_OPACITY));
        let col = col.child(row);
        if folded {
            return col.into_any_element();
        }
        let children = there.workspaces.iter().map(|ws| self.there_workspace(seen, ws, listing, cx)).collect();
        col.child(nested(children)).into_any_element()
    }
}

fn there_tab(row: &ThereRow<'_>, tab: &Tab<ThereId>, listing: &Listing, cx: &mut Context<Root>) -> AnyElement {
    let selected = row.chosen && listing.place.as_ref().is_some_and(|place| place.tab == Some(tab.id));
    let title = tab_title(tab);
    let details = there_tab_details(tab, &title, row.look);
    let note = row.seen.tab_note(tab, listing.now);
    let name = name_label(title).flex_initial().min_w(px(0.)).text_color(if selected {
        colours::text()
    } else {
        colours::soft()
    });
    let frame = row_frame(there_id("tab", row.seen, tab.id), selected)
        .gap(px(6.))
        .px(px(6.))
        .py(px(3.))
        .child(status_of(row.seen, tab.loudest()))
        .child(there_tab_text(name, note, details));
    if row.leaving {
        return frame.cursor_default().into_any_element();
    }
    pick_on_click(frame, MachinePlace::at(row.seen, row.ws, Some(tab.id)), cx).into_any_element()
}

fn alone_project_row(
    seen: &MachineSeen,
    there: &ThereProject,
    listing: &Listing,
    (fold, folded): (FoldThere, bool),
    cx: &mut Context<Root>,
) -> Stateful<Div> {
    let shown = listing
        .place
        .as_ref()
        .is_some_and(|place| place.machine == seen.name && there.workspaces.iter().any(|ws| ws.id == place.workspace));
    let name = name_label(there.name.clone()).min_w(px(0.)).font_weight(FontWeight::MEDIUM).text_color(if shown {
        colours::text()
    } else {
        colours::soft()
    });
    let lead_id = there_id("project-fold", seen, there.id);
    let lead = if listing.tree { fold_lead(lead_id, Some((fold, folded)), cx) } else { div().id(lead_id).w(px(4.)) };
    let end = End { gap: 4., buttons: 0, figures: true };
    let frame = row_frame(there_id("project", seen, there.id), shown && !listing.tree)
        .gap(px(4.))
        .px(px(6.))
        .py(px(3.))
        .child(lead)
        .child(name)
        .child(row_end(end, false).child(machine_tag(seen)).when(folded, |d| {
            d.when(listing.counts, |d| d.child(ui::dim(format!("{}", there.workspaces.len()))))
                .child(status_of(seen, there.attention()))
        }));
    match there.workspaces.first() {
        Some(ws) => pick_on_click(frame, MachinePlace::at(seen, ws, None), cx),
        None => frame.cursor_default(),
    }
}

fn workspace_clicks(row: Stateful<Div>, place: MachinePlace, aim: Aim, cx: &mut Context<Root>) -> Stateful<Div> {
    row.on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
        if e.click_count() == 2 {
            this.start_there_rename(&aim, window, cx);
        } else {
            let place = place.clone();
            this.store.update(cx, |s, cx| s.pick_machine_place(place, cx));
        }
    }))
}

#[cfg(test)]
mod tests;
