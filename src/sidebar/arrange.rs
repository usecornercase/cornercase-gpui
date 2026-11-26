use std::rc::Rc;

use gpui::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement, Stateful,
    StatefulInteractiveElement, Styled, div, px,
};

use super::rows::{Listing, list_text};
use super::sections::{AgentsRoom, Room};
use super::{Arrangement, card, list_end, sidebar_footer, tip};
use crate::app::{GAP, Root};
use crate::model::{MachineSeen, Section, SectionFolds, Session, Shown, ThereProject};
use crate::theme::colours;
use crate::ui::split::{self, Edge, Split};
use crate::ui::{self, Icon};

const PROJECTS_SHARE: f32 = 0.42;
const PROJECTS_MIN: f32 = 140.;
const PROJECTS_MAX: f32 = 260.;
const STACK: Split = Split::new(240., 90., f32::INFINITY);
const STACK_KEEP: f32 = 90.;
const AGENTS: Split = Split::new(200., 80., f32::INFINITY);
const LISTS_KEEP: f32 = 120.;

struct StackGrip;
struct AgentsGrip;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Sizes {
    pub(super) stack: Split,
    agents: Split,
}

impl Default for Sizes {
    fn default() -> Self {
        Self { stack: STACK, agents: AGENTS }
    }
}

fn stack_height(root: &mut Root) -> &mut Split {
    &mut root.side.sizes.stack
}

fn agents_height(root: &mut Root) -> &mut Split {
    &mut root.side.sizes.agents
}

fn scroller(id: &'static str) -> Stateful<Div> {
    div().id(id).flex_1().min_h_0().flex().flex_col().overflow_y_scroll()
}

fn rows_column() -> Div {
    list_text(div().flex().flex_col().gap(px(1.)).px(px(10.)))
}

fn list_box(open: bool) -> Div {
    let list = div().flex().flex_col();
    if open { list.flex_1().min_h_0() } else { list.flex_none() }
}

const DIVIDER_TOP: f32 = 6.;

pub(super) fn divider() -> Div {
    div().h(px(1.)).flex_none().mx(px(12.)).mt(px(DIVIDER_TOP)).bg(colours::border())
}

fn new_workspace_button(project: Option<u64>, cx: &mut Context<Root>) -> Stateful<Div> {
    ui::tool_button("sidebar-new-workspace", Icon::Plus, 24., false)
        .tooltip(tip("New workspace or flow in this project"))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                this.open_new_menu(project, e.position, window, cx);
            }),
        )
}

impl Root {
    pub(super) fn sidebar_cards(&self, cx: &mut Context<Self>) -> Div {
        let cards = div().flex_1().min_h_0().flex().flex_row().gap(px(GAP));
        if self.side.bots {
            return cards.child(self.bots_card(cx));
        }
        let look = &self.side.look.current;
        let arrangement = look.arrangement;
        let folds = self.section_folds(cx).unwrap_or_default();
        let room = Room::of(arrangement, folds, look.agents_section);
        if arrangement == Arrangement::SideBySide {
            return self.side_by_side(cards, folds, room, cx);
        }
        let lists = self.lists(arrangement, folds, room, cx);
        cards.child(self.main_card(lists, room, cx))
    }

    fn side_by_side(&self, cards: Div, folds: SectionFolds, room: Room, cx: &mut Context<Self>) -> Div {
        let width = px(self.projects_column());
        let projects = self.projects_section(folds.projects, cx);
        let column = card().flex_none().flex_basis(width).w(width).min_h_0().child(projects);
        let workspaces = self.workspaces_section(folds.workspaces, cx);
        cards.child(column).child(self.main_card(workspaces, room, cx))
    }

    fn lists(&self, arrangement: Arrangement, folds: SectionFolds, room: Room, cx: &mut Context<Self>) -> Div {
        let projects = self.projects_section(folds.projects, cx);
        match arrangement {
            Arrangement::Tree | Arrangement::SideBySide => projects,
            Arrangement::ProjectsOnTop => {
                let workspaces = self.workspaces_section(folds.workspaces, cx);
                self.stack((projects, folds.projects), (workspaces, folds.workspaces), room.stack_drags, cx)
            }
            Arrangement::WorkspacesOnTop => {
                let workspaces = self.workspaces_section(folds.workspaces, cx);
                self.stack((workspaces, folds.workspaces), (projects, folds.projects), room.stack_drags, cx)
            }
        }
    }

    fn stack(&self, top: (Div, bool), bottom: (Div, bool), drags: bool, cx: &mut Context<Self>) -> Div {
        let ((top, top_folded), (bottom, bottom_folded)) = (top, bottom);
        let top = match self.side.sizes.stack.chosen() {
            Some(height) if drags => top.flex_none().flex_basis(px(height)),
            _ => top,
        };
        let bottom = list_box(!bottom_folded).relative().child(divider()).child(bottom);
        let stack = list_box(!(top_folded && bottom_folded));
        if !drags {
            return stack.child(top).child(bottom);
        }
        let handle = split::handle("sidebar-stack-split", StackGrip, (Edge::Bottom, DIVIDER_TOP), stack_height, cx);
        stack
            .on_drag_move(split::follow::<_, StackGrip>(Edge::Bottom, STACK_KEEP, stack_height, cx))
            .child(top)
            .child(bottom.child(handle))
    }

    fn main_card(&self, lists: Div, room: Room, cx: &mut Context<Self>) -> Div {
        card().min_w_0().min_h_0().child(self.with_agents(lists, room.agents, cx)).child(sidebar_footer(cx))
    }

    fn with_agents(&self, lists: Div, room: AgentsRoom, cx: &mut Context<Self>) -> Div {
        let area = div().flex_1().min_h_0().relative().flex().flex_col();
        if room == AgentsRoom::Hidden {
            return area.child(lists);
        }
        let Some(agents) = self.agents_section(room == AgentsRoom::Title, cx) else { return area.child(lists) };
        match room {
            AgentsRoom::Split => self.split_agents(area.child(lists), agents, cx),
            AgentsRoom::Title => area.child(list_box(true).child(lists)).child(agents.flex_none()),
            AgentsRoom::Rest | AgentsRoom::Hidden => area.child(lists).child(agents.flex_1().min_h_0()),
        }
    }

    fn split_agents(&self, area: Div, agents: Div, cx: &mut Context<Self>) -> Div {
        let height = self.side.sizes.agents.size();
        let line = (Edge::Top, height - DIVIDER_TOP);
        area.on_drag_move(split::follow::<_, AgentsGrip>(Edge::Top, LISTS_KEEP, agents_height, cx))
            .child(agents.flex_none().h(px(height)))
            .child(split::handle("sidebar-agents-split", AgentsGrip, line, agents_height, cx))
    }

    pub(super) fn main_card_width(&self) -> f32 {
        if self.side.look.current.arrangement == Arrangement::SideBySide {
            self.pair_width.size() - GAP - self.projects_column()
        } else {
            self.sidebar_width.size()
        }
    }

    fn projects_column(&self) -> f32 {
        ((self.pair_width.size() - GAP) * PROJECTS_SHARE).clamp(PROJECTS_MIN, PROJECTS_MAX).round()
    }

    fn listing(&self, session: &Session) -> Listing {
        let look = &self.side.look.current;
        let place = self.side.machine_place.clone();
        let shown = match &place {
            Some(place) => Shown { project: session.pair_of(place), ..Shown::default() },
            None if self.side.bots => Shown { project: session.shown().project, ..Shown::default() },
            None => session.shown(),
        };
        Listing {
            shown,
            groups: Rc::new(session.groups.iter().map(|g| g.id).collect()),
            tree: look.arrangement == Arrangement::Tree,
            tabs: look.tabs_in_sidebar,
            place,
            now: crate::panels::queue::now(),
            counts: look.counts,
            spots: self.parts.spots.clone(),
        }
    }

    fn projects_section(&self, folded: bool, cx: &mut Context<Self>) -> Div {
        let list = list_box(!folded).child(self.sidebar_header(cx));
        list.children((!folded).then(|| self.projects_list(cx)))
    }

    fn projects_list(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        scroller("sidebar-list").child(self.project_rows(cx)).child(list_end(cx))
    }

    fn project_rows(&self, cx: &mut Context<Self>) -> Div {
        let Some(session) = self.session.as_ref() else { return rows_column() };
        let listing = self.listing(session);
        let mut rows = Vec::new();
        for section in session.sections() {
            rows.extend(self.section_rows(session, section, &listing, cx));
        }
        rows.extend(self.alone_project_rows(session, &listing, cx));
        rows_column().children(rows)
    }

    fn workspaces_section(&self, folded: bool, cx: &mut Context<Self>) -> Div {
        let section = list_box(!folded).child(self.workspaces_header(cx));
        section.children((!folded).then(|| scroller("sidebar-workspaces").child(self.workspace_rows(cx))))
    }

    fn workspaces_header(&self, cx: &mut Context<Self>) -> Div {
        if let Some((_, there)) = self.alone_shown() {
            return self.titled(Section::Workspaces, there.tags(), None, cx);
        }
        let session = self.session.as_ref();
        let project = session.and_then(|s| self.listing(s).shown.project);
        let tags = project.zip(session).map(|(id, s)| s.workspaces_tags(id)).unwrap_or_default();
        let button = new_workspace_button(project, cx);
        self.titled(Section::Workspaces, tags, Some(button), cx)
    }

    fn alone_shown(&self) -> Option<(&MachineSeen, &ThereProject)> {
        self.session.as_ref()?.alone_of(self.side.machine_place.as_ref()?)
    }

    fn workspace_rows(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(session) = self.session.as_ref() else { return rows_column().into_any_element() };
        let listing = self.listing(session);
        if let Some((seen, there)) = self.alone_shown() {
            let rows = there.workspaces.iter().map(|ws| self.there_workspace(seen, ws, &listing, cx));
            return rows_column().children(rows.collect::<Vec<_>>()).into_any_element();
        }
        let Some(project) = listing.shown.project.and_then(|id| session.projects.iter().find(|p| p.id == id)) else {
            return ui::note("No project shown.").into_any_element();
        };
        let siblings: Rc<Vec<u64>> = Rc::new(project.workspaces.iter().map(|w| w.id).collect());
        let mut rows: Vec<AnyElement> =
            project.workspaces.iter().map(|ws| self.render_workspace(ws, &listing, siblings.clone(), cx)).collect();
        rows.extend(self.there_rows_under(project.id, &listing, cx));
        rows_column().children(rows).into_any_element()
    }
}
