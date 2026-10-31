use gpui::{Div, FontWeight, ParentElement as _, Styled as _, div, px};
use serde::Deserialize;

use super::widgets::{alert, panel_box};
use crate::theme::colours;
use crate::ui;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(super) struct Checked {
    pub name: String,
    pub origin: String,
    pub replaced: bool,
    pub roles: Vec<CheckedRole>,
    pub check: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(super) struct CheckedRole {
    pub role: String,
    pub agent: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub edits: bool,
    pub worktree: Option<String>,
}

pub(super) fn parse_checked(text: &str) -> anyhow::Result<Checked> {
    Ok(serde_json::from_str(text)?)
}

impl CheckedRole {
    pub(super) fn facts(&self) -> String {
        let mut facts: Vec<String> =
            [Some(self.agent.clone()), self.model.clone(), self.effort.clone()].into_iter().flatten().collect();
        if self.edits {
            facts.push("edits".into());
        }
        if self.worktree.as_deref() == Some("own") {
            facts.push("own worktree".into());
        }
        facts.join(" · ")
    }
}

impl Checked {
    pub(super) fn headline(&self) -> String {
        let place = if self.origin == "project" { "the project's flows" } else { "yours" };
        let verb = if self.replaced { "replaces the flow of that name in" } else { "goes into" };
        format!("{} {verb} {place}, with {} agents.", self.name, self.roles.len())
    }
}

pub(super) fn preview(checked: &Checked) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(div().text_size(px(12.)).text_color(colours::soft()).child(checked.headline()))
        .child(panel_box().flex().flex_col().p(px(8.)).gap(px(4.)).children(checked.roles.iter().map(role_line)))
        .children(checked.check.clone().map(|c| alert(c, colours::waiting())))
        .children(checked.note.clone().map(|n| alert(n, colours::muted())))
}

fn role_line(role: &CheckedRole) -> Div {
    div()
        .flex()
        .gap(px(8.))
        .text_size(px(12.))
        .child(div().w(px(130.)).flex_none().font_weight(FontWeight::MEDIUM).child(ui::one_line(role.role.clone())))
        .child(ui::dim(ui::one_line(role.facts())))
}

#[cfg(test)]
mod tests;
