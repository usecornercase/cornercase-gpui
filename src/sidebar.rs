//! The sidebar: groups, projects and their workspaces (branch, pull request and its checks, how
//! far behind, setup, changes) and, under each workspace, its agents as a tree of who started
//! whom. Rows fold, rename in place (double-click), drag to reorder (`cornercase move`), and open
//! a menu with a right-click or their `⋯`. A quick filter at the top narrows the list.

mod forms;
pub(crate) mod order;

use std::rc::Rc;

use gpui::{
    AnyElement, AnyView, App, AppContext as _, ClickEvent, Context, Div, Entity, Focusable, FontWeight,
    InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, ParentElement, PathPromptOptions,
    Pixels, Point, Render, Rgba, SharedString, Stateful, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};

use crate::app::{Root, SIDEBAR_WIDTH, TITLEBAR_HEIGHT, tilde};
use crate::context_menu::{Entry, Menu, act, danger, item, submenu};
use crate::dialogs;
use crate::model::{AgentRow, Group, PaneInfo, Project, PullRequest, Shown, Workspace};
use crate::panels::PanelKind;
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent, badge, button_coloured as button};

pub(crate) use forms::close_agent;
use order::Place;

/// A row of the sidebar or the tab bar, by id: what a menu, a rename or a drag acts on.
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

    /// The flag commands name it with: `--group`, `--project`…
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

/// What a row carries while it is dragged.
#[derive(Clone)]
pub(crate) struct Dragged {
    pub target: Target,
    pub label: SharedString,
}

/// The pill that follows the pointer while a row is dragged.
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

/// Makes a row draggable as `target`.
pub(crate) fn draggable(el: Stateful<Div>, target: Target, label: impl Into<SharedString>) -> Stateful<Div> {
    let dragged = Dragged { target, label: label.into() };
    el.on_drag(dragged, |d: &Dragged, _, _, cx: &mut App| cx.new(|_| DragPreview(d.label.clone())))
}

/// A small text box shown on hover.
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

pub(crate) fn tip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    move |_, cx| cx.new(|_| Tip(text.clone())).into()
}

/// A row being renamed: its text field, in place of its name.
pub(crate) struct Rename {
    pub target: Target,
    pub field: Entity<TextField>,
    initial: String,
}

/// The sidebar's, the tabs' and the panes' own state, kept on `Root`.
#[derive(Default)]
pub(crate) struct SideState {
    pub menu: Option<Menu>,
    pub rename: Option<Rename>,
    pub filter: Option<Entity<TextField>>,
    pub query: String,
    /// A right press on a pane, until its release: a menu, or a move onto another pane.
    pub pane_press: Option<crate::terminal::PanePress>,
}

/// The tag of a pull request: `#485✓`, its colour, and whether it is crossed out (closed).
pub(crate) fn pull_tag(pr: &PullRequest) -> (String, Rgba, bool) {
    let number = format!("#{}", pr.number);
    match pr.state.as_str() {
        "merged" => (number, forms::palette_colour(99), false),
        "closed" => (number, colours::faint(), true),
        _ => match pr.checks.as_deref() {
            Some("failing") => (format!("{number}✗"), colours::failed(), false),
            Some("pending") => (format!("{number}•"), colours::working(), false),
            Some("passing") => (format!("{number}✓"), colours::done(), false),
            _ => (number, colours::muted(), false),
        },
    }
}

/// What hovering a pull request's tag says.
fn pull_tip(pr: &PullRequest) -> String {
    let state = if pr.conflicts { format!("{}, conflicts", pr.state) } else { pr.state.clone() };
    let mut text = format!("#{} {}\n{state}", pr.number, pr.title);
    if let Some(checks) = &pr.checks {
        text.push_str(&format!(" · checks {checks}"));
    }
    if !pr.failed.is_empty() {
        text.push_str(&format!("\nfailed: {}", pr.failed.join(", ")));
    }
    text
}

/// An agent's conversation title without the spinner or mark its program puts in front.
fn agent_title(pane: &PaneInfo) -> Option<String> {
    let title = pane.title.as_deref()?.trim_start_matches(|c: char| !c.is_alphanumeric()).trim();
    (!title.is_empty()).then(|| title.to_string())
}

fn plural(n: usize, one: &str) -> String {
    format!("{n} {one}{}", if n == 1 { "" } else { "s" })
}

fn chevron(id: SharedString, open: bool) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(14.))
        .flex_none()
        .text_size(px(15.))
        .text_color(colours::muted())
        .cursor_pointer()
        .hover(|s| s.text_color(colours::text()))
        .child(if open { "▾" } else { "▸" })
}

/// A row button that shows over the row's right end while the row is hovered, so it takes no
/// room from the name otherwise. `slot` counts from the right.
fn hover_button(id: SharedString, group: SharedString, glyph: &'static str, slot: usize) -> Stateful<Div> {
    ui::icon_button(id, glyph)
        .absolute()
        .top_0()
        .bottom_0()
        .right(px(4. + 22. * slot as f32))
        .w(px(20.))
        .flex()
        .items_center()
        .justify_center()
        .bg(colours::hover())
        .invisible()
        .group_hover(group, Styled::visible)
}

impl Root {
    // -----------------------------------------------------------------------------------------
    // Drawing.

    pub(crate) fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let filter = self.filter_field(cx);
        let query = self.side.query.clone();
        let shown = self.session.as_ref().map(super::model::Session::shown).unwrap_or_default();
        let mut list = div().flex().flex_col().gap(px(1.)).px(px(8.));
        if let Some(session) = &self.session {
            let sections = order::sections(session);
            let group_ids: Rc<Vec<u64>> = Rc::new(session.groups.iter().map(|g| g.id).collect());
            for (group, ids) in sections {
                let projects: Vec<&Project> = ids
                    .iter()
                    .filter_map(|id| session.projects.iter().find(|p| p.id == *id))
                    .filter(|p| order::shows_project(p, &query))
                    .collect();
                let group = group.and_then(|g| session.groups.iter().find(|x| x.id == g));
                if let Some(group) = group {
                    if !query.trim().is_empty() && projects.is_empty() {
                        continue;
                    }
                    list = list.child(self.render_group(group, ids.len(), shown, group_ids.clone(), cx));
                    if group.collapsed && query.trim().is_empty() {
                        continue;
                    }
                }
                let siblings = Rc::new(ids);
                for project in projects {
                    list = list.child(self.render_project(project, shown, &query, siblings.clone(), cx));
                }
            }
        }
        // Below the rows: a drop here takes a project out of its group, or a group to the end.
        let end = div()
            .id("sidebar-end")
            .flex_1()
            .min_h(px(40.))
            .drag_over::<Dragged>(|s, d, _, _| match d.target {
                Target::Project(_) | Target::Group(_) => s.bg(colours::hover()),
                _ => s,
            })
            .on_drop(cx.listener(|this, d: &Dragged, _, _| match d.target {
                Target::Project(id) => this.queue.cmd(cmd(&["move", "--project", &id.to_string(), "--ungroup"])),
                Target::Group(id) => this.queue.cmd(cmd(&["move", "--group", &id.to_string(), "--last"])),
                _ => {}
            }));
        let filtering = !query.is_empty();
        div()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .bg(colours::sidebar())
            .border_r_1()
            .border_color(colours::border())
            .child(div().h(px(TITLEBAR_HEIGHT)).flex_none())
            .child(
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
                    )),
            )
            .child(
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
                            .child(div().flex_1().child(filter.clone()))
                            .when(filtering, |d| {
                                d.child(ui::icon_button("filter-clear", "✕").on_click(cx.listener(
                                    move |this, _: &ClickEvent, window, cx| {
                                        if let Some(f) = &this.side.filter {
                                            f.update(cx, TextField::clear);
                                        }
                                        window.focus(&this.focus);
                                    },
                                )))
                            }),
                    ),
            )
            .child(
                div()
                    .id("sidebar-list")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .child(list)
                    .when(
                        filtering
                            && self
                                .session
                                .as_ref()
                                .is_some_and(|s| !s.projects.iter().any(|p| order::shows_project(p, &query))),
                        |d| d.child(ui::note("Nothing matches.")),
                    )
                    .child(end),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(4.))
                    .px(px(10.))
                    .pt(px(8.))
                    .border_t_1()
                    .border_color(colours::border())
                    .children(PanelKind::ALL.into_iter().filter(|k| !k.docked()).map(|kind| {
                        let store = self.store.clone();
                        ui::segment(SharedString::from(format!("open-{}", kind.title())), kind.title(), false)
                            .on_click(move |_: &ClickEvent, _, cx| store.update(cx, |s, cx| s.open(kind, cx)))
                    })),
            )
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .p(px(10.))
                    .child(button("new-workspace", "+ Workspace", colours::muted()).on_click(
                        cx.listener(|_, _: &ClickEvent, window, cx| dialogs::new_workspace(None, window, cx)),
                    ))
                    .child(
                        button("start-agent", "▶ Agent", colours::accent())
                            .on_click(cx.listener(|_, _: &ClickEvent, window, cx| dialogs::start_agent(window, cx))),
                    ),
            )
            .children(self.render_menu(cx))
    }

    /// The quick filter's field, made on first use.
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
                // Enter goes to the first workspace the filter shows.
                let query = this.side.query.clone();
                let first = this.session.as_ref().and_then(|s| {
                    s.projects
                        .iter()
                        .find_map(|p| p.workspaces.iter().find(|w| order::shows_workspace(p, w, &query)).map(|w| w.id))
                });
                if let Some(id) = first {
                    this.focus_item("workspace", id);
                }
            }
        })
        .detach();
        self.side.filter = Some(field.clone());
        field
    }

    /// Puts the keys in the quick filter (⌘P).
    pub(crate) fn focus_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let field = self.filter_field(cx);
        window.focus(&field.read(cx).focus_handle(cx));
        cx.notify();
    }

    /// The text field of a row being renamed, boxed, in place of its name.
    fn rename_box(&self, target: Target) -> Option<AnyElement> {
        let rename = self.side.rename.as_ref().filter(|r| r.target == target)?;
        Some(
            div().flex_1().min_w(px(0.)).child(ui::input_box(rename.field.clone(), true).py(px(1.))).into_any_element(),
        )
    }

    /// The drag-and-drop rules of a row: what may land on it and the line that shows where.
    fn drop_target(
        &self,
        el: Stateful<Div>,
        target: Target,
        siblings: Rc<Vec<u64>>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        el.drag_over::<Dragged>(move |s, d, _, _| {
            let into_group = matches!((d.target, target), (Target::Project(_), Target::Group(_)));
            if into_group {
                return s.bg(colours::selected());
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

    fn render_group(
        &self,
        group: &Group,
        count: usize,
        shown: Shown,
        groups: Rc<Vec<u64>>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = group.id;
        let gid = SharedString::from(format!("group-{id}"));
        let colour = forms::palette_colour(group.colour.unwrap_or(4));
        let collapsed = group.collapsed;
        let holds_shown = self
            .session
            .as_ref()
            .is_some_and(|s| s.projects.iter().any(|p| p.group == Some(id) && Some(p.id) == shown.project));
        let label = group.name.clone();
        let row = div()
            .id(gid.clone())
            .group(gid.clone())
            .relative()
            .mt(px(10.))
            .flex()
            .items_center()
            .gap(px(6.))
            .px(px(6.))
            .py(px(3.))
            .rounded(px(5.))
            .border_t_2()
            .border_b_2()
            .border_color(gpui::transparent_black())
            .cursor_pointer()
            .hover(|s| s.bg(colours::hover()))
            .when(collapsed && holds_shown, |d| d.bg(colours::selected()))
            .child(
                div().w(px(14.)).flex_none().text_color(colour).child(group.icon.clone().unwrap_or_else(|| "●".into())),
            )
            .child(match self.rename_box(Target::Group(id)) {
                Some(field) => field,
                None => div()
                    .flex_1()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colour)
                    .child(group.name.clone())
                    .into_any_element(),
            })
            .when(collapsed, |d| d.child(ui::dim(format!("{count}"))))
            .child(div().text_size(px(15.)).text_color(colours::muted()).child(if collapsed { "▸" } else { "▾" }))
            .child(hover_button(SharedString::from(format!("group-menu-{id}")), gid, "⋯", 0).on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.open_target_menu(Target::Group(id), e.position, window, cx);
                }),
            ))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    this.open_target_menu(Target::Group(id), e.position, window, cx)
                }),
            )
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                if e.click_count() == 2 {
                    this.start_rename(Target::Group(id), window, cx);
                } else {
                    this.set_folded(Target::Group(id), !collapsed);
                }
            }));
        let row = draggable(row, Target::Group(id), label);
        self.drop_target(row, Target::Group(id), groups, cx).into_any_element()
    }

    fn render_project(
        &self,
        project: &Project,
        shown: Shown,
        query: &str,
        siblings: Rc<Vec<u64>>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = project.id;
        let is_shown = shown.project == Some(id);
        let filtering = !query.trim().is_empty();
        let collapsed = project.collapsed && !filtering;
        let gid = SharedString::from(format!("project-{id}"));
        let row = div()
            .id(gid.clone())
            .group(gid.clone())
            .relative()
            .flex()
            .items_center()
            .gap(px(4.))
            .px(px(4.))
            .py(px(3.))
            .rounded(px(5.))
            .border_t_2()
            .border_b_2()
            .border_color(gpui::transparent_black())
            .cursor_pointer()
            .hover(|s| s.bg(colours::hover()))
            .child(chevron(SharedString::from(format!("project-fold-{id}")), !collapsed).on_click(cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    cx.stop_propagation();
                    this.set_folded(Target::Project(id), !collapsed);
                },
            )))
            .child(match self.rename_box(Target::Project(id)) {
                Some(field) => field,
                None => div()
                    .flex_1()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if is_shown { colours::text() } else { colours::muted() })
                    .child(project.name.clone())
                    .into_any_element(),
            })
            .when(collapsed, |d| {
                d.child(ui::dim(format!("{}", project.workspaces.len()))).child(badge(project.loudest()))
            })
            .child(hover_button(SharedString::from(format!("project-add-{id}")), gid.clone(), "+", 1).on_click(
                cx.listener(move |_, _: &ClickEvent, window, cx| {
                    cx.stop_propagation();
                    dialogs::new_workspace(Some(id), window, cx);
                }),
            ))
            .child(hover_button(SharedString::from(format!("project-menu-{id}")), gid, "⋯", 0).on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.open_target_menu(Target::Project(id), e.position, window, cx);
                }),
            ))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    this.open_target_menu(Target::Project(id), e.position, window, cx)
                }),
            )
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                if e.click_count() == 2 {
                    this.start_rename(Target::Project(id), window, cx);
                } else {
                    this.focus_item("project", id);
                }
            }));
        let row = draggable(row, Target::Project(id), project.name.clone());
        let row = self.drop_target(row, Target::Project(id), siblings, cx);
        let mut col = div().flex().flex_col().pt(px(4.)).child(row);
        if collapsed {
            return col.into_any_element();
        }
        let workspaces: Rc<Vec<u64>> = Rc::new(project.workspaces.iter().map(|w| w.id).collect());
        for ws in project.workspaces.iter().filter(|w| order::shows_workspace(project, w, query)) {
            col = col.child(self.render_workspace(ws, shown, filtering, workspaces.clone(), cx));
        }
        col.into_any_element()
    }

    fn render_workspace(
        &self,
        ws: &Workspace,
        shown: Shown,
        filtering: bool,
        siblings: Rc<Vec<u64>>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let wid = ws.id;
        let selected = shown.workspace == Some(wid);
        let rows = ws.agent_rows();
        let collapsed = ws.collapsed && !filtering;
        let gid = SharedString::from(format!("ws-{wid}"));
        let label = if ws.worktree { format!("⎇ {}", ws.name) } else { ws.name.clone() };

        // The second line: the branch when the name is not it, changes, tabs, closed agents.
        let mut detail: Vec<AnyElement> = Vec::new();
        if let Some(branch) = ws.branch.as_deref().filter(|b| *b != ws.name) {
            detail.push(div().overflow_hidden().text_ellipsis().child(branch.to_string()).into_any_element());
        }
        if let Some(ch) = ws.changes.as_ref().filter(|c| c.files > 0) {
            detail.push(
                div()
                    .flex()
                    .gap(px(3.))
                    .child(div().text_color(colours::done()).child(format!("+{}", ch.added)))
                    .child(div().text_color(colours::failed()).child(format!("−{}", ch.removed)))
                    .into_any_element(),
            );
        }
        let tabs = ws.tabs.len();
        let agents = ws.tabs.iter().filter(|t| t.agent_pane().is_some()).count();
        detail.push(div().child(plural(tabs, "tab")).into_any_element());
        if agents > 0 && collapsed {
            detail.push(div().child(plural(agents, "agent")).into_any_element());
        }
        if let Some(n) = ws.closed_agents.filter(|n| *n > 0) {
            detail.push(div().child(format!("{n} closed")).into_any_element());
        }

        let pr = ws.pull_request.clone().map(|pr| {
            let (text, colour, crossed) = pull_tag(&pr);
            let url = pr.url.clone();
            div()
                .id(SharedString::from(format!("ws-pr-{wid}")))
                .flex_none()
                .text_size(px(11.))
                .text_color(colour)
                .when(crossed, Styled::line_through)
                .cursor_pointer()
                .hover(Styled::underline)
                .child(text)
                .tooltip(tip(pull_tip(&pr)))
                .on_click(move |_: &ClickEvent, _, cx| {
                    cx.stop_propagation();
                    cx.open_url(&url);
                })
        });
        let setup = ws.setup.as_deref().map(|s| match s {
            "running" => ("setup…", colours::working()),
            _ => ("setup failed", colours::failed()),
        });

        let row = div()
            .id(gid.clone())
            .group(gid.clone())
            .relative()
            .flex()
            .items_center()
            .gap(px(4.))
            .ml(px(6.))
            .pl(px(4.))
            .pr(px(4.))
            .py(px(3.))
            .rounded(px(5.))
            .border_t_2()
            .border_b_2()
            .border_color(gpui::transparent_black())
            .cursor_pointer()
            .when(selected, |d| d.bg(colours::selected()))
            .hover(|s| s.bg(colours::hover()))
            .when(selected, |d| {
                d.child(
                    div().absolute().left_0().top(px(4.)).bottom(px(4.)).w(px(2.)).rounded_full().bg(colours::accent()),
                )
            })
            .child(if rows.is_empty() {
                div().id(SharedString::from(format!("ws-fold-{wid}"))).w(px(14.)).flex_none()
            } else {
                chevron(SharedString::from(format!("ws-fold-{wid}")), !collapsed).on_click(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        this.set_folded(Target::Workspace(wid), !collapsed);
                    },
                ))
            })
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(5.))
                            .child(match self.rename_box(Target::Workspace(wid)) {
                                Some(field) => field,
                                None => div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .whitespace_nowrap()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .text_color(if selected { colours::text() } else { colours::muted() })
                                    .child(label.clone())
                                    .into_any_element(),
                            })
                            .children(pr)
                            .when(ws.behind > 0, |d| {
                                d.child(
                                    div()
                                        .flex_none()
                                        .text_size(px(11.))
                                        .text_color(colours::muted())
                                        .child(format!("↓{}", ws.behind)),
                                )
                            })
                            .when(rows.is_empty() || collapsed, |d| d.child(badge(ws.loudest()))),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(6.))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_size(px(11.))
                            .text_color(colours::faint())
                            .children(setup.map(|(text, colour)| div().text_color(colour).child(text)))
                            .children(detail),
                    ),
            )
            .child(hover_button(SharedString::from(format!("ws-menu-{wid}")), gid, "⋯", 0).on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.open_target_menu(Target::Workspace(wid), e.position, window, cx);
                }),
            ))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    this.open_target_menu(Target::Workspace(wid), e.position, window, cx)
                }),
            )
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                if e.click_count() == 2 {
                    this.start_rename(Target::Workspace(wid), window, cx);
                } else {
                    this.focus_item("workspace", wid);
                }
            }));
        let row = draggable(row, Target::Workspace(wid), label);
        let row = self.drop_target(row, Target::Workspace(wid), siblings, cx);
        let mut col = div().flex().flex_col().child(row);
        if !collapsed {
            for agent in &rows {
                col = col.child(self.render_agent(agent, shown, cx));
            }
        }
        col.into_any_element()
    }

    fn render_agent(&self, row: &AgentRow<'_>, shown: Shown, cx: &mut Context<Self>) -> AnyElement {
        let pane = row.pane;
        let id = pane.id;
        let selected = shown.pane == Some(id);
        let kind = pane.agent.clone().unwrap_or_default();
        let tab_name = self
            .session
            .as_ref()
            .and_then(|s| s.projects.iter().flat_map(|p| &p.workspaces).flat_map(|w| &w.tabs).find(|t| t.id == row.tab))
            .map(|t| t.name.clone())
            .unwrap_or_default();
        let title = agent_title(pane).unwrap_or(tab_name);
        let mut details = vec![kind];
        details.extend(pane.role.clone());
        details.extend(pane.model.clone());
        details.extend(pane.context.map(|c| format!("{c}%")));
        if pane.subagents > 0 {
            details.push(plural(pane.subagents as usize, "subagent"));
        }
        if pane.resuming {
            details.push("resuming".into());
        }
        let gid = SharedString::from(format!("agent-{id}"));
        let folded = pane.collapsed;
        div()
            .id(gid.clone())
            .group(gid.clone())
            .relative()
            .flex()
            .items_center()
            .gap(px(4.))
            .ml(px(22. + 12. * row.depth as f32))
            .px(px(4.))
            .py(px(2.))
            .rounded(px(5.))
            .cursor_pointer()
            .when(selected, |d| d.bg(colours::selected()))
            .hover(|s| s.bg(colours::hover()))
            .child(if row.children > 0 {
                chevron(SharedString::from(format!("agent-fold-{id}")), !folded).w(px(10.)).on_click(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        let verb = if folded { "expand" } else { "collapse" };
                        this.queue.cmd(cmd(&[verb, "--pane", &id.to_string()]));
                    },
                ))
            } else {
                div().id(SharedString::from(format!("agent-fold-{id}"))).w(px(10.)).flex_none()
            })
            .child(badge(pane.status.as_deref()))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .text_ellipsis()
                            .text_size(px(12.))
                            .text_color(if selected { colours::text() } else { colours::muted() })
                            .child(title),
                    )
                    .child(
                        div()
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .text_ellipsis()
                            .text_size(px(10.5))
                            .text_color(colours::faint())
                            .child(details.join(" · ")),
                    ),
            )
            .when(folded && row.children > 0, |d| d.child(ui::dim(format!("+{}", row.children))))
            .child(hover_button(SharedString::from(format!("agent-menu-{id}")), gid, "⋯", 0).on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    let entries = this.agent_menu(id);
                    this.open_menu(e.position, entries, window, cx);
                }),
            ))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    let entries = this.agent_menu(id);
                    this.open_menu(e.position, entries, window, cx);
                }),
            )
            .on_click(cx.listener(move |this, _: &ClickEvent, _, _| this.focus_item("pane", id)))
            .into_any_element()
    }

    // -----------------------------------------------------------------------------------------
    // Acting on rows.

    /// Folds or unfolds a row (`collapse` / `expand`).
    pub(crate) fn set_folded(&mut self, target: Target, collapsed: bool) {
        let verb = if collapsed { "collapse" } else { "expand" };
        self.queue.cmd(cmd(&[verb, target.flag(), &target.id().to_string()]));
    }

    /// The name a rename starts from: the custom one, else what the row shows.
    fn current_name(&self, target: Target) -> Option<String> {
        let session = self.session.as_ref()?;
        let projects = &session.projects;
        let workspaces = || projects.iter().flat_map(|p| &p.workspaces);
        match target {
            Target::Group(id) => session.groups.iter().find(|g| g.id == id).map(|g| g.name.clone()),
            Target::Project(id) => {
                projects.iter().find(|p| p.id == id).map(|p| p.custom_name.clone().unwrap_or(p.name.clone()))
            }
            Target::Workspace(id) => {
                workspaces().find(|w| w.id == id).map(|w| w.custom_name.clone().unwrap_or(w.name.clone()))
            }
            Target::Tab(id) => workspaces()
                .flat_map(|w| &w.tabs)
                .find(|t| t.id == id)
                .map(|t| t.custom_name.clone().unwrap_or(t.name.clone())),
        }
    }

    /// Turns a row's name into a text field; enter renames, escape or a click elsewhere cancels.
    pub(crate) fn start_rename(&mut self, target: Target, window: &mut Window, cx: &mut Context<Self>) {
        let Some(initial) = self.current_name(target) else { return };
        let placeholder = match target {
            Target::Group(_) => "group name",
            Target::Project(_) => "empty: the folder's name",
            Target::Workspace(_) => "empty: the branch's name",
            Target::Tab(_) => "empty: the program's name",
        };
        let field = cx.new(|cx| {
            let mut f = TextField::new(placeholder, cx);
            f.set_text(initial.clone(), cx);
            f
        });
        cx.subscribe_in(&field, window, |this, _, event, window, cx| match event {
            TextFieldEvent::Submit => this.finish_rename(true, window, cx),
            TextFieldEvent::Cancel => this.finish_rename(false, window, cx),
            TextFieldEvent::Changed => {}
        })
        .detach();
        window.focus(&field.read(cx).focus_handle(cx));
        self.side.rename = Some(Rename { target, field, initial });
        cx.notify();
    }

    pub(crate) fn finish_rename(&mut self, save: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rename) = self.side.rename.take() else { return };
        let text = rename.field.read(cx).text().trim().to_string();
        // A group needs a name; the others go back to their default with an empty one.
        let empty_group = matches!(rename.target, Target::Group(_)) && text.is_empty();
        if save && text != rename.initial && !empty_group {
            self.act(vec!["rename".into(), rename.target.flag().into(), rename.target.id().to_string(), text]);
        }
        window.focus(&self.focus);
        cx.notify();
    }

    /// A row dropped on another: `cornercase move`, with the place the drop means.
    pub(crate) fn drop_on(&mut self, dragged: Target, target: Target) {
        let Some(session) = &self.session else { return };
        let sections = order::sections(session);
        let group_ids: Vec<u64> = session.groups.iter().map(|g| g.id).collect();
        let group_of = |project: u64| session.projects.iter().find(|p| p.id == project).and_then(|p| p.group);
        let args: Option<Vec<String>> = match (dragged, target) {
            (Target::Project(p), Target::Group(g)) => {
                Some(cmd(&["move", "--project", &p.to_string(), "--into", &g.to_string()]))
            }
            (Target::Project(p), Target::Project(q)) => {
                let siblings =
                    sections.iter().find(|(_, ids)| ids.contains(&q)).map(|(_, ids)| ids.clone()).unwrap_or_default();
                order::drop_place(&siblings, p, q).map(|place| match place {
                    Place::Before(id) => cmd(&["move", "--project", &p.to_string(), "--before", &id.to_string()]),
                    Place::End => match group_of(q).filter(|g| group_ids.contains(g)) {
                        Some(g) => cmd(&["move", "--project", &p.to_string(), "--into", &g.to_string()]),
                        None => cmd(&["move", "--project", &p.to_string(), "--ungroup"]),
                    },
                })
            }
            (Target::Group(a), Target::Group(_) | Target::Project(_)) => {
                let b = match target {
                    Target::Project(q) => group_of(q).filter(|g| group_ids.contains(g)),
                    other => Some(other.id()),
                };
                b.and_then(|b| order::drop_place(&group_ids, a, b)).map(|place| match place {
                    Place::Before(id) => cmd(&["move", "--group", &a.to_string(), "--before", &id.to_string()]),
                    Place::End => cmd(&["move", "--group", &a.to_string(), "--last"]),
                })
            }
            (Target::Workspace(a), Target::Workspace(b)) => {
                // Workspaces stay in their project.
                let siblings: Vec<u64> = session
                    .projects
                    .iter()
                    .find(|p| p.workspaces.iter().any(|w| w.id == b))
                    .map(|p| p.workspaces.iter().map(|w| w.id).collect())
                    .unwrap_or_default();
                siblings.contains(&a).then(|| order::drop_place(&siblings, a, b)).flatten().map(|place| match place {
                    Place::Before(id) => cmd(&["move", "--workspace", &a.to_string(), "--before", &id.to_string()]),
                    Place::End => cmd(&["move", "--workspace", &a.to_string(), "--last"]),
                })
            }
            (Target::Tab(a), Target::Tab(b)) => {
                let siblings: Vec<u64> = session
                    .projects
                    .iter()
                    .flat_map(|p| &p.workspaces)
                    .find(|w| w.tabs.iter().any(|t| t.id == b))
                    .map(|w| w.tabs.iter().map(|t| t.id).collect())
                    .unwrap_or_default();
                siblings.contains(&a).then(|| order::drop_place(&siblings, a, b)).flatten().map(|place| match place {
                    Place::Before(id) => cmd(&["move", "--tab", &a.to_string(), "--before", &id.to_string()]),
                    Place::End => cmd(&["move", "--tab", &a.to_string(), "--last"]),
                })
            }
            _ => None,
        };
        if let Some(args) = args {
            self.queue.cmd(args);
        }
    }

    // -----------------------------------------------------------------------------------------
    // Menus.

    pub(crate) fn open_target_menu(
        &mut self,
        target: Target,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entries = self.menu_for(target);
        self.open_menu(at, entries, window, cx);
    }

    fn menu_for(&self, target: Target) -> Vec<Entry> {
        let Some(session) = &self.session else { return Vec::new() };
        let id = target.id();
        let ids = id.to_string();
        let rename =
            |label: &str| item(label.to_string(), move |this, window, cx| this.start_rename(target, window, cx));
        let fold = |collapsed: bool| {
            item(if collapsed { "Expand" } else { "Collapse" }, move |this, _, _| this.set_folded(target, !collapsed))
        };
        let finder = |path: String| {
            item("Open in Finder", move |_, _, _| {
                let _ = std::process::Command::new("open").arg(&path).spawn();
            })
        };
        let copy_path = |path: String| {
            item("Copy path", move |this, _, cx| {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(path.clone()));
                this.store
                    .update(cx, |s, cx| s.toast(format!("Copied {}", tilde(&path)), crate::store::ToastKind::Info, cx));
            })
        };
        match target {
            Target::Group(_) => {
                let Some(group) = session.groups.iter().find(|g| g.id == id) else { return Vec::new() };
                let (name, collapsed) = (group.name.clone(), group.collapsed);
                let count = session.projects.iter().filter(|p| p.group == Some(id)).count();
                let style_name = name.clone();
                vec![
                    item("Add project…", move |this, window, cx| this.open_project_into(Some(id), window, cx)),
                    Entry::Separator,
                    rename("Rename group"),
                    item("Icon and colour…", move |_, window, cx| {
                        forms::group_style(id, style_name.clone(), window, cx)
                    }),
                    fold(collapsed),
                    Entry::Separator,
                    danger("Delete group…", move |_, window, cx| {
                        forms::delete_group(id, name.clone(), count, window, cx)
                    }),
                ]
            }
            Target::Project(_) => {
                let Some(project) = session.projects.iter().find(|p| p.id == id) else { return Vec::new() };
                let (name, path, collapsed) = (project.name.clone(), project.path.clone(), project.collapsed);
                let mut groups: Vec<Entry> = session
                    .groups
                    .iter()
                    .filter(|g| Some(g.id) != project.group)
                    .map(|g| {
                        let label = format!("{} {}", g.icon.clone().unwrap_or_default(), g.name);
                        let to = g.id.to_string();
                        item(label, {
                            let ids = ids.clone();
                            move |this, _, _| this.queue.cmd(cmd(&["move", "--project", &ids, "--into", &to]))
                        })
                    })
                    .collect();
                if project.group.is_some() {
                    groups.push(item("No group", {
                        let ids = ids.clone();
                        move |this, _, _| this.queue.cmd(cmd(&["move", "--project", &ids, "--ungroup"]))
                    }));
                }
                groups.push(Entry::Separator);
                groups.push(item("New group…", move |_, window, cx| forms::new_group(Some(id), window, cx)));
                vec![
                    item("New workspace…", move |_, window, cx| dialogs::new_workspace(Some(id), window, cx)),
                    act("Start agent here", cmd(&["start", "--project", &ids, "--focus"])),
                    Entry::Separator,
                    rename("Rename project"),
                    submenu("Move to group", groups),
                    fold(collapsed),
                    Entry::Separator,
                    finder(path.clone()),
                    copy_path(path),
                    Entry::Separator,
                    danger("Close project…", move |_, window, cx| {
                        dialogs::confirm(
                            format!("Close the project {name}?"),
                            "Every shell in it stops, agents included. Its folder and worktrees stay on disk.",
                            "Close project",
                            cmd(&["close", "--project", &id.to_string()]),
                            false,
                            window,
                            cx,
                        );
                    }),
                ]
            }
            Target::Workspace(_) => {
                let Some((_, ws)) = session.workspace(id) else { return Vec::new() };
                let (name, path, collapsed, worktree) = (ws.name.clone(), ws.path.clone(), ws.collapsed, ws.worktree);
                let mut entries = vec![
                    act("New tab here", cmd(&["new-tab", "--workspace", &ids, "--focus"])),
                    act("Start agent here", cmd(&["start", "--workspace", &ids, "--focus"])),
                ];
                if let Some(n) = ws.closed_agents.filter(|n| *n > 0) {
                    entries.push(item(format!("Reopen {}", plural(n, "closed agent")), {
                        let ids = ids.clone();
                        move |this, _, _| this.queue.cmd(cmd(&["agents", "reopen", "--workspace", &ids]))
                    }));
                }
                entries.push(Entry::Separator);
                entries.push(rename("Rename workspace"));
                if let Some(pr) = &ws.pull_request {
                    let (url, copy) = (pr.url.clone(), pr.url.clone());
                    entries.push(submenu(
                        format!("Pull request #{}", pr.number),
                        vec![
                            Entry::Note(SharedString::from(pr.title.clone())),
                            item("Open in browser", move |_, _, cx| cx.open_url(&url)),
                            item("Copy link", move |this, _, cx| {
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(copy.clone()));
                                this.store
                                    .update(cx, |s, cx| s.toast("Copied the link", crate::store::ToastKind::Info, cx));
                            }),
                        ],
                    ));
                }
                if !ws.agent_rows().is_empty() {
                    entries.push(fold(collapsed));
                }
                entries.extend([Entry::Separator, finder(path.clone()), copy_path(path), Entry::Separator]);
                if worktree {
                    let n = name.clone();
                    entries.push(danger("Remove worktree…", move |_, window, cx| {
                        forms::remove_worktree(id, n.clone(), window, cx)
                    }));
                }
                if !ws.tabs.is_empty() || !worktree {
                    let label = if worktree { "Close its tabs…" } else { "Close workspace…" };
                    let message = if worktree {
                        "Every tab in it stops, agents included. The worktree stays on disk and listed."
                    } else {
                        "Every tab in it stops, agents included. Its folder stays on disk."
                    };
                    entries.push(danger(label, move |_, window, cx| {
                        dialogs::confirm(
                            format!("Close {name}?"),
                            message,
                            "Close",
                            cmd(&["close", "--workspace", &id.to_string()]),
                            false,
                            window,
                            cx,
                        );
                    }));
                }
                entries
            }
            Target::Tab(_) => self.tab_menu(id),
        }
    }

    /// The menu of an agent's row (and of an agent's pane): focus, review marks, folds, close.
    pub(crate) fn agent_menu(&self, pane: u64) -> Vec<Entry> {
        let Some(info) = self.session.as_ref().and_then(|s| s.pane(pane)) else { return Vec::new() };
        let p = pane.to_string();
        let mut entries = vec![act("Show", cmd(&["focus", "--pane", &p]))];
        if info.status.as_deref() == Some("pending") {
            entries.push(act("Mark as reviewed", cmd(&["mark", "--pane", &p, "--reviewed"])));
        } else {
            entries.push(act("Mark as pending review", cmd(&["mark", "--pane", &p])));
        }
        let has_children = self
            .session
            .as_ref()
            .and_then(|s| {
                s.projects
                    .iter()
                    .flat_map(|p| &p.workspaces)
                    .find_map(|w| w.agent_rows().into_iter().find(|r| r.pane.id == pane))
            })
            .is_some_and(|r| r.children > 0);
        if has_children {
            let verb = if info.collapsed { "expand" } else { "collapse" };
            let label = if info.collapsed { "Expand" } else { "Collapse" };
            let p2 = p.clone();
            entries.push(item(label, move |this, _, _| this.queue.cmd(cmd(&[verb, "--pane", &p2]))));
        }
        entries.push(Entry::Separator);
        entries.push(danger("Close agent…", move |_, window, cx| close_agent(pane, window, cx)));
        entries
    }

    // -----------------------------------------------------------------------------------------
    // Opening projects.

    pub(crate) fn open_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_project_into(None, window, cx);
    }

    /// Picks a folder and opens it as a project (`open PATH --focus`), into a group when given.
    pub(crate) fn open_project_into(&mut self, group: Option<u64>, _window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(if group.is_some() { "Add project".into() } else { "Open project".into() }),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.first()
            {
                let path = path.to_string_lossy().into_owned();
                let _ = this.update(cx, |this, cx| match group {
                    None => this.act(vec!["open".into(), path, "--focus".into()]),
                    Some(group) => {
                        let mut args = vec!["open".into(), path, "--focus".into(), "--json".into()];
                        if let Some(view) = this.view_id {
                            args.extend(["--view".into(), view.to_string()]);
                        }
                        crate::store::run_json(cx, args, move |this: &mut Root, result, cx| match result {
                            Ok(v) => {
                                if let Some(project) = v.get("project").and_then(serde_json::Value::as_u64) {
                                    this.queue.cmd(cmd(&[
                                        "move",
                                        "--project",
                                        &project.to_string(),
                                        "--into",
                                        &group.to_string(),
                                    ]));
                                    this.set_folded(Target::Group(group), false);
                                }
                            }
                            Err(e) => this.say(e.to_string(), cx),
                        });
                    }
                });
            }
        })
        .detach();
    }
}

/// `["a", "b"]` as owned arguments.
pub(crate) fn cmd(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| (*s).to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pull_request_tags_follow_state_and_checks() {
        let pr = |state: &str, checks: Option<&str>| PullRequest {
            number: 485,
            state: state.into(),
            checks: checks.map(String::from),
            ..Default::default()
        };
        assert_eq!(pull_tag(&pr("open", Some("failing"))).0, "#485✗");
        assert_eq!(pull_tag(&pr("draft", Some("pending"))).0, "#485•");
        assert_eq!(pull_tag(&pr("open", Some("passing"))).0, "#485✓");
        assert_eq!(pull_tag(&pr("open", None)).0, "#485");
        // Merged and closed ones drop the glyph; closed is crossed out.
        assert_eq!(pull_tag(&pr("merged", Some("failing"))).0, "#485");
        assert!(pull_tag(&pr("closed", None)).2);
    }

    #[test]
    fn agent_titles_lose_their_spinner() {
        let pane = |t: &str| PaneInfo { title: Some(t.into()), ..Default::default() };
        assert_eq!(agent_title(&pane("✳ Fix the login")).as_deref(), Some("Fix the login"));
        assert_eq!(agent_title(&pane("⠋ ")), None);
    }
}
