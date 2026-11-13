use std::rc::Rc;

use gpui::{
    AnyElement, ClickEvent, Context, Div, ElementId, FontWeight, InteractiveElement as _, IntoElement as _,
    ParentElement as _, Rgba, Stateful, StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _,
    px,
};

use super::FlowsPanel;
use super::actions::{Question, ask_on_click, flow_buttons};
use super::cards::{avatar, conductor};
use super::diagram::{OnRole, Plan, role_colour};
use super::hero::{description, facts, hero, team, title};
use super::model::{FlowInfo, FlowSpec, RoleSpec};
use super::widgets::{agents_label, alert, chip, file_name, heading, open_path, panel_box, typed_chip};
use crate::store::args;
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, Tone, glass::tint};

impl FlowsPanel {
    pub(super) fn flow_page(&self, flow: &FlowInfo, hidden: bool, cx: &Context<Self>) -> Div {
        let spec = &flow.flow;
        let key = flow.key();
        let panel = cx.entity().downgrade();
        let on_role: OnRole = Rc::new(move |name, _, cx| {
            let _ = panel.update(cx, |this, cx| this.toggle_prompt(name, cx));
        });
        let plan = Plan { flow: spec, run: None, focus: self.prompt.as_deref(), key: &key, on_role: Some(&on_role) };
        div()
            .flex()
            .flex_col()
            .gap(px(22.))
            .child(flow_hero(flow, hidden, cx))
            .child(block(Icon::Flows, "Plan").child(self.diagram(plan)))
            .child(block(Icon::Accounts, "Roles").child(self.roles(spec, cx)))
            .when(!spec.notes.is_empty(), |d| d.child(block(Icon::File, "Notes").child(notes(&spec.notes))))
    }

    fn roles(&self, spec: &FlowSpec, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .children(spec.every_role().enumerate().map(|(i, role)| self.role_item(spec, (i, role), cx)))
    }

    fn role_item(&self, spec: &FlowSpec, (i, role): (usize, &RoleSpec), cx: &Context<Self>) -> Stateful<Div> {
        let open = self.prompt.as_deref() == Some(role.name.as_str());
        let colour = if i == 0 { colours::soft() } else { role_colour(spec, &role.name) };
        let name = role.name.clone();
        role_box(("role", i), colour, open)
            .cursor_pointer()
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_prompt(&name, cx)))
            .child(role_head((i, role), colour, open, None))
            .when(open, |d| d.child(prompt(role)))
    }

    pub(super) fn toggle_prompt(&mut self, name: &str, cx: &mut Context<Self>) {
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
        )
        .doing(format!("Deleting {path}…"));
        panel_box()
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(20.))
            .child(broken_title(path))
            .child(alert(format!("It cannot be read as a flow, so it has no diagram: {error}"), colours::failed()))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(
                        ui::icon_text_button("broken-open", Icon::FolderOpen, "Open file", Tone::Plain)
                            .on_click(open_path(path.to_string())),
                    )
                    .child(
                        ui::icon_text_button("broken-delete", Icon::Trash, "Delete file", Tone::Danger)
                            .on_click(ask_on_click(delete, cx)),
                    ),
            )
    }
}

pub(super) fn role_box(id: impl Into<ElementId>, colour: Rgba, open: bool) -> Stateful<Div> {
    panel_box()
        .id(id)
        .flex()
        .flex_col()
        .when(open, |d| d.border_color(tint(colour, 0.55)).bg(tint(colour, 0.06)))
        .when(!open, |d| d.hover(|s| s.border_color(colours::edge()).bg(colours::hover())))
}

pub(super) fn block(glyph: Icon, label: &str) -> Div {
    div().flex().flex_col().gap(px(10.)).child(heading(glyph, label))
}

fn flow_hero(flow: &FlowInfo, hidden: bool, cx: &Context<FlowsPanel>) -> Div {
    hero(&flow.flow)
        .child(flow_title(flow, hidden))
        .when(!flow.description.is_empty(), |d| d.child(description(&flow.description)))
        .children(flow.path.as_ref().map(|path| {
            div()
                .relative()
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(px(11.))
                .text_color(colours::faint())
                .child(ui::icon(Icon::File, 12.).text_color(colours::faint()))
                .child(div().min_w_0().flex().child(ui::one_line(path.clone())))
        }))
        .children(flow.problems.iter().map(|p| alert(format!("Cannot start: {p}"), colours::failed()).relative()))
        .children(flow.check.as_ref().map(|c| alert(c.clone(), colours::waiting()).relative()))
        .child(flow_buttons(flow, hidden, cx))
}

fn flow_title(flow: &FlowInfo, hidden: bool) -> Div {
    let spec = &flow.flow;
    let line = facts(vec![agents_label(flow), spec.outline(), format!("then {}", spec.end_label())])
        .children(flow.tags().into_iter().map(tag_chip))
        .when(hidden, |d| d.child(chip(Some(Icon::Trash), "deleted", colours::failed())));
    title(spec, None, flow.name.clone().into(), line).child(team(spec))
}

fn tag_chip(tag: &'static str) -> Div {
    match tag {
        "default" => chip(Some(Icon::StarFilled), tag, colours::accent()),
        _ => chip(None, tag, colours::muted()),
    }
}

pub(super) fn role_head((i, role): (usize, &RoleSpec), colour: Rgba, open: bool, under: Option<Div>) -> Div {
    let face = if i == 0 { conductor(30.) } else { avatar(&role.name, colour, 30.) };
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .px(px(12.))
        .py(px(10.))
        .child(face)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(5.))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(5.))
                        .child(
                            div()
                                .min_w_0()
                                .text_size(px(13.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(role.name.clone()),
                        )
                        .children(role_chips(role)),
                )
                .children(under),
        )
        .child(ui::icon(if open { Icon::ChevronDown } else { Icon::ChevronRight }, 14.).text_color(colours::faint()))
}

pub(super) fn role_chips(role: &RoleSpec) -> Vec<AnyElement> {
    let muted = colours::muted();
    let mut chips = vec![chip(Some(Icon::Agent), role.agent.clone(), colours::soft()).into_any_element()];
    chips.extend(role.model.clone().map(|model| chip(None, model, muted).into_any_element()));
    chips.extend(role.effort.clone().map(|effort| chip(None, format!("effort {effort}"), muted).into_any_element()));
    chips.push(
        if role.edits { chip(Some(Icon::Pencil), "edits", muted) } else { chip(Some(Icon::Eye), "read only", muted) }
            .into_any_element(),
    );
    if role.worktree == "own" {
        chips.push(chip(Some(Icon::Branch), "own worktree", muted).into_any_element());
    }
    if !role.env.is_empty() {
        let names = role.env.keys().cloned().collect::<Vec<_>>().join(", ");
        chips.push(typed_chip("role-env", format!("env {names}"), muted).into_any_element());
    }
    if !role.args.is_empty() {
        chips.push(typed_chip("role-args", format!("args {}", role.args.join(" ")), muted).into_any_element());
    }
    chips
}

fn prompt(role: &RoleSpec) -> Div {
    let text = if role.prompt.is_empty() { "(no prompt of its own)".to_string() } else { role.prompt.clone() };
    div()
        .mx(px(12.))
        .mb(px(12.))
        .p(px(12.))
        .rounded(px(radius::control()))
        .bg(colours::inset())
        .border_1()
        .border_color(colours::border())
        .text_size(px(12.))
        .line_height(px(18.))
        .text_color(colours::soft())
        .child(text)
}

fn notes(text: &str) -> Div {
    panel_box().p(px(14.)).text_size(px(12.)).line_height(px(18.)).text_color(colours::soft()).child(text.to_string())
}

fn broken_title(path: &str) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .child(
            div()
                .size(px(44.))
                .flex_none()
                .rounded(px(12.))
                .flex()
                .items_center()
                .justify_center()
                .bg(tint(colours::failed(), 0.12))
                .border_1()
                .border_color(tint(colours::failed(), 0.3))
                .child(ui::icon(Icon::File, 20.).text_color(colours::failed())),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(div().text_size(px(18.)).font_weight(FontWeight::BOLD).child(file_name(path)))
                .child(div().text_size(px(11.)).text_color(colours::faint()).child(ui::one_line(path.to_string()))),
        )
}
