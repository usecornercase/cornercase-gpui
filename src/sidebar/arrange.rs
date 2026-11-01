use std::rc::Rc;

use gpui::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement, Stateful,
    StatefulInteractiveElement, Styled, div, px,
};

use super::rows::{Listing, list_text};
use super::{Arrangement, card, list_end, section_head, sidebar_footer, sidebar_header, tip};
use crate::app::{GAP, Root};
use crate::model::Session;
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

pub(crate) struct Sizes {
    stack: Split,
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

const DIVIDER_TOP: f32 = 6.;

pub(super) fn divider() -> Div {
    div().h(px(1.)).flex_none().mx(px(12.)).mt(px(DIVIDER_TOP)).bg(colours::border())
}

fn workspaces_header(project: Option<u64>, cx: &mut Context<Root>) -> Div {
    section_head(
        "Workspaces",
        ui::tool_button("sidebar-new-workspace", Icon::Plus, 24., false)
            .tooltip(tip("New workspace or flow in this project"))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.open_new_menu(project, e.position, window, cx);
                }),
            ),
    )
}

impl Root {
    pub(super) fn sidebar_cards(&self, cx: &mut Context<Self>) -> Div {
        let cards = div().flex_1().min_h_0().flex().flex_row().gap(px(GAP));
        let projects =
            div().flex_1().min_h_0().flex().flex_col().child(sidebar_header(cx)).child(self.projects_list(cx));
        let lists = match self.side.look.current.arrangement {
            Arrangement::Tree => projects,
            Arrangement::ProjectsOnTop => self.stack(projects, self.workspaces_section(cx), cx),
            Arrangement::WorkspacesOnTop => self.stack(self.workspaces_section(cx), projects, cx),
            Arrangement::SideBySide => {
                let column = card().flex_none().w(px(self.projects_column())).min_h_0().child(projects);
                return cards.child(column).child(self.main_card(self.workspaces_section(cx), cx));
            }
        };
        cards.child(self.main_card(lists, cx))
    }

    fn stack(&self, top: Div, bottom: Div, cx: &mut Context<Self>) -> Div {
        let top = match self.side.sizes.stack.chosen() {
            Some(height) => top.flex_none().flex_basis(px(height)),
            None => top,
        };
        let bottom =
            div().flex_1().min_h_0().relative().flex().flex_col().child(divider()).child(bottom).child(split::handle(
                "sidebar-stack-split",
                StackGrip,
                (Edge::Bottom, DIVIDER_TOP),
                stack_height,
                cx,
            ));
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .on_drag_move(split::follow::<_, StackGrip>(Edge::Bottom, STACK_KEEP, stack_height, cx))
            .child(top)
            .child(bottom)
    }

    fn main_card(&self, lists: Div, cx: &mut Context<Self>) -> Div {
        card().min_w_0().min_h_0().child(self.with_agents(lists, cx)).child(sidebar_footer(cx))
    }

    fn with_agents(&self, lists: Div, cx: &mut Context<Self>) -> Div {
        let area = div().flex_1().min_h_0().relative().flex().flex_col().child(lists);
        let Some(agents) = self.agents_section(cx) else { return area };
        let height = self.side.sizes.agents.size();
        area.on_drag_move(split::follow::<_, AgentsGrip>(Edge::Top, LISTS_KEEP, agents_height, cx))
            .child(agents.h(px(height)))
            .child(split::handle(
                "sidebar-agents-split",
                AgentsGrip,
                (Edge::Top, height - DIVIDER_TOP),
                agents_height,
                cx,
            ))
    }

    fn projects_column(&self) -> f32 {
        ((self.pair_width.size() - GAP) * PROJECTS_SHARE).clamp(PROJECTS_MIN, PROJECTS_MAX).round()
    }

    fn listing(&self, session: &Session) -> Listing {
        let look = &self.side.look.current;
        Listing {
            shown: session.shown(),
            groups: Rc::new(session.groups.iter().map(|g| g.id).collect()),
            tree: look.arrangement == Arrangement::Tree,
            tabs: look.tabs_in_sidebar,
        }
    }

    fn projects_list(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        scroller("sidebar-list").child(self.project_rows(cx)).child(list_end(cx))
    }

    fn project_rows(&self, cx: &mut Context<Self>) -> Div {
        let Some(session) = &self.session else { return rows_column() };
        let listing = self.listing(session);
        let mut rows = Vec::new();
        for section in session.sections() {
            rows.extend(self.section_rows(session, section, &listing, cx));
        }
        rows_column().children(rows)
    }

    fn workspaces_section(&self, cx: &mut Context<Self>) -> Div {
        let project = self.session.as_ref().and_then(|s| s.shown().project);
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(workspaces_header(project, cx))
            .child(scroller("sidebar-workspaces").child(self.workspace_rows(cx)))
    }

    fn workspace_rows(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(session) = &self.session else { return rows_column().into_any_element() };
        let listing = self.listing(session);
        let Some(project) = listing.shown.project.and_then(|id| session.projects.iter().find(|p| p.id == id)) else {
            return ui::note("No project shown.").into_any_element();
        };
        let siblings: Rc<Vec<u64>> = Rc::new(project.workspaces.iter().map(|w| w.id).collect());
        let rows = project.workspaces.iter().map(|ws| self.render_workspace(ws, &listing, siblings.clone(), cx));
        rows_column().children(rows.collect::<Vec<_>>()).into_any_element()
    }
}
