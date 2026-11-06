use gpui::{
    AnyElement, App, ClickEvent, Context, Div, FocusHandle, Focusable as _, InteractiveElement as _, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, ParentElement as _, ScrollHandle, Stateful,
    StatefulInteractiveElement as _, Styled as _, Window, deferred, div, prelude::FluentBuilder as _, px,
};

use super::{StartFlow, fact_line};
use crate::theme::colours;
use crate::ui::motion::{self, Way};
use crate::ui::{self, Icon, menu};

const MENU_HEIGHT: f32 = 268.;
const BELOW: f32 = 6.;
const DROP: f32 = 4.;
const NONE_OPEN: &str = "Open a git repository's folder as a project first: a flow works in a worktree of its own.";
const UNCHOSEN: &str = "Choose the project the team works in";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProjectChoice {
    pub(super) name: String,
    pub(super) path: String,
}

pub(super) struct ProjectMenu {
    open: bool,
    highlight: Option<usize>,
    focus: FocusHandle,
    scroll: ScrollHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Empty,
    Fixed,
    Choose,
}

fn shape(count: usize, chosen: bool) -> Shape {
    match count {
        0 => Shape::Empty,
        1 if chosen => Shape::Fixed,
        _ => Shape::Choose,
    }
}

impl ProjectMenu {
    pub(super) fn new(cx: &mut App) -> Self {
        Self { open: false, highlight: None, focus: cx.focus_handle(), scroll: ScrollHandle::new() }
    }
}

fn place(project: &ProjectChoice) -> Vec<AnyElement> {
    vec![
        ui::icon(Icon::FolderOpen, 14.).text_color(colours::accent()).into_any_element(),
        div().flex().min_w_0().text_color(colours::text()).child(ui::one_line(project.name.clone())).into_any_element(),
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .text_color(colours::faint())
            .child(ui::one_line(ui::home_relative(&project.path)))
            .into_any_element(),
    ]
}

fn unchosen() -> Vec<AnyElement> {
    vec![
        ui::icon(Icon::FolderOpen, 14.).text_color(colours::muted()).into_any_element(),
        div().flex_1().min_w_0().flex().text_color(colours::muted()).child(ui::one_line(UNCHOSEN)).into_any_element(),
    ]
}

impl StartFlow {
    pub(super) fn project_picker(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let chosen = self.chosen_project();
        match (shape(self.projects.len(), chosen.is_some()), chosen) {
            (Shape::Empty, _) => ui::dim(NONE_OPEN).into_any_element(),
            (Shape::Fixed, Some(project)) => fact_line().children(place(project)).into_any_element(),
            _ => self.project_dropdown(chosen, window, cx).into_any_element(),
        }
    }

    fn chosen_project(&self) -> Option<&ProjectChoice> {
        let id = self.project?;
        self.projects.iter().find(|(choice, _)| *choice == id).map(|(_, project)| project)
    }

    fn project_dropdown(&self, chosen: Option<&ProjectChoice>, window: &Window, cx: &Context<Self>) -> Div {
        let open = self.menu.open;
        let close = cx.listener(|this, _: &MouseDownEvent, window, cx| this.close_projects(window, cx));
        let edge = if open { colours::accent() } else { colours::border() };
        let trigger = menu::dropdown("start-project", edge)
            .py(px(6.))
            .text_size(px(13.))
            .children(chosen.map_or_else(unchosen, place))
            .child(menu::chevron())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.toggle_projects(window, cx);
                }),
            );
        let menu = open.then(|| (menu::backdrop("start-projects-backdrop", window, close), self.project_menu(cx)));
        div().relative().flex().flex_col().child(trigger).when_some(menu, |d, (veil, card)| d.child(veil).child(card))
    }

    fn project_menu(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let rows: Vec<Stateful<Div>> = self.projects.iter().enumerate().map(|(i, _)| self.project_row(i, cx)).collect();
        let card = menu::card("start-projects", &self.menu.focus, &self.menu.scroll)
            .max_h(px(MENU_HEIGHT))
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| this.project_key(e, window, cx)))
            .children(rows);
        let shown = motion::slide(card, "start-projects-in", Way::Pop, (0., -DROP));
        deferred(div().absolute().top_full().left_0().right_0().pt(px(BELOW)).child(shown)).with_priority(2)
    }

    fn project_row(&self, index: usize, cx: &Context<Self>) -> Stateful<Div> {
        let (id, project) = &self.projects[index];
        let id = *id;
        let inks = menu::inks(self.menu.highlight == Some(index), false);
        let check = (self.project == Some(id)).then(|| ui::icon(Icon::Check, 13.).text_color(inks.mark));
        let name = div().flex().text_color(inks.value).child(ui::one_line(project.name.clone()));
        let path = div().flex().text_size(px(11.5)).text_color(inks.note);
        menu::entry(("start-project-choice", id), self.menu.highlight == Some(index), check)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(name)
                    .child(path.child(ui::one_line(ui::home_relative(&project.path)))),
            )
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                this.menu.highlight = menu::hovered(this.menu.highlight, index, *hovered);
                cx.notify();
            }))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.choose_project(id, window, cx)))
    }

    fn toggle_projects(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu.open {
            self.close_projects(window, cx);
            return;
        }
        let at = self.projects.iter().position(|(id, _)| Some(*id) == self.project);
        self.menu.open = true;
        self.menu.highlight = at;
        window.focus(&self.menu.focus);
        cx.notify();
    }

    fn close_projects(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.menu.open) {
            return;
        }
        if self.menu.focus.is_focused(window) {
            window.focus(&self.fields.prompt.read(cx).focus_handle(cx));
        }
        cx.notify();
    }

    fn choose_project(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        self.close_projects(window, cx);
        self.pick_project(id, cx);
    }

    fn project_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        match key {
            "escape" => self.close_projects(window, cx),
            "down" | "up" => {
                self.menu.highlight = menu::step(self.menu.highlight, self.projects.len(), key == "down");
                if let Some(at) = self.menu.highlight {
                    self.menu.scroll.scroll_to_item(at + menu::ABOVE);
                }
                cx.notify();
            }
            "enter" => {
                if let Some(id) = self.menu.highlight.and_then(|at| self.projects.get(at)).map(|(id, _)| *id) {
                    self.choose_project(id, window, cx);
                }
            }
            _ => return,
        }
        cx.stop_propagation();
    }
}

#[cfg(test)]
mod tests;
