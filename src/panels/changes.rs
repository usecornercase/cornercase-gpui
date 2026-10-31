mod actions;
mod diff;
mod diff_view;
mod file_list;
mod follow;
mod header;
mod open_view;
mod pieces;
mod selection;
mod wire;

use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    AnyView, App, AppContext as _, Context, Div, Hsla, InteractiveElement as _, IntoElement, ListAlignment, ListState,
    ParentElement as _, Render, Styled as _, Window, div, prelude::FluentBuilder as _, px, relative,
};

use crate::cli::Stream;
use crate::model::Session;
use crate::store::Store;
use crate::ui::split::{self, Edge, Split};

use diff::Row;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Follow {
    scope: Scope,
    workspace: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Selected {
    workspace: u64,
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
}

pub(crate) struct ChangesPanel {
    scope: Scope,
    project_mode: Mode,
    shown: Option<u64>,
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
    files_height: Split,
}

impl ChangesPanel {
    pub(crate) fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = Store::get(cx);
        cx.observe(&store, |this: &mut Self, _, cx| this.sync(cx)).detach();
        let mut panel = Self {
            scope: Scope::Project(Mode::Uncommitted),
            project_mode: Mode::Uncommitted,
            shown: None,
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
            files_height: FILES,
        };
        panel.sync(cx);
        panel
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let session = Store::get(cx).read(cx).session.clone();
        let session = session.as_ref();
        div()
            .size_full()
            .flex()
            .flex_col()
            .text_size(px(12.))
            .child(self.render_header(session, cx))
            .child(self.render_body(session, cx))
            .child(self.render_footer())
    }
}

pub(crate) fn build(window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| ChangesPanel::new(window, cx)).into()
}
