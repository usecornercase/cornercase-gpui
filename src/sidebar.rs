mod actions;
mod agents;
mod arrange;
mod folds;
mod forms;
mod look;
mod menus;
pub(crate) mod order;
pub(crate) mod rail;
mod rows;
mod tab_rows;
mod workspace;

use gpui::{
    App, AppContext as _, ClickEvent, Context, Div, Entity, FontWeight, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, Render, SharedString, Stateful, StatefulInteractiveElement, Styled, Window, div, px,
};

use crate::app::{GAP, Root, TITLEBAR_HEIGHT};
use crate::context_menu::{Menu, item};
use crate::dialogs;
use crate::keymap::{Command, with_keys};
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, TextField};

pub(crate) use crate::store::args as cmd;
pub(crate) use folds::Fold;
pub(crate) use forms::close_agent;
pub(crate) use look::Arrangement;
pub(crate) use menus::reopening;

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
        ui::glass::dragged().px(px(10.)).py(px(4.)).text_size(px(12.)).child(self.0.clone())
    }
}

pub(crate) fn draggable(el: Stateful<Div>, target: Target, label: impl Into<SharedString>) -> Stateful<Div> {
    let dragged = Dragged { target, label: label.into() };
    el.on_drag(dragged, |d: &Dragged, _, _, cx: &mut App| cx.new(|_| DragPreview(d.label.clone())))
}

pub(crate) struct Tip(SharedString);

impl Render for Tip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        ui::glass::popover()
            .max_w(px(360.))
            .px(px(9.))
            .py(px(6.))
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
    pub folds: folds::Folds,
    pub look: look::Looks,
    pub sizes: arrange::Sizes,
    pub hovered: Option<SharedString>,
}

impl Root {
    pub(crate) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .w(px(self.sidebar_extent()))
            .h_full()
            .flex_none()
            .relative()
            .flex()
            .flex_col()
            .pb(px(GAP))
            .child(title_strip())
            .child(self.sidebar_cards(cx))
            .children(self.render_menu(cx))
    }

    pub(crate) fn sidebar_extent(&self) -> f32 {
        if self.side.look.current.arrangement == Arrangement::SideBySide {
            self.pair_width.size()
        } else {
            self.sidebar_width.size()
        }
    }
}

fn card() -> Div {
    div()
        .relative()
        .flex_1()
        .flex()
        .flex_col()
        .rounded(px(radius::CARD))
        .bg(colours::surface())
        .border_1()
        .border_color(colours::border())
        .child(ui::glass::wash().rounded_t(px(radius::CARD - 1.)))
}

fn title_strip() -> impl IntoElement {
    div()
        .h(px(TITLEBAR_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(8.))
        .pl(px(34.))
        .child(ui::brand::mark(1.25, false))
        .child(ui::brand::wordmark(13.))
}

fn section_head(title: &'static str, button: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_between()
        .pl(px(20.))
        .pr(px(12.))
        .pt(px(8.))
        .pb(px(6.))
        .child(div().text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).text_color(colours::muted()).child(title))
        .child(button)
}

fn sidebar_header(cx: &mut Context<Root>) -> Div {
    section_head(
        "Projects",
        ui::tool_button("sidebar-new", Icon::Plus, 24., false)
            .tooltip(tip("Open a project or add a group"))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    let entries = vec![
                        item("Open project…", Root::open_project),
                        item("New group…", |_, window, cx| forms::new_group(None, window, cx)),
                    ];
                    this.open_menu(e.position, entries, window, cx);
                }),
            ),
    )
}

fn list_end(cx: &mut Context<Root>) -> impl IntoElement + use<> {
    div()
        .id("sidebar-end")
        .flex_1()
        .min_h(px(40.))
        .drag_over::<Dragged>(|s, d, _, _| match d.target {
            Target::Project(_) | Target::Group(_) => s.bg(colours::accent_soft()),
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
        .gap(px(8.))
        .px(px(12.))
        .pt(px(8.))
        .pb(px(12.))
        .child(
            ui::labelled_button("new-workspace", Icon::Plus, "Workspace", false)
                .tooltip(tip(with_keys("New workspace", Command::NewWorkspace)))
                .on_click(cx.listener(|_, _: &ClickEvent, window, cx| dialogs::new_workspace(None, window, cx))),
        )
        .child(
            ui::labelled_button("start-agent", Icon::Agent, "Agent", true)
                .tooltip(tip(with_keys("Start an agent", Command::StartAgent)))
                .on_click(cx.listener(|_, _: &ClickEvent, window, cx| dialogs::start_agent(window, cx))),
        )
}
