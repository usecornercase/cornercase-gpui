use std::rc::Rc;

use gpui::{
    AnyElement, ClickEvent, Context, Div, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px,
};

use super::rows::{drop_target, name_label, row_actions, row_base};
use super::workspace::{agent_details, plural};
use super::{Target, draggable};
use crate::app::Root;
use crate::model::{PaneInfo, Shown, Tab, Workspace};
use crate::theme::colours;
use crate::ui::{self, badge};

pub(super) fn tab_title(tab: &Tab) -> String {
    tab.agent_pane().and_then(PaneInfo::agent_title).unwrap_or_else(|| tab.name.clone())
}

pub(super) fn tab_details(tab: &Tab) -> String {
    let mut details = match tab.agent_pane() {
        Some(agent) => vec![agent_details(agent)],
        None => tab.panes.first().and_then(|p| p.program.clone()).into_iter().collect(),
    };
    if tab.panes.len() > 1 {
        details.push(plural(tab.panes.len(), "pane"));
    }
    details.join(" · ")
}

fn tab_text(title: AnyElement, details: Option<String>) -> Div {
    div().flex_1().min_w(px(0.)).flex().items_center().gap(px(6.)).child(title).children(details.map(|details| {
        div().flex_1().min_w(px(0.)).text_size(px(11.)).text_color(colours::faint()).child(ui::one_line(details))
    }))
}

fn new_tab_row(workspace: u64, cx: &mut Context<Root>) -> AnyElement {
    div()
        .id(SharedString::from(format!("ws-new-tab-{workspace}")))
        .px(px(6.))
        .py(px(2.))
        .rounded(px(crate::theme::radius::CONTROL))
        .text_size(px(11.))
        .text_color(colours::faint())
        .cursor_pointer()
        .hover(|s| s.bg(colours::hover()).text_color(colours::text()))
        .child("+ Tab")
        .on_click(cx.listener(move |this, _: &ClickEvent, _, _| {
            this.act(vec!["new-tab".into(), "--workspace".into(), workspace.to_string(), "--focus".into()]);
        }))
        .into_any_element()
}

impl Root {
    pub(super) fn render_tab_rows(&self, ws: &Workspace, shown: Shown, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let siblings: Rc<Vec<u64>> = Rc::new(ws.tabs.iter().map(|t| t.id).collect());
        let mut rows: Vec<AnyElement> = ws
            .tabs
            .iter()
            .map(|tab| self.render_tab_row(tab, shown.tab == Some(tab.id), siblings.clone(), cx))
            .collect();
        rows.push(new_tab_row(ws.id, cx));
        rows
    }

    fn render_tab_row(&self, tab: &Tab, selected: bool, siblings: Rc<Vec<u64>>, cx: &mut Context<Self>) -> AnyElement {
        let id = tab.id;
        let target = Target::Tab(id);
        let title = tab_title(tab);
        let details = Some(tab_details(tab)).filter(|d| !d.is_empty() && *d != title);
        let name = name_label(title.clone()).flex_initial().min_w(px(0.)).text_color(if selected {
            colours::text()
        } else {
            colours::soft()
        });
        let row = row_base(target, selected)
            .gap(px(6.))
            .px(px(6.))
            .py(px(3.))
            .child(badge(tab.loudest()))
            .child(tab_text(self.name_or_rename(target, name), details));
        let row = row_actions(row, target, move |this| this.focus_item("tab", id), cx);
        let row = draggable(row, target, title);
        drop_target(row, target, siblings, cx).into_any_element()
    }
}

#[cfg(test)]
mod tests;
