mod actions;
mod diff;
mod diff_view;
mod file_list;
mod filter;
mod follow;
mod header;
mod lines;
mod open_view;
mod pieces;
mod selection;
mod wire;

use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    AnyView, App, AppContext as _, Context, Div, Entity, Hsla, InteractiveElement as _, IntoElement, ListAlignment,
    ListOffset, ListState, ParentElement as _, Render, ScrollHandle, Styled as _, Window, div, point,
    prelude::FluentBuilder as _, px, relative,
};

use crate::cli::Stream;
use crate::model::{Aim, Session};
use crate::panels::on_machine;
use crate::store::{MachineRead, Store};
use crate::ui::counting::Counting;
use crate::ui::selection::Selecting;
use crate::ui::split::{self, Edge, Split};
use crate::ui::{TextField, TextFieldEvent};

use diff::{Row, Spot};
use filter::Query;
pub(crate) use pieces::status_letter;
pub(crate) use wire::FileStatus;
use wire::{Base, Gap, Mode, Patch, Summary};

const FILES: Split = Split::new(60., 60., f32::INFINITY);
const DIFF_KEEP: f32 = 120.;

struct FilesGrip;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    Project(Mode),
    Every,
}

impl Scope {
    fn mode(self) -> Mode {
        match self {
            Self::Project(mode) => mode,
            Self::Every => Mode::Uncommitted,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Whose {
    Here(u64),
    There(Aim, String),
}

impl Whose {
    fn arg(&self) -> String {
        match self {
            Self::Here(id) => id.to_string(),
            Self::There(aim, _) => aim.workspace_arg(),
        }
    }

    fn holds(&self, listed: u64) -> bool {
        match self {
            Self::Here(id) => *id == listed,
            Self::There(..) => true,
        }
    }

    fn here(&self) -> Option<u64> {
        match self {
            Self::Here(id) => Some(*id),
            Self::There(..) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Follow {
    scope: Scope,
    workspace: Option<Whose>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Selected {
    workspace: Whose,
    path: String,
    mode: Mode,
}

#[derive(Debug, Clone, Copy)]
struct Look {
    fg: Hsla,
    bold: bool,
    italic: bool,
}

type Gaps = HashMap<usize, (Gap, Vec<Look>)>;

struct DiffData {
    selected: Selected,
    patch: Patch,
    looks: Vec<Look>,
    gaps: Gaps,
    rows: Vec<Row>,
}

#[derive(Default)]
struct Open {
    digest: String,
    data: Option<Rc<DiffData>>,
    loading: bool,
    error: Option<String>,
    generation: u64,
    selecting: Selecting<Spot>,
}

pub(crate) struct ChangesPanel {
    scope: Scope,
    project_mode: Mode,
    shown: Option<Whose>,
    on_machine: Option<MachineRead>,
    hearing: follow::Hearing,
    follow: Option<Follow>,
    stream: Option<Stream>,
    stream_generation: u64,
    summary: Option<Summary>,
    ended: Option<String>,
    selected: Option<Selected>,
    open: Open,
    base: Option<Base>,
    base_open: bool,
    diff_list: ListState,
    wraps: bool,
    files_height: Split,
    counting: Counting,
    filter: Entity<TextField>,
    files_scroll: ScrollHandle,
}

impl ChangesPanel {
    pub(crate) fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = Store::get(cx);
        cx.observe(&store, |this: &mut Self, _, cx| this.sync(cx)).detach();
        let filter = cx.new(|cx| TextField::new("Filter by path: order, *.test.js, !*.lock", cx).clears_on_escape());
        cx.subscribe(&filter, |this, _, event: &TextFieldEvent, cx| this.filter_event(event, cx)).detach();
        let mut panel = Self {
            scope: Scope::Project(Mode::Uncommitted),
            project_mode: Mode::Uncommitted,
            shown: None,
            on_machine: None,
            hearing: follow::Hearing::default(),
            follow: None,
            stream: None,
            stream_generation: 0,
            summary: None,
            ended: None,
            selected: None,
            open: Open::default(),
            base: None,
            base_open: false,
            diff_list: ListState::new(0, ListAlignment::Top, px(400.)),
            wraps: true,
            files_height: FILES,
            counting: Counting::default(),
            filter,
            files_scroll: ScrollHandle::new(),
        };
        panel.sync(cx);
        panel
    }

    fn filter_event(&mut self, event: &TextFieldEvent, cx: &mut Context<Self>) {
        match event {
            TextFieldEvent::Changed => self.files_scroll.set_offset(point(px(0.), px(0.))),
            TextFieldEvent::Submit => Store::get(cx).update(cx, Store::give_keys_back),
            TextFieldEvent::Cancel => {}
        }
        cx.notify();
    }

    fn query(&self, cx: &App) -> Query {
        Query::parse(&self.filter.read(cx).text())
    }

    fn follow_wrap(&mut self, cx: &App) {
        let wraps = Store::get(cx).read(cx).prefs.wraps();
        if wraps == self.wraps {
            return;
        }
        self.wraps = wraps;
        let top = self.diff_list.logical_scroll_top();
        self.diff_list.reset(self.open.data.as_ref().map_or(0, |data| data.rows.len()));
        self.diff_list.scroll_to(ListOffset { item_ix: top.item_ix, offset_in_item: px(0.) });
    }

    fn reads_machine(&self) -> bool {
        self.machine_followed().is_some()
    }

    fn machine_shown(&self) -> Option<&MachineRead> {
        self.on_machine.as_ref().filter(|_| matches!(self.scope, Scope::Project(_)))
    }

    fn machine_note(&self) -> Option<on_machine::Note> {
        on_machine::note(self.machine_shown()?, &self.hearing.retry)
    }

    fn whose_row(&self, listed: u64) -> Whose {
        follow::whose_listed(self.follow.as_ref(), listed)
    }
}

fn files_box(selected: bool, height: Option<f32>) -> Div {
    let frame = match (selected, height) {
        (false, _) => div().flex_1().min_h(px(0.)),
        (true, None) => div().flex_none().max_h(relative(0.36)).min_h(px(60.)),
        (true, Some(height)) => div().flex_none().h(px(height)),
    };
    frame.flex().flex_col()
}

fn counting(panel: &mut ChangesPanel) -> &mut Counting {
    &mut panel.counting
}

fn files_height(panel: &mut ChangesPanel) -> &mut Split {
    &mut panel.files_height
}

impl ChangesPanel {
    fn render_body(&self, session: Option<&Session>, cx: &mut Context<Self>) -> Div {
        let selected = self.selected.clone();
        let files = self.render_files(session, cx);
        div()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .flex_col()
            .on_drag_move(split::follow::<_, FilesGrip>(Edge::Bottom, DIFF_KEEP, files_height, cx))
            .child(files_box(selected.is_some(), self.files_height.chosen()).child(files))
            .when_some(selected, |d, selected| d.child(self.render_open(&selected, cx)))
    }
}

impl Render for ChangesPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.follow_wrap(cx);
        let session = Store::get(cx).read(cx).session.clone();
        let session = session.as_ref();
        let panel = div()
            .size_full()
            .flex()
            .flex_col()
            .text_size(px(12.))
            .children(self.machine_shown().map(|read| {
                let offer = Store::get(cx).read(cx).update_offer(read.reach.older());
                on_machine::strip(read, &self.hearing.retry, &self.counting, (true, &offer))
            }))
            .child(self.render_header(session, window, cx))
            .child(self.render_body(session, cx))
            .child(self.render_footer());
        self.counting.keep(counting, cx);
        panel
    }
}

pub(crate) fn build(window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| ChangesPanel::new(window, cx)).into()
}
