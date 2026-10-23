use gpui::{
    AnyElement, ClickEvent, Context, Div, IntoElement, ParentElement as _, Stateful, StatefulInteractiveElement as _,
    Styled as _, div, prelude::FluentBuilder as _, px,
};

use super::actions::{Question, ask_on_click, flow_buttons};
use super::model::{FlowInfo, FlowSpec, RoleSpec};
use super::widgets::{agents_label, error_text, file_name, open_path, role_label, tag, text_box};
use super::{FlowsPanel, diagram};
use crate::store::args;
use crate::theme::colours;
use crate::ui;

impl FlowsPanel {
    pub(super) fn flow_page(&self, flow: &FlowInfo, hidden: bool, cx: &Context<Self>) -> Div {
        let spec = &flow.flow;
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(flow_title(flow, hidden))
            .when(!flow.description.is_empty(), |d| {
                d.child(div().text_size(px(13.)).text_color(colours::muted()).child(flow.description.clone()))
            })
            .children(flow.path.as_ref().map(|p| ui::dim(p.clone())))
            .children(flow.problems.iter().map(|p| error_text(format!("Cannot start: {p}"))))
            .children(flow.check.as_ref().map(|c| error_text(c.clone())))
            .child(flow_buttons(flow, hidden, cx))
            .child(ui::section("Plan"))
            .child(diagram::diagram(spec, None))
            .child(ui::section("Roles"))
            .children(spec.every_role().enumerate().flat_map(|(i, role)| self.role_rows(spec, i, role, cx)))
            .when(!spec.notes.is_empty(), |d| {
                d.child(ui::section("Notes")).child(div().text_size(px(12.)).child(spec.notes.clone()))
            })
    }

    fn role_rows(&self, spec: &FlowSpec, i: usize, role: &RoleSpec, cx: &Context<Self>) -> Vec<AnyElement> {
        let open = self.prompt.as_deref() == Some(role.name.as_str());
        let mut rows = vec![role_row(spec, (i, role), open, cx).into_any_element()];
        if open {
            rows.push(prompt_box(role).into_any_element());
        }
        rows
    }

    fn toggle_prompt(&mut self, name: &str, cx: &mut Context<Self>) {
        self.prompt = if self.prompt.as_deref() == Some(name) { None } else { Some(name.to_string()) };
        cx.notify();
    }

    pub(super) fn broken_page(&self, path: &str, cx: &Context<Self>) -> Div {
        let error = self
            .listing
            .as_ref()
            .and_then(|l| l.broken.iter().find(|b| b.path == path))
            .map(|b| b.error.clone())
            .unwrap_or_default();
        let delete = Question::new(
            "Delete this file?",
            format!("{path} is deleted from disk."),
            "Delete",
            args(&["flow", "delete", "--path", path]),
        );
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(ui::title(file_name(path)).text_size(px(16.)))
            .child(ui::dim(path.to_string()))
            .child(error_text(format!("It cannot be read as a flow, so it has no diagram: {error}")))
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .pt(px(6.))
                    .child(ui::button("broken-open", "Open file").on_click(open_path(path.to_string())))
                    .child(ui::danger_button("broken-delete", "Delete file").on_click(ask_on_click(delete, cx))),
            )
    }
}

fn flow_title(flow: &FlowInfo, hidden: bool) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(ui::title(flow.name.clone()).text_size(px(16.)))
        .children(flow.tags().into_iter().map(tag))
        .when(hidden, |d| d.child(tag("deleted")))
        .child(div().flex_1())
        .child(ui::dim(agents_label(flow)))
}

fn role_row(spec: &FlowSpec, (i, role): (usize, &RoleSpec), open: bool, cx: &Context<FlowsPanel>) -> Stateful<Div> {
    let name = role.name.clone();
    ui::row(("role", i), open)
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_prompt(&name, cx)))
        .child(div().text_color(colours::faint()).text_size(px(10.)).child(if open { "▾" } else { "▸" }))
        .child(role_label(diagram::role_colour(spec, &role.name), role.name.clone()))
        .child(
            div().flex_1().truncate().text_size(px(12.)).text_color(colours::muted()).child(role.facts().join(" · ")),
        )
}

fn prompt_box(role: &RoleSpec) -> Div {
    text_box().ml(px(24.)).mb(px(6.)).child(if role.prompt.is_empty() {
        "(no prompt of its own)".to_string()
    } else {
        role.prompt.clone()
    })
}
