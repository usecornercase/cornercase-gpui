//! The window: the sidebar of projects and workspaces, the tabs of the shown workspace, the
//! terminal surface, a docked panel, sheets, dialogs, toasts and a status bar. All of it is drawn
//! from what cornercase prints (`view`, `surface`, `status --app`…), and every action is a
//! `cornercase` command.
//!
//! The rest of `Root` lives beside this file: `terminal.rs` (keys, mouse, drawing the surface),
//! `sidebar.rs` and `tabs.rs`.

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use futures::StreamExt as _;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui::{
    AnyView, App, AppContext as _, ClickEvent, ClipboardItem, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Pixels, Point, Render, SharedString, StatefulInteractiveElement as _, Styled, Window,
    div, prelude::FluentBuilder as _, px,
};

use crate::cli::{self, Queue, Stream};
use crate::model::{Applied, Frame, Grid, Project, Session, Workspace};
use crate::notify::{self, Notifier};
use crate::panels::PanelKind;
use crate::render::Metrics;
use crate::store::{Store, ToastKind};
use crate::theme::{self, colours};
use crate::ui;

pub const SIDEBAR_WIDTH: f32 = 248.;
pub const TITLEBAR_HEIGHT: f32 = 38.;
pub const DOCK_WIDTH: f32 = 420.;
pub const PADDING: f32 = 6.;
pub const FONT_FAMILY: &str = "Menlo";

pub enum Msg {
    Started(Result<String, String>),
    Session(u64, Box<Session>),
    ViewEnded(u64, String),
    Frame(u64, Box<Frame>),
    SurfaceEnded(u64, String),
    Error(String),
    Copied(String),
    NoVisibility,
}

#[derive(Clone, PartialEq)]
pub enum Conn {
    Starting,
    Connected,
    Lost(String),
}

pub enum Drag {
    /// The program reads the mouse: presses, drags and releases go to it.
    Program { pane: u64, motion: bool },
    /// cornercase's own selection, in cells of the pane.
    Select { pane: u64, from: (u16, u16), to: (u16, u16) },
    /// A divider of the tab's split.
    Divider { tab: u64, path: Vec<bool>, down: bool, area: crate::model::Area },
}

pub struct Root {
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
    pub(crate) metrics: Option<Metrics>,
    pub(crate) font_size: f32,
    pub(crate) origin: Rc<Cell<Option<Point<Pixels>>>>,
    pub(crate) size: Option<(u16, u16)>,
    pub(crate) drag: Option<Drag>,
    pub(crate) scroll_rest: f32,
    pub(crate) marked: String,
    pub(crate) conn: Conn,
    pub(crate) confirm_close: Option<u64>,
    /// Whether to ask for patches: cornercase 0.12.189 refuses `surface --view --patches` (its
    /// feature check sends the view along, usecornercase/cornercase-terminal#461), so a refusal
    /// falls back to complete frames.
    pub(crate) patches: bool,
    pub(crate) report_visibility: bool,
    pub(crate) visible: bool,
    /// Panels built so far, kept so each keeps its state while closed.
    pub(crate) panels: HashMap<PanelKind, AnyView>,
    had_overlay: bool,
    /// Agent notices, host events and the app state of the status bar.
    notifier: Entity<Notifier>,
    /// The window's title as last set.
    title: String,
}

impl Focusable for Root {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Root {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
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
            while let Some(msg) = rx.next().await {
                if this.update(cx, |this, cx| this.handle(msg, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        // Redraw on every change of the shared state, and give the keys back to the terminal
        // once a dialog or sheet closes.
        cx.observe_in(&store, window, |this, store, window, cx| {
            let s = store.read(cx);
            let overlay = s.dialog.is_some() || s.sheet.is_some();
            if this.had_overlay && !overlay {
                window.focus(&this.focus);
            }
            this.had_overlay = overlay;
            cx.notify();
        })
        .detach();
        cx.observe_window_activation(window, |this, window, cx| {
            let active = window.is_window_active();
            if active != this.visible {
                this.visible = active;
                if let Some(id) = this.view_id
                    && this.report_visibility
                {
                    let flag = if active { "--visible" } else { "--hidden" };
                    let tx = this.tx.clone();
                    let args = vec!["view-set".into(), "--view".into(), id.to_string(), flag.into()];
                    this.queue.cmd_then(args, move |r| {
                        if r.is_err() {
                            let _ = tx.unbounded_send(Msg::NoVisibility);
                        }
                    });
                }
            }
            cx.notify();
        })
        .detach();

        let mut root = Self {
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
            metrics: None,
            font_size: std::env::var("CORNERCASE_GPUI_FONT_SIZE").ok().and_then(|s| s.parse().ok()).unwrap_or(13.),
            origin: Rc::new(Cell::new(None)),
            size: None,
            drag: None,
            scroll_rest: 0.,
            marked: String::new(),
            conn: Conn::Starting,
            confirm_close: None,
            patches: true,
            report_visibility: true,
            visible: true,
            panels: HashMap::new(),
            had_overlay: false,
            notifier,
            title: String::new(),
        };
        // For checking a panel by eye: CORNERCASE_GPUI_OPEN=changes opens it at start.
        if let Ok(name) = std::env::var("CORNERCASE_GPUI_OPEN")
            && let Some(kind) = PanelKind::ALL.into_iter().find(|k| k.title().eq_ignore_ascii_case(&name))
        {
            root.store.update(cx, |s, cx| s.open(kind, cx));
        }
        root.start();
        root
    }

    // -----------------------------------------------------------------------------------------
    // Talking to cornercase.

    /// `cornercase start-server` finds the running server or starts one, and gives the terminals
    /// our colours; then the view opens.
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
        let (cols, rows) = self.size.unwrap_or((120, 36));
        let mut args = vec!["view".to_string(), "--cols".into(), cols.to_string(), "--rows".into(), rows.to_string()];
        // The first view starts where the command line says (`--tab 7`), else on what the window shows.
        if self.generation == 1 {
            args.extend(start_place());
        }
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
        self.store.update(cx, |s, cx| s.follow_app(cx));
    }

    fn open_surface(&mut self, view: u64, cx: &mut Context<Self>) {
        let generation = self.generation;
        let mut args = vec!["surface".to_string(), "--view".into(), view.to_string()];
        if self.patches {
            args.push("--patches".into());
        }
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
            Err(e) => self.say(e.to_string(), cx),
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
        match msg {
            Msg::Started(Ok(_)) => self.open_view(cx),
            Msg::Started(Err(e)) => {
                self.lost(e, cx);
                self.retry(cx);
            }
            Msg::Session(generation, session) if generation == self.generation => {
                self.conn = Conn::Connected;
                let id = session.view.as_ref().map(|v| v.id);
                if id.is_some() && id != self.view_id {
                    self.view_id = id;
                    if let Some(id) = id {
                        self.open_surface(id, cx);
                        if let Some((cols, rows)) = self.size
                            && session.view.as_ref().is_some_and(|v| v.cols != Some(cols) || v.rows != Some(rows))
                        {
                            self.send_size(cols, rows);
                        }
                    }
                }
                let view_id = self.view_id;
                let shared = (*session).clone();
                self.store.update(cx, |s, cx| {
                    s.session = Some(shared);
                    s.view_id = view_id;
                    cx.notify();
                });
                self.session = Some(*session);
            }
            Msg::ViewEnded(generation, err) if generation == self.generation => {
                let why = err.lines().last().unwrap_or("the server stopped").to_string();
                self.lost(if why.is_empty() { "the server stopped".into() } else { why }, cx);
                self.retry(cx);
            }
            Msg::Frame(generation, frame) if generation == self.generation => {
                if let Applied::NeedRedraw = Rc::make_mut(&mut self.grid).apply(*frame) {
                    let surface = self.grid.surface;
                    self.queue.coalesced("redraw", vec!["redraw".into(), "--surface".into(), surface.to_string()]);
                } else {
                    self.has_frame = true;
                }
            }
            Msg::SurfaceEnded(generation, err) if generation == self.generation => {
                self.has_frame = false;
                if self.patches && err.contains("patches") {
                    self.patches = false;
                } else if !err.trim().is_empty() {
                    self.say(err.trim().to_string(), cx);
                }
                // The view lives on (it settles on a neighbour when its tab closes): stream again.
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(Duration::from_millis(400)).await;
                    let _ = this.update(cx, |this, cx| {
                        if this.generation == generation
                            && this.surface_stream.is_some()
                            && let Some(id) = this.view_id
                        {
                            this.open_surface(id, cx);
                            cx.notify();
                        }
                    });
                })
                .detach();
            }
            Msg::Error(e) => self.say(e, cx),
            Msg::NoVisibility => self.report_visibility = false,
            Msg::Copied(text) => {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                self.store.update(cx, |s, cx| s.toast("Copied", ToastKind::Info, cx));
            }
            _ => return,
        }
        cx.notify();
    }

    fn retry(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(1500)).await;
            let _ = this.update(cx, |this, cx| {
                if matches!(this.conn, Conn::Lost(_)) {
                    this.start();
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// An error or a short message, as a toast.
    pub(crate) fn say(&mut self, text: String, cx: &mut Context<Self>) {
        let text = text.lines().next().unwrap_or_default().to_string();
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

    fn panel(&mut self, kind: PanelKind, window: &mut Window, cx: &mut Context<Self>) -> AnyView {
        if let Some(view) = self.panels.get(&kind) {
            return view.clone();
        }
        let view = kind.build(window, cx);
        self.panels.insert(kind, view.clone());
        view
    }

    // -----------------------------------------------------------------------------------------
    // Drawing the chrome around the terminal.

    fn render_dock(&mut self, kind: PanelKind, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let view = self.panel(kind, window, cx);
        let store = self.store.clone();
        div()
            .w(px(DOCK_WIDTH))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(colours::sidebar())
            .border_l_1()
            .border_color(colours::border())
            .child(
                div()
                    .h(px(TITLEBAR_HEIGHT))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(12.))
                    .border_b_1()
                    .border_color(colours::border())
                    .child(ui::title(kind.title()))
                    .child(ui::icon_button("dock-close", "✕").on_click(move |_: &ClickEvent, _, cx| {
                        store.update(cx, |s, cx| {
                            s.dock = None;
                            cx.notify();
                        })
                    })),
            )
            .child(div().flex_1().overflow_hidden().child(view))
    }

    fn render_sheet(&mut self, kind: PanelKind, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let view = self.panel(kind, window, cx);
        let store = self.store.clone();
        let close = store.clone();
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x0000_0088))
            .child(
                div()
                    .id("sheet")
                    .w(gpui::relative(0.82))
                    .h(gpui::relative(0.86))
                    .max_w(px(1100.))
                    .flex()
                    .flex_col()
                    .rounded(px(10.))
                    .bg(colours::panel())
                    .border_1()
                    .border_color(colours::border())
                    .shadow_lg()
                    .overflow_hidden()
                    .on_key_down(move |e: &KeyDownEvent, _, cx| {
                        if e.keystroke.key == "escape" {
                            close.update(cx, |s, cx| s.close_sheet(cx));
                        }
                    })
                    .child(
                        div()
                            .h(px(42.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(px(16.))
                            .border_b_1()
                            .border_color(colours::border())
                            .child(ui::title(kind.title()))
                            .child(
                                ui::icon_button("sheet-close", "✕")
                                    .on_click(move |_: &ClickEvent, _, cx| store.update(cx, |s, cx| s.close_sheet(cx))),
                            ),
                    )
                    .child(div().flex_1().overflow_hidden().child(view)),
            )
    }

    fn render_toasts(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let toasts: Vec<_> = self.store.read(cx).toasts.iter().filter(|t| !t.expired()).cloned().collect();
        let store = self.store.clone();
        div().absolute().bottom(px(34.)).right(px(14.)).flex().flex_col().gap(px(8.)).children(toasts.into_iter().map(
            move |t| {
                let colour = match t.kind {
                    ToastKind::Info => colours::accent(),
                    ToastKind::Error => colours::failed(),
                    ToastKind::NeedsYou => colours::waiting(),
                    ToastKind::Done => colours::done(),
                };
                let id = t.id;
                let (s1, s2) = (store.clone(), store.clone());
                div()
                    .id(SharedString::from(format!("toast-{id}")))
                    .w(px(340.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(12.))
                    .py(px(9.))
                    .rounded(px(8.))
                    .bg(colours::panel())
                    .border_1()
                    .border_color(colours::border())
                    .border_l_4()
                    .shadow_lg()
                    .child(div().w(px(3.)).h(px(16.)).rounded_full().bg(colour).flex_none())
                    .child(div().flex_1().text_size(px(12.)).child(t.text.clone()))
                    .when_some(t.action.clone(), move |d, (label, args)| {
                        d.child(ui::primary_button(SharedString::from(format!("toast-go-{id}")), label).on_click(
                            move |_: &ClickEvent, _, cx| {
                                s1.update(cx, |s, cx| {
                                    s.act(args.clone());
                                    s.dismiss_toast(id, cx);
                                })
                            },
                        ))
                    })
                    .child(
                        ui::icon_button(SharedString::from(format!("toast-x-{id}")), "✕")
                            .on_click(move |_: &ClickEvent, _, cx| s2.update(cx, |s, cx| s.dismiss_toast(id, cx))),
                    )
            },
        ))
    }

    fn render_status(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let version = self.session.as_ref().map(|s| format!("cornercase {}", s.version)).unwrap_or_default();
        let (dot, conn, why) = match &self.conn {
            Conn::Starting => (colours::working(), "connecting…".to_string(), None),
            Conn::Connected => (colours::done(), version, None),
            Conn::Lost(why) => (colours::failed(), "disconnected".to_string(), Some(why.clone())),
        };
        let pane = self.active_pane().and_then(|id| self.session.as_ref()?.pane(id).cloned());
        let middle = pane
            .map(|p| {
                let program = p.agent.or(p.program).unwrap_or_else(|| "shell".into());
                format!("{program} · {}", tilde(&p.path))
            })
            .unwrap_or_default();
        let size_text = self.size.map(|(c, r)| format!("{c}×{r}")).unwrap_or_default();
        let items = self.notifier.read(cx).status_items(&self.notifier);
        let store = self.store.read(cx);
        let dock = store.dock;
        let docks = [PanelKind::Changes, PanelKind::Files, PanelKind::Todo].map(|kind| {
            let store = self.store.clone();
            ui::segment(SharedString::from(format!("dock-{}", kind.title())), kind.title(), dock == Some(kind))
                .on_click(move |_: &ClickEvent, _, cx| store.update(cx, |s, cx| s.toggle_dock(kind, cx)))
        });
        let separator = || div().w(px(1.)).h(px(12.)).flex_none().bg(colours::border());
        div()
            .h(px(26.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(10.))
            .bg(colours::sidebar())
            .border_t_1()
            .border_color(colours::border())
            .text_size(px(11.))
            .text_color(colours::faint())
            .child(
                div()
                    .id("conn")
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(div().size(px(7.)).rounded_full().bg(dot))
                    .child(conn)
                    .when_some(why, |d, why| d.tooltip(notify::tip(why))),
            )
            .child(div().flex_1().overflow_hidden().whitespace_nowrap().text_ellipsis().text_color(colours::muted()).child(middle))
            .when(!items.is_empty(), |d| d.child(div().flex_none().flex().items_center().gap(px(2.)).children(items)).child(separator()))
            .child(div().flex_none().child(size_text))
            .child(div().flex().flex_none().gap(px(2.)).children(docks))
    }
}

/// A path with the home folder as `~`.
pub fn tilde(path: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    match path.strip_prefix(&home) {
        Some(rest) if !home.is_empty() => format!("~{rest}"),
        _ => path.to_string(),
    }
}

/// `--tab ID`, `--workspace ID` or `--project ID` from our own command line, for the first view.
fn start_place() -> Vec<String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    args.windows(2)
        .find(|w| matches!(w[0].as_str(), "--tab" | "--workspace" | "--project") && w[1].parse::<u64>().is_ok())
        .map(|w| w.to_vec())
        .unwrap_or_default()
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (dock, sheet, dialog) = {
            let s = self.store.read(cx);
            (s.dock, s.sheet, s.dialog.clone())
        };
        let title = self.session.as_ref().map_or_else(|| "cornercase".to_string(), notify::window_title);
        if title != self.title {
            window.set_window_title(&title);
            self.title = title;
        }
        let flashing = self.notifier.read(cx).flashing();
        let dock_el = dock.map(|kind| self.render_dock(kind, window, cx));
        let sheet_el = sheet.map(|kind| self.render_sheet(kind, window, cx));
        let dialog_el = dialog.map(|view| {
            div().absolute().inset_0().flex().items_center().justify_center().bg(gpui::rgba(0x0000_0088)).child(view)
        });
        div()
            .size_full()
            .relative()
            .flex()
            .flex_row()
            .bg(colours::terminal())
            .text_color(colours::text())
            .text_size(px(13.))
            .font_family(".SystemUIFont")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .child(self.render_sidebar(cx))
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(self.render_tabs(cx))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_row()
                            .overflow_hidden()
                            .child(
                                div()
                                    .flex_1()
                                    .h_full()
                                    .flex()
                                    .relative()
                                    .overflow_hidden()
                                    .child(self.render_terminal(window, cx))
                                    // A bell in the shown tab: a short flash of the terminal's edge.
                                    .when(flashing, |d| {
                                        d.child(
                                            div()
                                                .absolute()
                                                .inset_0()
                                                .border_2()
                                                .border_color(colours::working())
                                                .bg(gpui::rgba(0xe0af_6812)),
                                        )
                                    }),
                            )
                            .children(dock_el),
                    )
                    .child(self.render_status(cx)),
            )
            .children(sheet_el)
            .children(dialog_el)
            .child(self.render_toasts(cx))
    }
}
