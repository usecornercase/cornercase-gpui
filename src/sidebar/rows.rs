use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, FontWeight, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, SharedString, Stateful, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use super::{Dragged, Fold, Target, draggable, forms, order};
use crate::app::Root;
use crate::model::{Group, Project, Session, Shown};
use crate::theme::{colours, radius};
use crate::ui::{self, badge, glass};

#[derive(Clone, Copy)]
pub(super) struct End {
    pub gap: f32,
    pub buttons: u8,
    pub figures: bool,
}

pub(super) struct Listing {
    pub shown: Shown,
    pub groups: Rc<Vec<u64>>,
    pub tree: bool,
    pub tabs: bool,
}

fn id_prefix(target: Target) -> &'static str {
    match target {
        Target::Group(_) => "group",
        Target::Project(_) => "project",
        Target::Workspace(_) => "ws",
        Target::Tab(_) => "tab",
    }
}

const GUIDE: f32 = 13.;
const BUTTON: f32 = 22.;
const BUTTON_GAP: f32 = 2.;
const BESIDE: f32 = 4.;

pub(super) fn row_group_id(target: Target) -> SharedString {
    SharedString::from(format!("{}-{}", id_prefix(target), target.id()))
}

pub(super) fn chevron(id: SharedString, open: bool) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(14.))
        .flex_none()
        .text_size(px(13.))
        .text_color(colours::faint())
        .cursor_pointer()
        .hover(|s| s.text_color(colours::text()))
        .child(if open { "▾" } else { "▸" })
}

fn button_margin(slot: u8) -> f32 {
    BESIDE + (BUTTON + BUTTON_GAP) * f32::from(slot)
}

fn room(buttons: u8) -> f32 {
    buttons.checked_sub(1).map_or(0., |last| button_margin(last) + BUTTON)
}

fn end_margin(end: End, hovered: bool) -> f32 {
    match (hovered, end.figures) {
        (true, _) => room(end.buttons),
        (false, true) => 0.,
        (false, false) => -end.gap,
    }
}

fn after_hover(current: Option<SharedString>, row: SharedString, hovered: bool) -> Option<SharedString> {
    if hovered {
        Some(row)
    } else if current.as_ref() == Some(&row) {
        None
    } else {
        current
    }
}

pub(super) fn hover_button(id: SharedString, group: SharedString, glyph: &'static str, slot: u8) -> Stateful<Div> {
    div()
        .id(id)
        .absolute()
        .right(relative(1.))
        .mr(px(button_margin(slot)))
        .size(px(BUTTON))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::CHIP))
        .text_color(colours::faint())
        .bg(colours::raised())
        .hover(|s| s.bg(colours::raised_hover()).text_color(colours::text()))
        .cursor_pointer()
        .invisible()
        .group_hover(group, Styled::visible)
        .child(glyph)
}

pub(super) fn trailing(end: End, hovered: bool) -> Div {
    div().flex_none().flex().items_center().gap(px(end.gap)).ml(px(end_margin(end, hovered)))
}

pub(super) fn hover_listener(
    row: SharedString,
    cx: &mut Context<Root>,
) -> impl Fn(&bool, &mut Window, &mut App) + 'static {
    cx.listener(move |this, hovered: &bool, _, cx| this.hover_row(row.clone(), *hovered, cx))
}

pub(super) fn row_base(target: Target, lit: bool) -> Stateful<Div> {
    let gid = row_group_id(target);
    let row = div()
        .id(gid.clone())
        .group(gid)
        .relative()
        .flex()
        .items_center()
        .rounded(px(radius::CONTROL))
        .border_t_2()
        .border_b_2()
        .border_color(gpui::transparent_black())
        .cursor_pointer();
    if lit { glass::lit(row) } else { row.hover(|s| s.bg(colours::hover())) }
}

pub(super) fn menu_button(target: Target, cx: &mut Context<Root>) -> Stateful<Div> {
    let id = SharedString::from(format!("{}-menu-{}", id_prefix(target), target.id()));
    hover_button(id, row_group_id(target), "⋯", 0).on_mouse_down(
        MouseButton::Left,
        cx.listener(move |this, e: &MouseDownEvent, window, cx| {
            cx.stop_propagation();
            this.open_target_menu(target, e.position, window, cx);
        }),
    )
}

pub(super) fn row_actions(
    row: Stateful<Div>,
    target: Target,
    primary: impl Fn(&mut Root) + 'static,
    cx: &mut Context<Root>,
) -> Stateful<Div> {
    row.on_hover(hover_listener(row_group_id(target), cx))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                this.open_target_menu(target, e.position, window, cx);
            }),
        )
        .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
            if e.click_count() == 2 {
                this.start_rename(target, window, cx);
            } else {
                primary(this);
                cx.notify();
            }
        }))
}

pub(super) fn drop_target(
    el: Stateful<Div>,
    target: Target,
    siblings: Rc<Vec<u64>>,
    cx: &mut Context<Root>,
) -> Stateful<Div> {
    el.drag_over::<Dragged>(move |s, d, _, _| {
        let into_group = matches!((d.target, target), (Target::Project(_), Target::Group(_)));
        if into_group {
            return s.bg(colours::accent_soft());
        }
        let accepted =
            d.target.same_kind(target) || matches!((d.target, target), (Target::Group(_), Target::Project(_)));
        if !accepted || d.target == target {
            return s;
        }
        if order::lands_after(&siblings, d.target.id(), target.id()) {
            s.border_color(colours::accent()).border_b_2()
        } else {
            s.border_color(colours::accent()).border_t_2()
        }
    })
    .on_drop(cx.listener(move |this, d: &Dragged, _, _| this.drop_on(d.target, target)))
}

pub(super) fn list_text(list: Div) -> Div {
    list.text_size(px(12.5)).line_height(relative(1.4))
}

pub(super) fn nested(children: Vec<AnyElement>) -> Div {
    div()
        .ml(px(GUIDE))
        .pl(px(5.))
        .border_l_1()
        .border_color(colours::border())
        .flex()
        .flex_col()
        .gap(px(1.))
        .children(children)
}

pub(super) fn name_label(name: impl Into<SharedString>) -> Div {
    div().flex_1().child(ui::one_line(name))
}

impl Root {
    fn hover_row(&mut self, row: SharedString, hovered: bool, cx: &mut Context<Self>) {
        let next = after_hover(self.side.hovered.clone(), row, hovered);
        if next != self.side.hovered {
            self.side.hovered = next;
            cx.notify();
        }
    }

    pub(super) fn row_hovered(&self, row: &SharedString) -> bool {
        self.side.hovered.as_ref() == Some(row)
    }

    pub(super) fn row_end(&self, target: Target, end: End, cx: &mut Context<Self>) -> Div {
        trailing(end, self.row_hovered(&row_group_id(target))).child(menu_button(target, cx))
    }

    pub(super) fn name_or_rename(&self, target: Target, label: Div) -> AnyElement {
        let Some(rename) = self.side.rename.as_ref().filter(|r| r.target == target) else {
            return label.into_any_element();
        };
        div().flex_1().min_w(px(0.)).child(ui::input_box(rename.field.clone(), true).py(px(1.))).into_any_element()
    }

    pub(super) fn section_rows(
        &self,
        session: &Session,
        (group, ids): (Option<u64>, Vec<u64>),
        listing: &Listing,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let projects: Vec<&Project> =
            ids.iter().filter_map(|id| session.projects.iter().find(|p| p.id == *id)).collect();
        let mut rows = Vec::new();
        if let Some(group) = group.and_then(|g| session.groups.iter().find(|x| x.id == g)) {
            rows.push(self.render_group(group, ids.len(), listing, cx));
            if group.collapsed {
                return rows;
            }
        }
        let siblings = Rc::new(ids);
        for project in projects {
            rows.push(self.render_project(project, listing, siblings.clone(), cx));
        }
        rows
    }

    fn render_group(&self, group: &Group, count: usize, listing: &Listing, cx: &mut Context<Self>) -> AnyElement {
        let id = group.id;
        let target = Target::Group(id);
        let colour = forms::palette_colour(group.colour.unwrap_or(4));
        let collapsed = group.collapsed;
        let holds_shown = self
            .session
            .as_ref()
            .is_some_and(|s| s.projects.iter().any(|p| p.group == Some(id) && Some(p.id) == listing.shown.project));
        let name =
            name_label(group.name.clone()).text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(colour);
        let row = row_base(target, false)
            .mt(px(10.))
            .gap(px(6.))
            .px(px(8.))
            .py(px(3.))
            .when(collapsed && holds_shown, |d| d.bg(colours::selected()))
            .child(
                div().w(px(14.)).flex_none().text_color(colour).child(group.icon.clone().unwrap_or_else(|| "●".into())),
            )
            .child(self.name_or_rename(target, name))
            .child(self.group_end(target, count, collapsed, cx));
        let row = row_actions(row, target, move |this| this.set_folded(Fold::Group(id), !collapsed), cx);
        let row = draggable(row, target, group.name.clone());
        drop_target(row, target, listing.groups.clone(), cx).into_any_element()
    }

    fn group_end(&self, target: Target, count: usize, collapsed: bool, cx: &mut Context<Self>) -> Div {
        let fold = div().text_size(px(13.)).text_color(colours::faint()).child(if collapsed { "▸" } else { "▾" });
        self.row_end(target, End { gap: 6., buttons: 1, figures: true }, cx)
            .when(collapsed, |d| d.child(ui::dim(format!("{count}"))))
            .child(fold)
    }

    fn render_project(
        &self,
        project: &Project,
        listing: &Listing,
        siblings: Rc<Vec<u64>>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = project.id;
        let target = Target::Project(id);
        let collapsed = project.collapsed || !listing.tree;
        let row = self.project_row(project, (listing.shown.project == Some(id), listing.tree), collapsed, cx);
        let row = row_actions(row, target, move |this| this.focus_item("project", id), cx);
        let row = draggable(row, target, project.name.clone());
        let row = drop_target(row, target, siblings, cx);
        let col = div().flex().flex_col().gap(px(1.)).when(listing.tree, |d| d.pt(px(4.))).child(row);
        if collapsed || !listing.tree {
            return col.into_any_element();
        }
        let workspaces: Rc<Vec<u64>> = Rc::new(project.workspaces.iter().map(|w| w.id).collect());
        let mut children = Vec::new();
        for ws in &project.workspaces {
            children.push(self.render_workspace(ws, listing, workspaces.clone(), cx));
        }
        col.child(nested(children)).into_any_element()
    }

    fn project_row(
        &self,
        project: &Project,
        (shown, tree): (bool, bool),
        collapsed: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let id = project.id;
        let target = Target::Project(id);
        let name = name_label(project.name.clone())
            .min_w(px(0.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(if shown { colours::text() } else { colours::soft() });
        let lead = if tree { project_fold(id, collapsed, cx) } else { div().id(("project-lead", id)).w(px(4.)) };
        row_base(target, shown && !tree)
            .gap(px(4.))
            .px(px(6.))
            .py(px(3.))
            .child(lead)
            .child(self.name_or_rename(target, name))
            .child(self.project_end(project, collapsed, cx))
    }

    fn project_end(&self, project: &Project, collapsed: bool, cx: &mut Context<Self>) -> Div {
        let id = project.id;
        let target = Target::Project(id);
        self.row_end(target, End { gap: 4., buttons: 2, figures: collapsed }, cx)
            .when(collapsed, |d| {
                d.child(ui::dim(format!("{}", project.workspaces.len()))).child(badge(project.attention()))
            })
            .child(
                hover_button(SharedString::from(format!("project-add-{id}")), row_group_id(target), "+", 1)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            this.open_new_menu(Some(id), e.position, window, cx);
                        }),
                    ),
            )
    }
}

fn project_fold(id: u64, collapsed: bool, cx: &mut Context<Root>) -> Stateful<Div> {
    chevron(SharedString::from(format!("project-fold-{id}")), !collapsed).on_click(cx.listener(
        move |this, _: &ClickEvent, _, cx| {
            cx.stop_propagation();
            this.set_folded(Fold::Project(id), !collapsed);
            cx.notify();
        },
    ))
}

#[cfg(test)]
mod tests;
