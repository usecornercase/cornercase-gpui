mod address;
mod agent;
mod agent_tools;
mod card;
mod cdp;
mod chooser;
mod chrome;
mod console;
mod console_view;
mod device_menu;
mod devices;
mod downloads;
mod element;
mod engine;
mod follow;
mod frame;
mod input;
mod keyboard;
mod notices;
mod page;
mod picker;
mod pointer;
mod ports;
mod prompts;
mod select;
mod shots;
mod socket;
mod start_page;
mod states;
mod tab_bar;
mod tabs;
mod toolbar;
mod tunnels;
mod typing;
mod viewport;
mod watch;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt as _;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui::{
    AnyView, AppContext as _, Context, CursorStyle, Entity, FocusHandle, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, RenderImage, SharedString, Styled as _, Window, div, px,
};

use crate::panels::PanelKind;
use crate::store::Store;
use crate::theme::colours;
use crate::ui::text_field::TextField;
use crate::ui::{self, Icon};
use engine::{Engine, Incoming};
use input::Input;
use notices::Notices;
use ports::DevServers;
use tabs::Tabs;
pub(crate) use viewport::View;

pub(crate) fn quit() {
    tunnels::quit();
    chrome::quit();
}

pub(crate) enum Status {
    Starting,
    Ready(Engine),
    Failed(String),
}

pub(crate) struct Browser {
    focus: FocusHandle,
    status: Status,
    tx: UnboundedSender<Incoming>,
    tabs: Tabs,
    claims: HashMap<String, Option<u64>>,
    stale: Rc<RefCell<Vec<Arc<RenderImage>>>>,
    view: View,
    resizing: u64,
    casting: Option<String>,
    workspace: Option<u64>,
    shown: bool,
    address: Entity<TextField>,
    address_url: String,
    restore: Vec<(String, Option<u64>)>,
    attempts: u32,
    cursor: CursorStyle,
    user_agent: Option<String>,
    dev: DevServers,
    agents: agent::Agents,
    devices_open: bool,
    address_pressed: bool,
    strip: tab_bar::Strip,
    input: Input,
    notices: Notices,
}

pub(crate) fn build(window: &mut Window, cx: &mut gpui::App) -> AnyView {
    cx.new(|cx| Browser::new(window, cx)).into()
}

impl Browser {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (tx, mut rx) = unbounded::<Incoming>();
        cx.spawn(async move |this, cx| {
            while let Some(msg) = rx.next().await
                && this.update(cx, |this, cx| this.incoming(msg, cx)).is_ok()
            {}
        })
        .detach();
        cx.observe(&Store::get(cx), |this, store, cx| this.store_changed(&store, cx)).detach();
        let address = cx.new(|cx| TextField::new("Address, a port such as 5173, or a search", cx));
        Self::watch_address(&address, window, cx);
        engine::start(tx.clone());
        let mut browser = Self::fresh(cx.focus_handle(), tx, address, DevServers::new(cx));
        browser.store_changed(&Store::get(cx), cx);
        browser
    }

    fn fresh(focus: FocusHandle, tx: UnboundedSender<Incoming>, address: Entity<TextField>, dev: DevServers) -> Self {
        Self {
            focus,
            status: Status::Starting,
            tx,
            tabs: Tabs::default(),
            claims: HashMap::new(),
            stale: Rc::default(),
            view: View::default(),
            resizing: 0,
            casting: None,
            workspace: None,
            shown: false,
            address,
            address_url: String::new(),
            restore: Vec::new(),
            attempts: 0,
            cursor: CursorStyle::Arrow,
            user_agent: None,
            dev,
            agents: agent::Agents::default(),
            devices_open: false,
            address_pressed: false,
            strip: tab_bar::Strip::default(),
            input: Input::default(),
            notices: Notices::default(),
        }
    }

    fn incoming(&mut self, msg: Incoming, cx: &mut Context<Self>) {
        match msg {
            Incoming::Ready(Ok(engine)) => self.ready(engine, cx),
            Incoming::Ready(Err(why)) => self.status = Status::Failed(why),
            Incoming::Event(event) => self.event(&event, cx),
            Incoming::Frame(session, shot) => self.frame(&session, shot, cx),
            Incoming::Ended(why) => self.ended(why, cx),
        }
        cx.notify();
    }

    fn store_changed(&mut self, store: &Entity<Store>, cx: &mut Context<Self>) {
        self.agents_changed(store, cx);
        let s = store.read(cx);
        let shown = s.dock == Some(PanelKind::Browser);
        let workspace = s.session.as_ref().and_then(|s| s.shown().workspace);
        if shown != self.shown || workspace != self.workspace {
            self.shown = shown;
            self.workspace = workspace;
            self.recast(cx);
            cx.notify();
        }
        self.dev_changed(store, cx);
    }

    fn ended(&mut self, why: String, cx: &mut Context<Self>) {
        let tabs = std::mem::take(&mut self.tabs);
        self.restore = tabs.list.iter().map(|t| (t.url.clone(), t.workspace)).collect();
        self.stale.borrow_mut().extend(tabs.list.into_iter().filter_map(|t| t.shot.map(|s| s.image)));
        self.casting = None;
        self.status = Status::Failed(why);
        let wait = Duration::from_secs(1u64 << self.attempts.min(4));
        self.attempts += 1;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let _ = this.update(cx, Self::restart);
        })
        .detach();
    }

    fn restart(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.status, Status::Ready(_)) {
            self.status = Status::Starting;
            engine::start(self.tx.clone());
            cx.notify();
        }
    }

    fn active_url(&self) -> Option<String> {
        self.tabs.active(self.workspace).map(|t| t.url.clone())
    }

    pub(crate) fn focus_address(&self, window: &mut Window, cx: &gpui::App) {
        window.focus(&self.address.read(cx).focus_handle(cx));
    }

    fn sync_address(&mut self, window: &Window, cx: &mut Context<Self>) {
        let url = self.active_url().unwrap_or_default();
        let focused = self.address.read(cx).focus_handle(cx).is_focused(window);
        if url != self.address_url && !focused {
            self.address_url.clone_from(&url);
            self.address.update(cx, |field, cx| field.set_text(shown_address(&url), cx));
        }
    }

    fn body(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let content = match &self.status {
            Status::Starting => Self::starting().into_any_element(),
            Status::Failed(why) => Self::failed(why, cx).into_any_element(),
            Status::Ready(_) if self.tabs.active(self.workspace).is_none() => self.start_page(cx).into_any_element(),
            Status::Ready(_) => self.viewport(window, cx).into_any_element(),
        };
        let overlay = self.agent_overlay(window, cx);
        div()
            .id("browser-body")
            .flex_1()
            .min_h_0()
            .relative()
            .overflow_hidden()
            .child(content)
            .children(self.page_cover(window, cx))
            .children(overlay)
    }
}

pub(crate) fn say(text: impl Into<String>, cx: &mut Context<Browser>) {
    Store::get(cx).update(cx, |s, cx| s.toast(text, crate::store::ToastKind::Error, cx));
}

fn notice(title: &'static str) -> gpui::Div {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(10.))
        .p(px(24.))
        .child(ui::emblem(Icon::Globe))
        .child(div().text_color(colours::soft()).child(title))
}

fn shown_address(url: &str) -> SharedString {
    if url == "about:blank" { SharedString::default() } else { SharedString::from(url.to_string()) }
}

impl Render for Browser {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_address(window, cx);
        self.sync_agents(window, cx);
        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .children(self.tab_bar(cx))
            .child(self.toolbar(window, cx))
            .children(self.device_bar(cx))
            .child(self.body(window, cx))
            .children(self.console_drawer(window, cx))
            .children(self.device_backdrop(cx))
            .on_drag_move(console_view::follow_drawer(cx))
            .track_focus(&self.panel_focus(cx))
    }
}
