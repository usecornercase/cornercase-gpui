use gpui::{Context, Div, InteractiveElement, ParentElement, StatefulInteractiveElement, Styled, div, px};

use super::arrange::divider;
use super::rows::list_text;
use super::section_head;
use super::workspace::AgentPlace;
use crate::app::Root;
use crate::model::{AgentRow, Session};
use crate::ui;

pub(crate) fn places_shown(rows: &[AgentRow<'_>]) -> Vec<bool> {
    let mut path: Vec<u64> = Vec::new();
    rows.iter()
        .map(|row| {
            path.truncate(row.depth);
            let shown = row.depth == 0 || path.last() != Some(&row.workspace);
            path.push(row.workspace);
            shown
        })
        .collect()
}

fn place(session: &Session, workspace: u64) -> Option<String> {
    let (project, ws) = session.workspace(workspace)?;
    Some(if project.workspaces.len() > 1 { format!("{} › {}", project.name, ws.name) } else { project.name.clone() })
}

impl Root {
    pub(super) fn agents_section(&self, cx: &mut Context<Self>) -> Option<Div> {
        if !self.side.look.current.agents_section {
            return None;
        }
        let session = self.session.as_ref()?;
        let order: Vec<u64> = session.sections().into_iter().flat_map(|(_, ids)| ids).collect();
        let rows = session.agent_rows(&order);
        let count = ui::dim(rows.len().to_string());
        let list = self.agent_list(session, &rows, cx);
        Some(
            div()
                .flex_none()
                .flex()
                .flex_col()
                .child(divider())
                .child(section_head("Agents", count))
                .child(div().id("sidebar-agents").flex_1().min_h_0().overflow_y_scroll().child(list)),
        )
    }

    fn agent_list(&self, session: &Session, rows: &[AgentRow<'_>], cx: &mut Context<Self>) -> Div {
        if rows.is_empty() {
            return ui::note("No agents running.");
        }
        let shown = session.shown();
        let list = list_text(div().flex().flex_col().gap(px(1.)).px(px(10.)).pb(px(4.)));
        list.children(rows.iter().zip(places_shown(rows)).map(|(row, with_place)| {
            let place = AgentPlace {
                scope: "section-agent",
                place: with_place.then(|| place(session, row.workspace)).flatten(),
                closable: true,
            };
            self.render_agent(row, shown, &place, cx)
        }))
    }
}

#[cfg(test)]
mod tests;
