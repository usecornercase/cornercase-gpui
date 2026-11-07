use gpui::{AnyElement, Context, Div, InteractiveElement, ParentElement, StatefulInteractiveElement, Styled, div, px};

use super::arrange::divider;
use super::machine_agents::ThereListing;
use super::rows::list_text;
use super::section_head;
use super::workspace::AgentPlace;
use crate::app::Root;
use crate::model::{AgentRow, SectionRow, Session, Shown};
use crate::ui;

pub(crate) fn places_shown(rows: &[SectionRow<'_>]) -> Vec<bool> {
    let mut path = Vec::new();
    rows.iter()
        .map(|row| {
            path.truncate(row.depth());
            let shown = row.depth() == 0 || path.last() != Some(&row.workspace());
            path.push(row.workspace());
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
        let store = self.store.read(cx);
        let rows = session.section_agents(&session.project_order(), |aim| store.machine_leaving(aim));
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

    fn agent_list(&self, session: &Session, rows: &[SectionRow<'_>], cx: &mut Context<Self>) -> Div {
        if rows.is_empty() {
            return ui::note("No agents running.");
        }
        let there = ThereListing {
            place: self.side.machine_place.clone(),
            now: crate::panels::queue::now(),
            card: self.main_card_width(),
        };
        let list = list_text(div().flex().flex_col().gap(px(1.)).px(px(10.)).pb(px(4.)));
        list.children(rows.iter().zip(places_shown(rows)).map(|(row, with_place)| match row {
            SectionRow::Here(row) => self.here_agent(session, row, with_place, cx),
            SectionRow::There(row) => {
                self.there_agent(row, with_place.then(|| session.there_place(&row.agent)), &there, cx)
            }
        }))
    }

    fn here_agent(
        &self,
        session: &Session,
        row: &AgentRow<'_>,
        with_place: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let shown = if self.side.machine_place.is_some() { Shown::default() } else { session.shown() };
        let place = AgentPlace {
            scope: "section-agent",
            place: with_place.then(|| place(session, row.workspace)).flatten(),
            closable: true,
        };
        self.render_agent(row, shown, &place, cx)
    }
}

#[cfg(test)]
mod tests;
