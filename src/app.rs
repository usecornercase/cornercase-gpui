mod connection;
mod dock;
pub(crate) mod frame;
pub(crate) mod keeper;
mod looks;
pub(crate) mod machine;
pub(crate) mod placement;
mod status;
mod surface;
mod toasts;
mod widths;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use futures::StreamExt as _;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui::{
    AnyElement, AnyView, App, AppContext as _, ClickEvent, ClipboardItem, Context, Div, ElementId, Entity, FocusHandle,
    Focusable, InteractiveElement, IntoElement, MouseButton, ParentElement, Render, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled, Window, div, prelude::FluentBuilder as _, px,
};

use crate::cli::{self, Queue, Target};
use crate::desktop::{self, Board};
use crate::model::{Frame, Project, Session, Workspace};
use crate::notify::{self, Notifier};
use crate::panels::{PanelKind, plugins};
use crate::platform::Os;
use crate::render::Metrics;
use crate::render::layout::{Along, Layout};
use crate::render::moving::Moves;
use crate::self_update;
use crate::sidebar::{Arrangement, Fold, SideState};
use crate::store::{Store, ToastKind};
use crate::terminal::Pointer;
use crate::theme::{self, colours, radius};
use crate::ui;
use crate::ui::counting::Counting;
use crate::ui::motion::{self, Leaving, Way};
use crate::ui::split::{self, Edge, Split};
pub(crate) use connection::Connection;
use dock::{dock_card, over_the_terminal};
use machine::MachineTab;

pub(crate) const TITLEBAR_HEIGHT: f32 = 44.;
pub(crate) const GAP: f32 = 8.;
const SIDEBAR: Split = Split::new(256., 200., 560.);
const PAIR: Split = Split::new(456., 360., 760.);
const DOCK: Split = Split::new(428., 308., 1208.);
const WIDE_DOCK: Split = Split::new(720., 420., 4096.);
const DOCKS: DockWidths = DockWidths { panel: DOCK, browser: WIDE_DOCK, flow: WIDE_DOCK };
const DOCK_HEADER: f32 = 42.;
const TERMINAL_KEEP: f32 = 240.;
const DOCK_SLIDE: f32 = 24.;
const SHEET_RISE: f32 = 14.;
const DIALOG_RISE: f32 = 10.;
const RETRY: Duration = Duration::from_millis(1500);

pub(crate) enum Msg {
    Started(cli::Start),
    Missing(cli::Missing),
    Stream(u64, Heard),
    Error(String),
    Copied(Board, String),
    Elsewhere(String),
    Folded(Fold, bool, Option<String>),
    ViewMoved,
}

pub(crate) enum Heard {
    Line(Box<Session>),
    ViewEnded(String),
    Frame(Box<Frame>),
    SurfaceEnded(String),
    QueueError(String),
    FocusRefused(String),
    Copied(String, Option<u64>),
    Bell(Option<u64>),
}

#[derive(Clone, PartialEq)]
pub(crate) enum Conn {
    Starting,
    Connected,
    Lost(String),
    Missing(cli::Missing),
    Older(cli::Older),
    Behind(cli::Behind),
}

pub(crate) enum Drag {
    Program { pane: u64, button: MouseButton, motion: bool, cell: (u16, u16) },
    Select { pane: u64, from: (u16, u16), to: (u16, u16) },
    Divider { tab: u64, path: Vec<bool>, down: bool, along: Along },
    Link { pane: u64, cell: (u16, u16) },
    Menu { pane: u64 },
}

struct SidebarGrip;

struct DockGrip;

pub(crate) struct Root {
    pub(crate) focus: FocusHandle,
    pub(crate) store: Entity<Store>,
    pub(crate) tx: UnboundedSender<Msg>,
    pub(crate) own: Connection,
    pub(crate) machine: Option<MachineTab>,
    pub(crate) session: Option<Session>,
    pub(crate) metrics: Option<Metrics>,
    pub(crate) font_size: f32,
    pub(crate) layout: Rc<RefCell<Layout>>,
    pub(crate) drag: Option<Drag>,
    pub(crate) pointer: Pointer,
    pub(crate) moves: Rc<RefCell<Moves>>,
    pub(crate) marked: String,
    pub(crate) conn: Conn,
    pub(crate) host: Option<String>,
    attempts: u32,
    pub(crate) came_up: bool,
    pub(crate) updated_there: Option<cli::Older>,
    pub(crate) answered: Option<(String, cli::Said)>,
    place: Vec<String>,
    pub(crate) side: SideState,
    pub(crate) visible: bool,
    window_active: bool,
    pub(crate) panels: HashMap<PanelKind, AnyView>,
    pub(crate) sidebar_width: Split,
    pub(crate) pair_width: Split,
    dock_widths: DockWidths,
    had: Overlays,
    leaving_sheet: Leaving<PanelKind>,
    leaving_dialog: Leaving<AnyView>,
    leaving_dock: Leaving<PanelKind>,
    notifier: Entity<Notifier>,
    title: String,
    pub(crate) dock_focus: FocusHandle,
    beat: Rc<motion::Beat>,
    pub(crate) counting: Counting,
    given: looks::Given,
    palette_settled: u64,
}

impl Focusable for Root {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DockWidths {
    panel: Split,
    browser: Split,
    flow: Split,
}

impl DockWidths {
    fn wide(self, kind: PanelKind) -> Option<Split> {
        match kind {
            PanelKind::Browser => Some(self.browser),
            PanelKind::Flow => Some(self.flow),
            _ => None,
        }
    }
}

struct Kept {
    place: Vec<String>,
    sidebar_width: Split,
    pair_width: Split,
    dock_widths: DockWidths,
    font_size: f32,
}

impl Kept {
    fn at_start(width: f32, cx: &mut App) -> Self {
        let saved = keeper::Keeper::get(cx).read(cx).kept_widths();
        let (sidebar_width, pair_width, dock_widths) = widths::restored(&saved, width);
        let shown = widths::of(sidebar_width, pair_width, dock_widths);
        keeper::Keeper::get(cx).update(cx, |keeper, _| keeper.start(shown));
        Self { place: start_place(), sidebar_width, pair_width, dock_widths, font_size: font_size_from_env() }
    }

    fn apply(self, root: &mut Root) {
        root.place = self.place;
        root.sidebar_width = self.sidebar_width;
        root.pair_width = self.pair_width;
        root.dock_widths = self.dock_widths;
        root.font_size = self.font_size;
    }
}

impl Root {
    pub(crate) fn new(window: &mut Window, width: f32, cx: &mut Context<Self>) -> Self {
        frame::show_backdrop(window);
        let missing = theme::missing_font();
        let mut root = Self::open(Kept::at_start(width, cx), window, cx);
        root.open_requested_panel(cx);
        if let Some(missing) = missing {
            root.say(missing, cx);
        }
        root
    }

    fn open(kept: Kept, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus);
        let (tx, mut rx) = unbounded::<Msg>();
        let errors = tx.clone();
        let moved = tx.clone();
        let queue = Queue::new(
            move |e| {
                let _ = errors.unbounded_send(Msg::Error(e));
            },
            move || {
                let _ = moved.unbounded_send(Msg::ViewMoved);
            },
        );
        let store = Store::init(queue.clone(), cx);
        let notifier = cx.new(|cx| Notifier::new(store.clone(), window, cx));
        cx.observe(&notifier, |_, _, cx| cx.notify()).detach();
        if let Some(updater) = self_update::entity(cx) {
            cx.observe(&updater, |_, _, cx| cx.notify()).detach();
        }
        cx.spawn(async move |this, cx| {
            while let Some(msg) = rx.next().await
                && this.update(cx, |this, cx| this.handle(msg, cx)).is_ok()
            {}
        })
        .detach();
        cx.observe_in(&store, window, |this, store, window, cx| this.store_changed(&store, window, cx)).detach();
        Self::follow_activation(window, cx);
        let mut root = Self::fresh((focus, cx.focus_handle()), store, tx, queue, notifier);
        kept.apply(&mut root);
        root.start(cx);
        root.follow_appearance(window, cx);
        Self::follow_window_bounds(window, cx);
        Self::follow_keeper(cx);
        Self::take_back_colours_on_quit(cx);
        root
    }

    fn follow_activation(window: &mut Window, cx: &mut Context<Self>) {
        cx.observe_window_activation(window, |this, window, cx| {
            this.activation_changed(window.is_window_active());
            cx.notify();
        })
        .detach();
    }

    fn store_changed(&mut self, store: &Entity<Store>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(target) = store.update(cx, |s, _| s.switch_to.take()) {
            self.switch(target, window, cx);
            return;
        }
        if let Some(focus) = store.update(cx, |s, _| s.refocus.take()) {
            window.focus(&focus);
        }
        let to_terminal = store.update(cx, |s, _| std::mem::take(&mut s.keys_to_terminal));
        let place = store.read(cx).machine_place.clone();
        if place != self.side.machine_place {
            self.machine_place_changed(place, window, cx);
        }
        self.follow_link(cx);
        let s = store.read(cx);
        let now = Overlays {
            open: s.dialog.is_some() || s.sheet.is_some(),
            sheet: s.sheet,
            full: s.shows_file_full_screen(),
        };
        let in_dock = self.dock_focus.is_focused(window) || self.dock_focus.contains_focused(window, cx);
        let gave_way = s.dialog.is_none() && sheet_gave_way(self.had.sheet, s.sheet);
        if to_terminal || (self.had.open && !now.open) || gave_way || (s.shown_dock().is_none() && in_dock) {
            window.focus(if now.full { &self.dock_focus } else { &self.focus });
        }
        let full_changed = now.full != self.had.full;
        self.had = now;
        if full_changed {
            self.own.tell_visible(self.visible && !self.had.full);
            self.tell_machine_visible();
        }
        self.track_leaving(cx);
        cx.notify();
    }

    fn switch(&mut self, target: Target, window: &mut Window, cx: &mut Context<Self>) {
        let kept = Kept {
            place: Vec::new(),
            sidebar_width: self.sidebar_width,
            pair_width: self.pair_width,
            dock_widths: self.dock_widths,
            font_size: self.font_size,
        };
        let left = cli::connect(target);
        plugins::follow_window_processes(cx);
        cli::kill_all();
        cli::leave(&left);
        self.take_back_colours();
        window.replace_root(cx, move |window, cx| Self::open(kept, window, cx));
    }

    fn fresh(
        (focus, dock_focus): (FocusHandle, FocusHandle),
        store: Entity<Store>,
        tx: UnboundedSender<Msg>,
        queue: Queue,
        notifier: Entity<Notifier>,
    ) -> Self {
        Self {
            focus,
            store,
            tx,
            own: Connection::new(cli::target(), queue),
            machine: None,
            session: None,
            metrics: None,
            font_size: font_size_from_env(),
            layout: Rc::default(),
            drag: None,
            pointer: Pointer::default(),
            moves: Rc::default(),
            marked: String::new(),
            conn: Conn::Starting,
            host: cli::host(),
            attempts: 0,
            came_up: false,
            updated_there: None,
            answered: None,
            place: Vec::new(),
            side: SideState::default(),
            visible: true,
            window_active: true,
            panels: HashMap::new(),
            sidebar_width: SIDEBAR,
            pair_width: PAIR,
            dock_widths: DOCKS,
            had: Overlays::default(),
            leaving_sheet: Leaving::default(),
            leaving_dialog: Leaving::default(),
            leaving_dock: Leaving::default(),
            notifier,
            title: String::new(),
            dock_focus,
            beat: Rc::default(),
            counting: Counting::default(),
            given: looks::Given::of(theme::terminal_json()),
            palette_settled: 0,
        }
    }

    fn track_leaving(&mut self, cx: &mut Context<Self>) {
        let (sheet, dialog, dock) = {
            let s = self.store.read(cx);
            (s.sheet, s.dialog.clone(), s.shown_dock())
        };
        if let Some(serial) = self.leaving_sheet.track(sheet) {
            Self::end_leave(cx, move |this| this.leaving_sheet.finish(serial));
        }
        if let Some(serial) = self.leaving_dialog.track(dialog) {
            Self::end_leave(cx, move |this| this.leaving_dialog.finish(serial));
        }
        if let Some(serial) = self.leaving_dock.track(dock) {
            Self::end_leave(cx, move |this| this.leaving_dock.finish(serial));
        }
    }

    fn end_leave(cx: &mut Context<Self>, finish: impl FnOnce(&mut Self) -> bool + 'static) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(motion::LEAVE + Duration::from_millis(20)).await;
            let _ = this.update(cx, |this, cx| finish(this).then(|| cx.notify()));
        })
        .detach();
    }

    pub(crate) fn activation_changed(&mut self, active: bool) {
        self.window_active = active;
        self.tell_machine_visible();
        let visible = active && self.machine.is_none();
        if visible == self.visible {
            return;
        }
        self.visible = visible;
        self.own.tell_visible(visible && !self.had.full);
    }

    fn open_requested_panel(&self, cx: &mut Context<Self>) {
        if let Ok(name) = std::env::var("CORNERCASE_GPUI_OPEN")
            && let Some(kind) = PanelKind::ALL.into_iter().find(|k| k.title().eq_ignore_ascii_case(&name))
        {
            self.store.update(cx, |s, cx| s.open(kind, cx));
        }
    }

    pub(crate) fn start(&mut self, cx: &App) {
        self.conn = Conn::Starting;
        let tx = self.tx.clone();
        let (cols, rows) = self.own.size.unwrap_or((160, 48));
        let size = (cols.max(90), rows.max(10));
        let chosen = self.store.read(cx).prefs.cornercase_bin.clone();
        std::thread::spawn(move || start_server(&tx, size, chosen));
    }

    fn open_view(&mut self, cx: &mut Context<Self>) {
        self.own.queue.reconnect();
        let (cols, rows) = self.own.size.unwrap_or((120, 36));
        let mut args = vec!["view".to_string(), "--cols".into(), cols.to_string(), "--rows".into(), rows.to_string()];
        args.extend(std::mem::take(&mut self.place));
        if let Err(e) = self.own.open_view(&args, &self.tx) {
            self.lost(e.to_string(), cx);
        }
        self.store.update(cx, |s, cx| {
            s.follow_app(cx);
            plugins::read_offers(s, cx);
        });
    }

    fn started(&mut self, start: cli::Start, cx: &mut Context<Self>) {
        match cli::step(start, self.updated_there.as_ref()) {
            cli::Step::Open => {
                self.updated_there = None;
                self.open_view(cx);
            }
            cli::Step::Behind(behind) => {
                self.answered = None;
                self.drop_connection(Conn::Behind(behind), cx);
            }
            cli::Step::Older(older) => self.drop_connection(Conn::Older(older), cx),
            cli::Step::Lost(why) => {
                self.lost(why, cx);
                self.retry(cx);
            }
        }
    }

    fn lost(&mut self, why: String, cx: &mut Context<Self>) {
        self.drop_connection(Conn::Lost(why), cx);
    }

    fn drop_connection(&mut self, conn: Conn, cx: &mut Context<Self>) {
        self.own.close();
        self.own.has_frame = false;
        self.conn = conn;
        self.store.update(cx, |s, cx| {
            s.view_id = None;
            cx.notify();
        });
    }

    fn handle(&mut self, msg: Msg, cx: &mut Context<Self>) {
        if matches!(msg, Msg::Started(_)) {
            self.given.forget();
            plugins::follow_window_processes(cx);
        }
        match msg {
            Msg::Started(start) => self.started(start, cx),
            Msg::Missing(missing) => self.drop_connection(Conn::Missing(missing), cx),
            Msg::Stream(serial, heard) => {
                if !self.stream_heard(serial, heard, cx) {
                    return;
                }
            }
            Msg::Error(e) => self.say(&e, cx),
            Msg::Copied(Board::Clipboard, text) => {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                self.store.update(cx, |s, cx| s.toast("Copied", ToastKind::Info, cx));
            }
            Msg::Copied(Board::Primary, text) => desktop::write_primary(&text, cx),
            Msg::Elsewhere(path) => self.store.update(cx, |s, cx| s.copy_elsewhere(path, cx)),
            Msg::Folded(fold, collapsed, error) => self.fold_answered(fold, collapsed, error.as_deref(), cx),
            Msg::ViewMoved => self.store.update(cx, Store::leave_machine_place),
        }
        cx.notify();
    }

    fn stream_heard(&mut self, serial: u64, heard: Heard, cx: &mut Context<Self>) -> bool {
        if self.own.owns(serial) {
            self.own_heard(serial, heard, cx);
            return true;
        }
        let there = self.machine.as_ref().is_some_and(|tab| tab.conn.owns(serial));
        if there {
            self.machine_heard(heard, cx);
        }
        there
    }

    fn own_heard(&mut self, serial: u64, heard: Heard, cx: &mut Context<Self>) {
        match heard {
            Heard::Line(session) => self.session_arrived(*session, cx),
            Heard::ViewEnded(err) => self.view_ended(&err, cx),
            Heard::Frame(frame) => {
                self.own.frame_arrived(*frame);
            }
            Heard::SurfaceEnded(err) => self.surface_ended(serial, &err, cx),
            Heard::QueueError(e) | Heard::FocusRefused(e) => self.say(&e, cx),
            Heard::Copied(..) | Heard::Bell(_) => {}
        }
    }

    fn session_arrived(&mut self, mut session: Session, cx: &mut Context<Self>) {
        self.side.folds.apply(&mut session);
        self.conn = Conn::Connected;
        self.came_up = true;
        self.answered = None;
        self.attempts = 0;
        self.follow_surface(&session, cx);
        let view_id = self.own.view_id;
        let moved = self.session.as_ref().is_some_and(|old| old.shown() != session.shown());
        let shared = session.clone();
        self.store.update(cx, |s, cx| {
            if moved {
                s.leave_file_full_screen(cx);
            }
            s.jobs.keep_leaving(|workspace| shared.workspace(workspace).is_some());
            s.follow_accounts(&shared, cx);
            s.session = Some(shared);
            s.follow_machines(cx);
            s.follow_awaited(cx);
            s.view_id = view_id;
            cx.notify();
        });
        self.session = Some(session);
        self.drop_drags_without_tab();
        self.publish_tab(cx);
        self.follow_look(cx);
    }

    fn view_ended(&mut self, err: &str, cx: &mut Context<Self>) {
        let why = err.lines().last().filter(|why| !why.is_empty()).unwrap_or("the server stopped");
        self.lost(why.to_string(), cx);
        self.retry(cx);
    }

    fn retry(&mut self, cx: &mut Context<Self>) {
        let wait = if self.host.is_some() { cli::backoff(self.attempts) } else { RETRY };
        self.attempts += 1;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let _ = this.update(cx, Self::restart_if_lost);
        })
        .detach();
    }

    fn restart_if_lost(&mut self, cx: &mut Context<Self>) {
        if matches!(self.conn, Conn::Lost(_)) {
            self.start(cx);
            cx.notify();
        }
    }

    pub(crate) fn say(&mut self, text: &str, cx: &mut Context<Self>) {
        let text = first_line(text).to_string();
        self.store.update(cx, |s, cx| s.toast(text, ToastKind::Error, cx));
    }

    pub(crate) fn act(&mut self, args: Vec<String>) {
        self.own.act(args);
    }

    pub(crate) fn focus_item(&mut self, kind: &str, id: u64) {
        self.act(vec!["focus".into(), format!("--{kind}"), id.to_string()]);
    }

    pub(crate) fn focus_on_shown(&self, kind: &str, id: u64) {
        self.shown().act(vec!["focus".into(), format!("--{kind}"), id.to_string()]);
    }

    pub(crate) fn set_size(&mut self, cols: u16, rows: u16) {
        let conn = self.shown_mut();
        if conn.size != Some((cols, rows)) {
            conn.size = Some((cols, rows));
            conn.send_size((cols, rows));
        }
    }

    pub(crate) fn active_pane(&self) -> Option<u64> {
        self.takes_input().then(|| self.shown().grid.active_pane().map(|p| p.pane)).flatten()
    }

    pub(crate) fn shown_workspace(&self) -> Option<(&Project, &Workspace)> {
        let session = self.session.as_ref()?;
        session.workspace(session.shown().workspace?)
    }

    pub(crate) fn panel(&mut self, kind: PanelKind, window: &mut Window, cx: &mut Context<Self>) -> AnyView {
        if let Some(view) = self.panels.get(&kind) {
            return view.clone();
        }
        let view = kind.build(window, cx);
        self.panels.insert(kind, view.clone());
        view
    }

    fn dock_size(&self, kind: PanelKind, window: &Window) -> f32 {
        let width = f32::from(window.viewport_size().width);
        let (split, size) = match self.dock_widths.wide(kind) {
            Some(wide) => (wide, wide.chosen().unwrap_or_else(|| wide_default(width))),
            None => (self.dock_widths.panel, self.dock_widths.panel.size()),
        };
        split.fitted(size, widths::dock_room(width, self.sidebar_extent()))
    }

    fn render_dock(
        &mut self,
        kind: PanelKind,
        full: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let view = self.panel(kind, window, cx);
        let width = (!full).then(|| self.dock_size(kind, window));
        let card = dock_card(kind, view, &self.store, width).track_focus(&self.dock_focus);
        motion::slide(card, SharedString::from(format!("dock-in-{}", kind.title())), Way::In, (DOCK_SLIDE, 0.))
    }

    fn render_leaving_dock(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let (serial, kind) = self.leaving_dock.gone().map(|(serial, kind)| (serial, *kind))?;
        let view = self.panel(kind, window, cx);
        let width = Some(self.dock_size(kind, window));
        let card = dock_card(kind, view, &self.store, width).track_focus(&self.dock_focus);
        Some(
            div()
                .absolute()
                .top_0()
                .right_0()
                .h_full()
                .child(motion::slide(card, ("dock-out", serial), Way::Out, (DOCK_SLIDE, 0.)))
                .child(occluder()),
        )
    }

    fn render_sheet(
        &mut self,
        kind: PanelKind,
        window_frame: frame::Frame,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let card = sheet_card(kind, self.panel(kind, window, cx), &self.store);
        let id = format!("sheet-in-{}", kind.title());
        overlay(
            SharedString::from(format!("{id}-veil")),
            Way::In,
            window_frame,
            motion::slide(card, SharedString::from(id), Way::In, (0., SHEET_RISE)),
        )
    }

    fn render_leaving(
        &mut self,
        window_frame: frame::Frame,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let sheet = self.leaving_sheet.gone().map(|(serial, kind)| (serial, *kind));
        let sheet = sheet.map(|(serial, kind)| {
            let card = sheet_card(kind, self.panel(kind, window, cx), &self.store);
            overlay(
                ("sheet-out-veil", serial),
                Way::Out,
                window_frame,
                motion::slide(card, ("sheet-out", serial), Way::Out, (0., SHEET_RISE)),
            )
            .into_any_element()
        });
        let dialog = self.leaving_dialog.gone().map(|(serial, view)| {
            let card = motion::slide(div().child(view.clone()), ("dialog-out", serial), Way::Out, (0., DIALOG_RISE));
            overlay(("dialog-out-veil", serial), Way::Out, window_frame, card).into_any_element()
        });
        sheet.into_iter().chain(dialog).collect()
    }

    fn update_title(&mut self, window: &mut Window) {
        let title = self.session.as_ref().map_or_else(|| "cornercase".to_string(), notify::window_title);
        let title = match &self.host {
            Some(host) => format!("{title} on {host}"),
            None => title,
        };
        if title != self.title {
            window.set_window_title(&title);
            self.title = title;
        }
    }

    fn render_main(
        &mut self,
        window_frame: frame::Frame,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let dock = self.store.read(cx).shown_dock().map_or(0., |kind| self.dock_size(kind, window));
        let at = (Edge::Right, self.sidebar_extent());
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .relative()
            .flex()
            .flex_row()
            .on_drag_move(split::follow::<_, SidebarGrip>(Edge::Right, TERMINAL_KEEP + dock, sidebar_width, cx))
            .child(self.render_sidebar(window_frame, cx))
            .child(self.render_workspace_area(window_frame, window, cx))
            .child(split::handle("sidebar-split", SidebarGrip, at, sidebar_width, cx))
    }

    fn render_workspace_area(
        &mut self,
        window_frame: frame::Frame,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .pl(px(GAP))
            .overflow_hidden()
            .child(self.render_tabs(window_frame, cx))
            .child(self.render_body(window, cx))
            .child(self.render_status(cx))
    }

    fn render_body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let shown = self.store.read(cx).shown_dock();
        let full = self.store.read(cx).shows_file_full_screen();
        let dock = shown.map(|kind| self.render_dock(kind, full, window, cx));
        let leaving_dock = self.render_leaving_dock(window, cx);
        let width = dock_access(shown);
        let at = shown.map(|kind| (Edge::Left, self.dock_size(kind, window)));
        let kept_room = at.filter(|_| full).map(|(_, size)| div().w(px(size)).flex_none());
        let handle = at.filter(|_| !full).map(|at| split::handle("dock-split", DockGrip, at, width, cx));
        div()
            .flex_1()
            .flex()
            .flex_row()
            .relative()
            .overflow_hidden()
            .on_drag_move(split::follow::<_, DockGrip>(Edge::Left, TERMINAL_KEEP, width, cx))
            .child(self.render_terminal_area(window, cx))
            .children(kept_room)
            .children(dock.map(|dock| over_the_terminal(dock, full)))
            .children(leaving_dock)
            .children(handle)
    }

    fn render_terminal_area(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let flashing = self.notifier.read(cx).flashing();
        let covered = self.store.read(cx).shows_file_full_screen();
        div().flex_1().h_full().flex().pr(px(GAP)).when(covered, Styled::invisible).child(
            div()
                .flex_1()
                .h_full()
                .flex()
                .relative()
                .overflow_hidden()
                .child(self.render_terminal(window, cx))
                .when_some(flashing, |d, flash| d.child(motion::fade(bell_flash(), ("bell", flash), Way::Out))),
        )
    }
}

fn sidebar_width(root: &mut Root) -> &mut Split {
    if root.side.look.current.arrangement == Arrangement::SideBySide {
        return &mut root.pair_width;
    }
    &mut root.sidebar_width
}

fn dock_width(root: &mut Root) -> &mut Split {
    &mut root.dock_widths.panel
}

fn browser_width(root: &mut Root) -> &mut Split {
    &mut root.dock_widths.browser
}

fn flow_width(root: &mut Root) -> &mut Split {
    &mut root.dock_widths.flow
}

fn dock_access(kind: Option<PanelKind>) -> split::Access<Root> {
    match kind {
        Some(PanelKind::Browser) => browser_width,
        Some(PanelKind::Flow) => flow_width,
        _ => dock_width,
    }
}

#[derive(Clone, Copy, Default)]
struct Overlays {
    open: bool,
    sheet: Option<PanelKind>,
    full: bool,
}

fn sheet_gave_way(before: Option<PanelKind>, now: Option<PanelKind>) -> bool {
    before.is_some() && now.is_some() && before != now
}

fn wide_default(window_width: f32) -> f32 {
    (window_width / 2.).clamp(DOCK.size(), WIDE_DOCK.size())
}

fn bell_flash() -> Div {
    div()
        .absolute()
        .inset_0()
        .rounded(px(radius::card()))
        .border_2()
        .border_color(colours::working())
        .bg(ui::glass::tint(colours::working(), 0.07))
}

fn overlay(id: impl Into<ElementId>, way: Way, frame: frame::Frame, card: impl IntoElement) -> impl IntoElement {
    let backdrop = div().absolute().inset_0().occlude().flex().items_center().justify_center().child(card);
    let backdrop = frame::round(backdrop, frame);
    let backdrop = match way {
        Way::Out => backdrop.child(occluder()),
        _ => backdrop.on_mouse_down(MouseButton::Left, |_, _, cx| {
            Store::get(cx).update(cx, Store::close_overlay);
        }),
    };
    motion::veil(backdrop, id, way, colours::veil())
}

fn occluder() -> Div {
    div().absolute().inset_0().occlude()
}

fn sheet_card(kind: PanelKind, view: AnyView, store: &Entity<Store>) -> Stateful<Div> {
    ui::glass::card(radius::sheet())
        .id("sheet")
        .w(gpui::relative(0.82))
        .h(gpui::relative(0.86))
        .max_w(px(1100.))
        .flex()
        .flex_col()
        .overflow_hidden()
        .occlude()
        .child(sheet_header(kind, store))
        .child(div().flex_1().overflow_hidden().child(view))
}

fn sheet_header(kind: PanelKind, store: &Entity<Store>) -> Div {
    let store = store.clone();
    div()
        .h(px(50.))
        .flex_none()
        .flex()
        .items_center()
        .justify_between()
        .pl(px(20.))
        .pr(px(14.))
        .border_b_1()
        .border_color(colours::border())
        .child(panel_title(kind))
        .child(
            ui::icon_button("sheet-close", "✕")
                .on_click(move |_: &ClickEvent, _, cx| store.update(cx, Store::close_sheet)),
        )
}

fn panel_title(kind: PanelKind) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(ui::icon(kind.icon(), 15.).text_color(colours::accent()))
        .child(ui::title(kind.title()))
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or_default()
}

fn font_size_from_env() -> f32 {
    std::env::var("CORNERCASE_GPUI_FONT_SIZE").ok().and_then(|s| s.parse().ok()).unwrap_or(13.)
}

fn start_server(tx: &UnboundedSender<Msg>, (cols, rows): (u16, u16), chosen: Option<String>) {
    let msg = start_message(cli::ensure_binary(chosen), || {
        cli::start_server(&start_args((cols, rows)), &theme::terminal_json())
    });
    let _ = tx.unbounded_send(msg);
}

fn start_args((cols, rows): (u16, u16)) -> Vec<String> {
    let words = ["start-server", "--cols", &cols.to_string(), "--rows", &rows.to_string(), "--theme", "-", "--json"];
    words.iter().map(ToString::to_string).collect()
}

fn start_message(found: Result<(), cli::Missing>, start: impl FnOnce() -> cli::Start) -> Msg {
    match found {
        Ok(()) => Msg::Started(start()),
        Err(missing) => Msg::Missing(missing),
    }
}

fn start_place() -> Vec<String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    args.windows(2)
        .find(|w| matches!(w[0].as_str(), "--tab" | "--workspace" | "--project") && w[1].parse::<u64>().is_ok())
        .map(<[String]>::to_vec)
        .unwrap_or_default()
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.update_title(window);
        self.keep_beating(window, cx);
        self.keep_the_keys(window, cx);
        let window_frame = frame::Frame::read(window, Os::CURRENT);
        let (sheet, dialog) = {
            let s = self.store.read(cx);
            (s.sheet, s.dialog.clone())
        };
        let sheet = sheet.map(|kind| self.sheet_or_peek(kind, window_frame, window, cx));
        let content = div()
            .size_full()
            .relative()
            .flex()
            .flex_row()
            .bg(colours::glass())
            .text_color(colours::text())
            .text_size(px(13.))
            .font_family(theme::UI_FAMILY)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .child(self.render_rail(cx))
            .child(self.render_main(window_frame, window, cx))
            .children(sheet)
            .children(dialog.map(|view| {
                let id = view.entity_id();
                overlay(
                    ("dialog-in-veil", id),
                    Way::In,
                    window_frame,
                    motion::slide(div().occlude().child(view), ("dialog-in", id), Way::In, (0., DIALOG_RISE)),
                )
            }))
            .children(self.render_leaving(window_frame, window, cx))
            .child(crate::tour::Guide::get(cx))
            .child(self.render_toasts(cx));
        self.keep_counting(cx);
        self.keep_widths(cx);
        frame::framed(content, window_frame, window)
    }
}

#[cfg(test)]
mod tests;
