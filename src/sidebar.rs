mod actions;
mod forms;
mod menus;
pub(crate) mod order;
mod rows;
mod workspace;

use std::rc::Rc;

use gpui::{
    App, AppContext as _, ClickEvent, Context, Div, Entity, Focusable, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, ParentElement, Render, SharedString, Stateful, StatefulInteractiveElement, Styled,
    Window, div, prelude::FluentBuilder as _, px,
};

use crate::app::{Root, SIDEBAR_WIDTH, TITLEBAR_HEIGHT};
use crate::context_menu::{Menu, item};
use crate::dialogs;
use crate::model::Session;
use crate::panels::PanelKind;
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent, button_coloured as button};

pub(crate) use crate::store::args as cmd;
pub(crate) use forms::close_agent;
use rows::Listing;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    Group(u64),
    Project(u64),
    Workspace(u64),
    Tab(u64),
}

impl Target {
    pub(crate) fn id(self) -> u64 {
        match self {
            Target::Group(id) | Target::Project(id) | Target::Workspace(id) | Target::Tab(id) => id,
        }
    }

    pub(crate) fn flag(self) -> &'static str {
        match self {
            Target::Group(_) => "--group",
            Target::Project(_) => "--project",
            Target::Workspace(_) => "--workspace",
            Target::Tab(_) => "--tab",
        }
    }

    fn same_kind(self, other: Target) -> bool {
        std::mem::discriminant(&self) == std::mem::discriminant(&other)
    }
}

#[derive(Clone)]
pub(crate) struct Dragged {
    pub target: Target,
    pub label: SharedString,
}

pub(crate) struct DragPreview(SharedString);

impl Render for DragPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(10.))
            .py(px(4.))
            .rounded(px(6.))
            .bg(colours::selected())
            .border_1()
            .border_color(colours::accent())
            .text_size(px(12.))
            .text_color(colours::text())
            .shadow_lg()
            .child(self.0.clone())
    }
}

pub(crate) fn draggable(el: Stateful<Div>, target: Target, label: impl Into<SharedString>) -> Stateful<Div> {
    let dragged = Dragged { target, label: label.into() };
    el.on_drag(dragged, |d: &Dragged, _, _, cx: &mut App| cx.new(|_| DragPreview(d.label.clone())))
}

pub(crate) struct Tip(SharedString);

impl Render for Tip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .max_w(px(360.))
            .px(px(8.))
            .py(px(5.))
            .rounded(px(5.))
            .bg(colours::panel())
            .border_1()
            .border_color(colours::border())
            .text_size(px(12.))
            .text_color(colours::text())
            .child(self.0.clone())
    }
}

pub(crate) fn tip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView + 'static {
    let text = text.into();
    move |_, cx| cx.new(|_| Tip(text.clone())).into()
}

pub(crate) struct Rename {
    pub target: Target,
    pub field: Entity<TextField>,
    initial: String,
}

#[derive(Default)]
pub(crate) struct SideState {
    pub menu: Option<Menu>,
    pub rename: Option<Rename>,
    pub filter: Option<Entity<TextField>>,
    pub query: String,
    pub pane_press: Option<crate::terminal::PanePress>,
}

impl Root {
    pub(crate) fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let filter = self.filter_field(cx);
        let filtering = !self.side.query.is_empty();
        div()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .bg(colours::sidebar())
            .border_r_1()
            .border_color(colours::border())
            .child(div().h(px(TITLEBAR_HEIGHT)).flex_none())
            .child(sidebar_header(cx))
            .child(filter_bar(filter, filtering, cx))
            .child(self.sidebar_list(cx))
            .child(self.panel_buttons())
            .child(sidebar_footer(cx))
            .children(self.render_menu(cx))
    }

    fn sidebar_list(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let query = self.side.query.clone();
        let nothing_matches = !query.is_empty()
            && self.session.as_ref().is_some_and(|s| !s.projects.iter().any(|p| order::shows_project(p, &query)));
        div()
            .id("sidebar-list")
            .flex_1()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .child(self.sidebar_rows(cx))
            .when(nothing_matches, |d| d.child(ui::note("Nothing matches.")))
            .child(list_end(cx))
    }

    fn sidebar_rows(&self, cx: &mut Context<Self>) -> Div {
        let list = div().flex().flex_col().gap(px(1.)).px(px(8.));
        let Some(session) = &self.session else { return list };
        let listing = Listing {
            shown: session.shown(),
            query: self.side.query.clone(),
            groups: Rc::new(session.groups.iter().map(|g| g.id).collect()),
        };
        let mut rows = Vec::new();
        for section in order::sections(session) {
            rows.extend(self.section_rows(session, section, &listing, cx));
        }
        list.children(rows)
    }

    fn panel_buttons(&self) -> impl IntoElement + use<> {
        let store = self.store.clone();
        div()
            .flex()
            .flex_wrap()
            .gap(px(4.))
            .px(px(10.))
            .pt(px(8.))
            .border_t_1()
            .border_color(colours::border())
            .children(PanelKind::ALL.into_iter().filter(|k| !k.docked()).map(move |kind| {
                let store = store.clone();
                ui::segment(SharedString::from(format!("open-{}", kind.title())), kind.title(), false)
                    .on_click(move |_: &ClickEvent, _, cx| store.update(cx, |s, cx| s.open(kind, cx)))
            }))
    }

    fn filter_field(&mut self, cx: &mut Context<Self>) -> Entity<TextField> {
        if let Some(field) = &self.side.filter {
            return field.clone();
        }
        let field = cx.new(|cx| TextField::new("Filter  ⌘P", cx));
        cx.subscribe(&field, |this, field, event, cx| match event {
            TextFieldEvent::Changed => {
                this.side.query = field.read(cx).text();
                cx.notify();
            }
            TextFieldEvent::Cancel => field.update(cx, TextField::clear),
            TextFieldEvent::Submit => {
                if let Some(id) = this.first_shown_workspace() {
                    this.focus_item("workspace", id);
                }
            }
        })
        .detach();
        self.side.filter = Some(field.clone());
        field
    }

    fn first_shown_workspace(&self) -> Option<u64> {
        let query = &self.side.query;
        self.session.as_ref().and_then(|s: &Session| {
            s.projects
                .iter()
                .find_map(|p| p.workspaces.iter().find(|w| order::shows_workspace(p, w, query)).map(|w| w.id))
        })
    }

    pub(crate) fn focus_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let field = self.filter_field(cx);
        window.focus(&field.read(cx).focus_handle(cx));
        cx.notify();
    }
}

fn sidebar_header(cx: &mut Context<Root>) -> impl IntoElement + use<> {
    div()
        .flex()
        .items_center()
        .justify_between()
        .px(px(14.))
        .pb(px(6.))
        .child(div().text_size(px(11.)).text_color(colours::faint()).child("PROJECTS"))
        .child(button("sidebar-new", "+ New", colours::muted()).on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, e: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                let entries = vec![
                    item("Open project…", Root::open_project),
                    item("New group…", |_, window, cx| forms::new_group(None, window, cx)),
                ];
                this.open_menu(e.position, entries, window, cx);
            }),
        ))
}

fn filter_bar(filter: Entity<TextField>, filtering: bool, cx: &mut Context<Root>) -> impl IntoElement + use<> {
    let clear = filtering.then(|| {
        ui::icon_button("filter-clear", "✕").on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
            if let Some(f) = &this.side.filter {
                f.update(cx, TextField::clear);
            }
            window.focus(&this.focus);
        }))
    });
    div()
        .px(px(10.))
        .pb(px(6.))
        .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, _| {
            if e.keystroke.key == "escape" {
                window.focus(&this.focus);
            }
        }))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(8.))
                .py(px(2.))
                .rounded(px(5.))
                .bg(colours::terminal())
                .border_1()
                .border_color(colours::border())
                .text_size(px(12.))
                .child(div().text_color(colours::faint()).child("⌕"))
                .child(div().flex_1().child(filter))
                .children(clear),
        )
}

fn list_end(cx: &mut Context<Root>) -> impl IntoElement + use<> {
    div()
        .id("sidebar-end")
        .flex_1()
        .min_h(px(40.))
        .drag_over::<Dragged>(|s, d, _, _| match d.target {
            Target::Project(_) | Target::Group(_) => s.bg(colours::hover()),
            _ => s,
        })
        .on_drop(cx.listener(|this, d: &Dragged, _, _| match d.target {
            Target::Project(id) => this.queue.cmd(actions::ungroup(id)),
            Target::Group(_) => this.queue.cmd(actions::place_args(d.target, order::Place::End)),
            _ => {}
        }))
}

fn sidebar_footer(cx: &mut Context<Root>) -> impl IntoElement + use<> {
    div()
        .flex()
        .gap(px(6.))
        .p(px(10.))
        .child(
            button("new-workspace", "+ Workspace", colours::muted())
                .on_click(cx.listener(|_, _: &ClickEvent, window, cx| dialogs::new_workspace(None, window, cx))),
        )
        .child(
            button("start-agent", "▶ Agent", colours::accent())
                .on_click(cx.listener(|_, _: &ClickEvent, window, cx| dialogs::start_agent(window, cx))),
        )
}
