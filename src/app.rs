mod status;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use futures::StreamExt as _;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui::{
    AnyElement, AnyView, App, AppContext as _, ClickEvent, ClipboardItem, Context, Div, ElementId, Entity, FocusHandle,
    Focusable, InteractiveElement, IntoElement, MouseButton, ParentElement, Render, Rgba, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled, Window, div, prelude::FluentBuilder as _, px,
};

use crate::cli::{self, Queue, Stream, Target};
use crate::model::{Applied, Frame, Grid, Project, Session, Workspace};
use crate::notify::{self, Notifier};
use crate::panels::PanelKind;
use crate::render::Metrics;
use crate::render::layout::{Along, Layout};
use crate::render::moving::Moves;
use crate::sidebar::{Arrangement, Fold, SideState};
use crate::store::{Store, Toast, ToastKind};
use crate::terminal::Pointer;
use crate::theme::{self, colours, radius};
use crate::ui;
use crate::ui::motion::{self, Leaving, Way};
use crate::ui::split::{self, Edge, Split};

pub(crate) const TITLEBAR_HEIGHT: f32 = 44.;
pub(crate) const GAP: f32 = 8.;
const SIDEBAR: Split = Split::new(256., 200., 560.);
const PAIR: Split = Split::new(456., 360., 760.);
const DOCK: Split = Split::new(428., 308., 1208.);
const BROWSER_DOCK: Split = Split::new(720., 420., 4096.);
const DOCKS: DockWidths = DockWidths { panel: DOCK, browser: BROWSER_DOCK };
const DOCK_HEADER: f32 = 42.;
const TERMINAL_KEEP: f32 = 240.;
const DOCK_SLIDE: f32 = 24.;
const SHEET_RISE: f32 = 14.;
const DIALOG_RISE: f32 = 10.;
const TOAST_SLIDE: f32 = 28.;
const RETRY: Duration = Duration::from_millis(1500);

pub(crate) enum Msg {
    Started(Result<String, String>),
    Session(u64, Box<Session>),
    ViewEnded(u64, String),
    Frame(u64, Box<Frame>),
    SurfaceEnded(u64, String),
    Error(String),
    Copied(String),
    Elsewhere(String),
    Folded(Fold, bool, Option<String>),
}

#[derive(Clone, PartialEq)]
pub(crate) enum Conn {
    Starting,
    Connected,
    Lost(String),
}

pub(crate) enum Drag {
    Program { pane: u64, motion: bool },
    Select { pane: u64, from: (u16, u16), to: (u16, u16) },
    Divider { tab: u64, path: Vec<bool>, down: bool, along: Along },
    Link { pane: u64, cell: (u16, u16) },
}

struct SidebarGrip;

struct DockGrip;

pub(crate) struct Root {
    pub(crate) focus: FocusHandle,
    pub(crate) store: Entity<Store>,
    pub(crate) tx: UnboundedSender<Msg>,
    pub(crate) queue: Queue,
    pub(crate) generation: u64,
    pub(crate) view_stream: Option<Stream>,
    pub(crate) surface_stream: Option<Stream>,
    pub(crate) session: Option<Session>,
    pub(crate) view_id: Option<u64>,
    pub(crate) grid: Rc<Grid>,
    pub(crate) has_frame: bool,
    pub(crate) surface_error: Option<String>,
    pub(crate) metrics: Option<Metrics>,
    pub(crate) font_size: f32,
    pub(crate) layout: Rc<RefCell<Layout>>,
    pub(crate) size: Option<(u16, u16)>,
    pub(crate) drag: Option<Drag>,
    pub(crate) pointer: Pointer,
    pub(crate) moves: Rc<RefCell<Moves>>,
    pub(crate) marked: String,
    pub(crate) conn: Conn,
    pub(crate) host: Option<String>,
    attempts: u32,
    place: Vec<String>,
    pub(crate) side: SideState,
    pub(crate) visible: bool,
    pub(crate) panels: HashMap<PanelKind, AnyView>,
    pub(crate) sidebar_width: Split,
    pub(crate) pair_width: Split,
    dock_widths: DockWidths,
    had_overlay: bool,
    leaving_sheet: Leaving<PanelKind>,
    leaving_dialog: Leaving<AnyView>,
    leaving_dock: Leaving<PanelKind>,
    notifier: Entity<Notifier>,
    title: String,
    pub(crate) dock_focus: FocusHandle,
    beat: Rc<motion::Beat>,
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
}

struct Kept {
    place: Vec<String>,
    sidebar_width: Split,
    pair_width: Split,
    dock_widths: DockWidths,
    font_size: f32,
}

impl Kept {
    fn at_start() -> Self {
        Self {
            place: start_place(),
            sidebar_width: SIDEBAR,
            pair_width: PAIR,
            dock_widths: DOCKS,
            font_size: font_size_from_env(),
        }
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
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let root = Self::open(Kept::at_start(), window, cx);
        root.open_requested_panel(cx);
        root
    }

    fn open(kept: Kept, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus);
        let (tx, mut rx) = unbounded::<Msg>();
        let errors = tx.clone();
        let queue = Queue::new(move |e| {
            let _ = errors.unbounded_send(Msg::Error(e));
        });
        let store = Store::init(queue.clone(), cx);
        let notifier = cx.new(|cx| Notifier::new(store.clone(), window, cx));
        cx.observe(&notifier, |_, _, cx| cx.notify()).detach();
        cx.spawn(async move |this, cx| {
            while let Some(msg) = rx.next().await
                && this.update(cx, |this, cx| this.handle(msg, cx)).is_ok()
            {}
        })
        .detach();
        cx.observe_in(&store, window, |this, store, window, cx| this.store_changed(&store, window, cx)).detach();
        cx.observe_window_activation(window, |this, window, cx| {
            this.activation_changed(window.is_window_active());
            cx.notify();
        })
        .detach();
        let mut root = Self::fresh((focus, cx.focus_handle()), store, tx, queue, notifier);
        kept.apply(&mut root);
        root.start();
        root
    }

    fn store_changed(&mut self, store: &Entity<Store>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(target) = store.update(cx, |s, _| s.switch_to.take()) {
            self.switch(target, window, cx);
            return;
        }
        let s = store.read(cx);
        let overlay = s.dialog.is_some() || s.sheet.is_some();
        let in_dock = self.dock_focus.is_focused(window) || self.dock_focus.contains_focused(window, cx);
        if (self.had_overlay && !overlay) || (s.dock.is_none() && in_dock) {
            window.focus(&self.focus);
        }
        self.had_overlay = overlay;
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
        cli::kill_all();
        cli::leave(&left);
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
            queue,
            generation: 0,
            view_stream: None,
            surface_stream: None,
            session: None,
            view_id: None,
            grid: Rc::new(Grid::default()),
            has_frame: false,
            surface_error: None,
            metrics: None,
            font_size: font_size_from_env(),
            layout: Rc::default(),
            size: None,
            drag: None,
            pointer: Pointer::default(),
            moves: Rc::default(),
            marked: String::new(),
            conn: Conn::Starting,
            host: cli::host(),
            attempts: 0,
            place: Vec::new(),
            side: SideState::default(),
            visible: true,
            panels: HashMap::new(),
            sidebar_width: SIDEBAR,
            pair_width: PAIR,
            dock_widths: DOCKS,
            had_overlay: false,
            leaving_sheet: Leaving::default(),
            leaving_dialog: Leaving::default(),
            leaving_dock: Leaving::default(),
            notifier,
            title: String::new(),
            dock_focus,
            beat: Rc::default(),
        }
    }

    fn track_leaving(&mut self, cx: &mut Context<Self>) {
        let (sheet, dialog, dock) = {
            let s = self.store.read(cx);
            (s.sheet, s.dialog.clone(), s.dock)
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

    fn activation_changed(&mut self, active: bool) {
        if active == self.visible {
            return;
        }
        self.visible = active;
        if let Some(id) = self.view_id {
            let flag = if active { "--visible" } else { "--hidden" };
            self.queue.cmd(vec!["view-set".into(), "--view".into(), id.to_string(), flag.into()]);
        }
    }

    fn open_requested_panel(&self, cx: &mut Context<Self>) {
        if let Ok(name) = std::env::var("CORNERCASE_GPUI_OPEN")
            && let Some(kind) = PanelKind::ALL.into_iter().find(|k| k.title().eq_ignore_ascii_case(&name))
        {
            self.store.update(cx, |s, cx| s.open(kind, cx));
        }
    }

    fn start(&mut self) {
        self.conn = Conn::Starting;
        let tx = self.tx.clone();
        let (cols, rows) = self.size.unwrap_or((160, 48));
        let (cols, rows) = (cols.max(90), rows.max(10));
        std::thread::spawn(move || {
            let args = ["start-server", "--cols", &cols.to_string(), "--rows", &rows.to_string(), "--theme", "-"];
            let result = cli::run_with_stdin(&args, Some(&theme::theme_json())).map_err(|e| e.to_string());
            let _ = tx.unbounded_send(Msg::Started(result));
        });
    }

    fn open_view(&mut self, cx: &mut Context<Self>) {
        self.generation += 1;
        let generation = self.generation;
        self.surface_stream = None;
        self.view_stream = None;
        self.view_id = None;
        self.queue.reconnect();
        let (cols, rows) = self.size.unwrap_or((120, 36));
        let mut args = vec!["view".to_string(), "--cols".into(), cols.to_string(), "--rows".into(), rows.to_string()];
        args.extend(std::mem::take(&mut self.place));
        let (tx, end) = (self.tx.clone(), self.tx.clone());
        let stream = cli::stream(
            &args,
            move |line| match serde_json::from_str::<Session>(&line) {
                Ok(s) => {
                    let _ = tx.unbounded_send(Msg::Session(generation, Box::new(s)));
                }
                Err(e) => {
                    let _ = tx.unbounded_send(Msg::Error(format!("view line: {e}")));
                }
            },
            move |err| {
                let _ = end.unbounded_send(Msg::ViewEnded(generation, err));
            },
        );
        match stream {
            Ok(s) => self.view_stream = Some(s),
            Err(e) => self.lost(e.to_string(), cx),
        }
        self.store.update(cx, Store::follow_app);
    }

    fn open_surface(&mut self, view: u64, cx: &mut Context<Self>) {
        let generation = self.generation;
        let args = ["surface".to_string(), "--view".into(), view.to_string(), "--patches".into()];
        let (tx, end) = (self.tx.clone(), self.tx.clone());
        let stream = cli::stream(
            &args,
            move |line| match serde_json::from_str::<Frame>(&line) {
                Ok(f) => {
                    let _ = tx.unbounded_send(Msg::Frame(generation, Box::new(f)));
                }
                Err(e) => {
                    let _ = tx.unbounded_send(Msg::Error(format!("surface line: {e}")));
                }
            },
            move |err| {
                let _ = end.unbounded_send(Msg::SurfaceEnded(generation, err));
            },
        );
        match stream {
            Ok(s) => self.surface_stream = Some(s),
            Err(e) => self.say(&e.to_string(), cx),
        }
    }

    fn lost(&mut self, why: String, cx: &mut Context<Self>) {
        self.surface_stream = None;
        self.view_stream = None;
        self.view_id = None;
        self.has_frame = false;
        self.conn = Conn::Lost(why);
        self.store.update(cx, |s, cx| {
            s.view_id = None;
            cx.notify();
        });
    }

    fn handle(&mut self, msg: Msg, cx: &mut Context<Self>) {
        let current = self.generation;
        match msg {
            Msg::Started(Ok(_)) => self.open_view(cx),
            Msg::Started(Err(e)) => {
                self.lost(e, cx);
                self.retry(cx);
            }
            Msg::Session(generation, session) if generation == current => self.session_arrived(*session, cx),
            Msg::ViewEnded(generation, err) if generation == current => self.view_ended(&err, cx),
            Msg::Frame(generation, frame) if generation == current => self.frame_arrived(*frame),
            Msg::SurfaceEnded(generation, err) if generation == current => self.surface_ended(generation, &err, cx),
            Msg::Error(e) => self.say(&e, cx),
            Msg::Copied(text) => {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                self.store.update(cx, |s, cx| s.toast("Copied", ToastKind::Info, cx));
            }
            Msg::Elsewhere(path) => self.store.update(cx, |s, cx| s.copy_elsewhere(path, cx)),
            Msg::Folded(fold, collapsed, error) => self.fold_answered(fold, collapsed, error.as_deref(), cx),
            _ => return,
        }
        cx.notify();
    }

    fn session_arrived(&mut self, mut session: Session, cx: &mut Context<Self>) {
        self.side.folds.apply(&mut session);
        self.conn = Conn::Connected;
        self.attempts = 0;
        if let Some(id) = session.view.as_ref().map(|v| v.id)
            && Some(id) != self.view_id
        {
            self.view_id = Some(id);
            self.open_surface(id, cx);
            self.match_view_size(&session);
        }
        let view_id = self.view_id;
        let shared = session.clone();
        self.store.update(cx, |s, cx| {
            s.jobs.keep_leaving(|workspace| shared.workspace(workspace).is_some());
            s.session = Some(shared);
            s.view_id = view_id;
            cx.notify();
        });
        self.session = Some(session);
        self.follow_look(cx);
    }

    fn match_view_size(&mut self, session: &Session) {
        if let Some((cols, rows)) = self.size
            && session.view.as_ref().is_some_and(|v| v.cols != Some(cols) || v.rows != Some(rows))
        {
            self.send_size(cols, rows);
        }
    }

    fn view_ended(&mut self, err: &str, cx: &mut Context<Self>) {
        let why = err.lines().last().filter(|why| !why.is_empty()).unwrap_or("the server stopped");
        self.lost(why.to_string(), cx);
        self.retry(cx);
    }

    fn frame_arrived(&mut self, frame: Frame) {
        if let Applied::NeedRedraw = Rc::make_mut(&mut self.grid).apply(frame) {
            let surface = self.grid.surface;
            self.queue.coalesced("redraw", vec!["redraw".into(), "--surface".into(), surface.to_string()]);
        } else {
            self.has_frame = true;
            self.surface_error = None;
        }
    }

    fn surface_ended(&mut self, generation: u64, err: &str, cx: &mut Context<Self>) {
        self.has_frame = false;
        let err = first_line(err.trim());
        let repeated = !err.is_empty() && self.surface_error.as_deref() == Some(err);
        if !err.is_empty() && !repeated {
            self.say(err, cx);
        }
        self.surface_error = (!err.is_empty()).then(|| err.to_string());
        let wait = if repeated { 5000 } else { 400 };
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(wait)).await;
            let _ = this.update(cx, |this, cx| this.reopen_surface(generation, cx));
        })
        .detach();
    }

    fn reopen_surface(&mut self, generation: u64, cx: &mut Context<Self>) {
        if self.generation == generation
            && self.surface_stream.is_some()
            && let Some(id) = self.view_id
        {
            self.open_surface(id, cx);
            cx.notify();
        }
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
            self.start();
            cx.notify();
        }
    }

    pub(crate) fn say(&mut self, text: &str, cx: &mut Context<Self>) {
        let text = first_line(text).to_string();
        self.store.update(cx, |s, cx| s.toast(text, ToastKind::Error, cx));
    }

    pub(crate) fn act(&mut self, args: Vec<String>) {
        if let Some(id) = self.view_id {
            let mut args = args;
            args.push("--view".into());
            args.push(id.to_string());
            self.queue.cmd(args);
        }
    }

    pub(crate) fn focus_item(&mut self, kind: &str, id: u64) {
        self.act(vec!["focus".into(), format!("--{kind}"), id.to_string()]);
    }

    fn send_size(&mut self, cols: u16, rows: u16) {
        if let Some(id) = self.view_id {
            let args = vec![
                "view-set".into(),
                "--view".into(),
                id.to_string(),
                "--cols".into(),
                cols.to_string(),
                "--rows".into(),
                rows.to_string(),
            ];
            self.queue.coalesced("size", args);
        }
    }

    pub(crate) fn set_size(&mut self, cols: u16, rows: u16) {
        if self.size != Some((cols, rows)) {
            self.size = Some((cols, rows));
            self.send_size(cols, rows);
        }
    }

    pub(crate) fn active_pane(&self) -> Option<u64> {
        self.has_frame.then(|| self.grid.active_pane().map(|p| p.pane)).flatten()
    }

    pub(crate) fn input(&self, event: serde_json::Value) {
        if let Some(pane) = self.active_pane() {
            self.queue.input(pane, event);
        }
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
        let widths = self.dock_widths;
        if kind != PanelKind::Browser {
            return widths.panel.size();
        }
        widths.browser.chosen().unwrap_or_else(|| browser_default(f32::from(window.viewport_size().width)))
    }

    fn render_dock(
        &mut self,
        kind: PanelKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let view = self.panel(kind, window, cx);
        let card = dock_card(kind, view, &self.store, self.dock_size(kind, window)).track_focus(&self.dock_focus);
        motion::slide(card, SharedString::from(format!("dock-in-{}", kind.title())), Way::In, (DOCK_SLIDE, 0.))
    }

    fn render_leaving_dock(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let (serial, kind) = self.leaving_dock.gone().map(|(serial, kind)| (serial, *kind))?;
        let view = self.panel(kind, window, cx);
        let card = dock_card(kind, view, &self.store, self.dock_size(kind, window)).track_focus(&self.dock_focus);
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let card = sheet_card(kind, self.panel(kind, window, cx), &self.store);
        let id = format!("sheet-in-{}", kind.title());
        overlay(
            SharedString::from(format!("{id}-veil")),
            Way::In,
            motion::slide(card, SharedString::from(id), Way::In, (0., SHEET_RISE)),
        )
    }

    fn render_leaving(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let sheet = self.leaving_sheet.gone().map(|(serial, kind)| (serial, *kind));
        let sheet = sheet.map(|(serial, kind)| {
            let card = sheet_card(kind, self.panel(kind, window, cx), &self.store);
            overlay(
                ("sheet-out-veil", serial),
                Way::Out,
                motion::slide(card, ("sheet-out", serial), Way::Out, (0., SHEET_RISE)),
            )
            .into_any_element()
        });
        let dialog = self.leaving_dialog.gone().map(|(serial, view)| {
            let card = motion::slide(div().child(view.clone()), ("dialog-out", serial), Way::Out, (0., DIALOG_RISE));
            overlay(("dialog-out-veil", serial), Way::Out, card).into_any_element()
        });
        sheet.into_iter().chain(dialog).collect()
    }

    fn render_toasts(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let toasts: Vec<_> = self.store.read(cx).toasts.iter().filter(|t| !t.expired()).cloned().collect();
        let store = self.store.clone();
        div().absolute().bottom(px(40.)).right(px(16.)).flex().flex_col().gap(px(8.)).children(toasts.into_iter().map(
            move |t| {
                let id = t.id;
                motion::slide(toast_view(&store, t), ("toast-in", id), Way::In, (TOAST_SLIDE, 0.))
            },
        ))
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

    fn render_main(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let dock = self.store.read(cx).dock.map_or(0., |kind| self.dock_size(kind, window));
        let at = (Edge::Right, self.sidebar_extent());
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .relative()
            .flex()
            .flex_row()
            .on_drag_move(split::follow::<_, SidebarGrip>(Edge::Right, TERMINAL_KEEP + dock, sidebar_width, cx))
            .child(self.render_sidebar(cx))
            .child(self.render_workspace_area(window, cx))
            .child(split::handle("sidebar-split", SidebarGrip, at, sidebar_width, cx))
    }

    fn render_workspace_area(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .pl(px(GAP))
            .overflow_hidden()
            .child(self.render_tabs(cx))
            .child(self.render_body(window, cx))
            .child(self.render_status(cx))
    }

    fn render_body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let shown = self.store.read(cx).dock;
        let dock = shown.map(|kind| self.render_dock(kind, window, cx));
        let leaving_dock = self.render_leaving_dock(window, cx);
        let width: split::Access<Self> = if shown == Some(PanelKind::Browser) { browser_width } else { dock_width };
        let at = shown.map(|kind| (Edge::Left, self.dock_size(kind, window)));
        let handle = at.map(|at| split::handle("dock-split", DockGrip, at, width, cx));
        div()
            .flex_1()
            .flex()
            .flex_row()
            .relative()
            .overflow_hidden()
            .on_drag_move(split::follow::<_, DockGrip>(Edge::Left, TERMINAL_KEEP, width, cx))
            .child(self.render_terminal_area(window, cx))
            .children(dock)
            .children(leaving_dock)
            .children(handle)
    }

    fn render_terminal_area(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let flashing = self.notifier.read(cx).flashing();
        div().flex_1().h_full().flex().pr(px(GAP)).child(
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

fn browser_default(window_width: f32) -> f32 {
    (window_width / 2.).clamp(DOCK.size(), BROWSER_DOCK.size())
}

fn toast_colour(kind: ToastKind) -> Rgba {
    match kind {
        ToastKind::Info => colours::accent(),
        ToastKind::Error => colours::failed(),
        ToastKind::NeedsYou => colours::waiting(),
        ToastKind::Done => colours::done(),
    }
}

fn toast_view(store: &Entity<Store>, toast: Toast) -> Stateful<Div> {
    let id = toast.id;
    let (go, dismiss) = (store.clone(), store.clone());
    let colour = toast_colour(toast.kind);
    ui::glass::card(radius::CARD)
        .id(SharedString::from(format!("toast-{id}")))
        .w(px(348.))
        .flex()
        .items_center()
        .gap(px(10.))
        .px(px(14.))
        .py(px(10.))
        .occlude()
        .child(
            div().w(px(3.)).h(px(18.)).rounded_full().bg(colour).shadow(ui::glass::glow(colour, 0.6, 8.)).flex_none(),
        )
        .child(div().flex_1().min_w_0().text_size(px(12.)).child(toast.text))
        .when(toast.count > 1, |d| d.child(toast_count(toast.count)))
        .when_some(toast.action, move |d, (label, args)| {
            d.child(ui::primary_button(SharedString::from(format!("toast-go-{id}")), label).on_click(
                move |_: &ClickEvent, _, cx| {
                    go.update(cx, |s, cx| {
                        s.act(args.clone());
                        s.dismiss_toast(id, cx);
                    });
                },
            ))
        })
        .child(
            ui::icon_button(SharedString::from(format!("toast-x-{id}")), "✕")
                .on_click(move |_: &ClickEvent, _, cx| dismiss.update(cx, |s, cx| s.dismiss_toast(id, cx))),
        )
}

fn toast_count(count: u32) -> impl IntoElement {
    div()
        .flex_none()
        .px(px(6.))
        .rounded(px(radius::CHIP))
        .bg(colours::selected())
        .text_size(px(11.))
        .text_color(colours::muted())
        .child(format!("×{count}"))
}

fn bell_flash() -> Div {
    div()
        .absolute()
        .inset_0()
        .rounded(px(radius::CARD))
        .border_2()
        .border_color(colours::working())
        .bg(ui::glass::tint(colours::working(), 0.07))
}

fn overlay(id: impl Into<ElementId>, way: Way, card: impl IntoElement) -> impl IntoElement {
    let backdrop = div().absolute().inset_0().occlude().flex().items_center().justify_center().child(card);
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

fn dock_card(kind: PanelKind, view: AnyView, store: &Entity<Store>, width: f32) -> Div {
    let card = div()
        .size_full()
        .flex()
        .flex_col()
        .rounded(px(radius::CARD))
        .bg(colours::surface())
        .border_1()
        .border_color(colours::border())
        .child(dock_header(kind, store))
        .child(div().flex_1().overflow_hidden().child(view));
    div().w(px(width)).h_full().flex_none().flex().pr(px(GAP)).child(card)
}

fn dock_header(kind: PanelKind, store: &Entity<Store>) -> Div {
    let store = store.clone();
    div()
        .h(px(DOCK_HEADER))
        .flex_none()
        .flex()
        .items_center()
        .justify_between()
        .pl(px(14.))
        .pr(px(10.))
        .border_b_1()
        .border_color(colours::border())
        .child(panel_title(kind))
        .child(ui::icon_button("dock-close", "✕").on_click(move |_: &ClickEvent, _, cx| {
            store.update(cx, Store::close_dock);
        }))
}

fn sheet_card(kind: PanelKind, view: AnyView, store: &Entity<Store>) -> Stateful<Div> {
    ui::glass::card(radius::SHEET)
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

pub(crate) fn tilde(path: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    match path.strip_prefix(&home) {
        Some(rest) if !home.is_empty() => format!("~{rest}"),
        _ => path.to_string(),
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
        let (sheet, dialog) = {
            let s = self.store.read(cx);
            (s.sheet, s.dialog.clone())
        };
        let sheet = sheet.map(|kind| self.render_sheet(kind, window, cx));
        div()
            .size_full()
            .relative()
            .flex()
            .flex_row()
            .bg(colours::glass())
            .text_color(colours::text())
            .text_size(px(13.))
            .font_family(".SystemUIFont")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .child(self.render_rail(cx))
            .child(self.render_main(window, cx))
            .children(sheet)
            .children(dialog.map(|view| {
                let id = view.entity_id();
                overlay(
                    ("dialog-in-veil", id),
                    Way::In,
                    motion::slide(div().occlude().child(view), ("dialog-in", id), Way::In, (0., DIALOG_RISE)),
                )
            }))
            .children(self.render_leaving(window, cx))
            .child(crate::tour::Guide::get(cx))
            .child(self.render_toasts(cx))
    }
}

#[cfg(test)]
mod tests;
