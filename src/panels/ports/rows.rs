use std::collections::HashSet;

use gpui::{
    AnyElement, App, ClickEvent, ClipboardItem, Context, Div, FontWeight, IntoElement, ParentElement as _,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use super::PortsPanel;
use super::tunnel::{tunnel_button, tunnel_mark};
use super::view::{Go, PortView, stop_many_title, stop_many_warning, stop_title, stop_warning};
use crate::cli::Target;
use crate::dialogs::Question;
use crate::model::link::rewrite;
use crate::model::tunnels::{Toward, Way, older_note};
use crate::notify::tip;
use crate::panels::poll::{Stop, StopOrder, Watch, confirm_stop, confirm_stops};
use crate::panels::queue::open_pane;
use crate::store::{Busy, NoticeThere, Store, ToastKind};
use crate::theme::{colours, radius};
use crate::ui::{self, Icon};
use crate::{cli, desktop};

const TICK: f32 = 15.;

pub(super) fn remote_notes(host: &str, way: &Way) -> Vec<String> {
    let here = desktop::here();
    let own = format!(
        "These ports are on {host}: Open shows one in the Browser panel, through an ssh tunnel to this {here}."
    );
    match way {
        Way::Cornercase => vec![format!(
            "These ports are on {host}: Tunnel forwards one to this {here} through cornercase, which lists it, and Open \
             shows it in the Browser panel."
        )],
        Way::Own(older) => vec![own, older_note(older, host)],
        Way::Here | Way::Waiting => vec![own],
    }
}

#[derive(Clone)]
pub(super) struct Side {
    pub(super) toward: Toward,
    pub(super) target: Option<Target>,
}

impl Side {
    pub(super) fn machine(&self) -> Option<&str> {
        self.toward.machine()
    }

    pub(super) fn can_stop(&self) -> bool {
        self.machine().is_none() || self.target.is_some()
    }

    fn order(&self, busy: Busy, command: Vec<String>) -> StopOrder {
        match (self.machine(), &self.target) {
            (Some(machine), Some(target)) => StopOrder::on(machine, target.clone(), busy, command),
            _ => StopOrder::here(busy, command),
        }
    }
}

fn stopping(view: &PortView) -> String {
    format!("stop-port:{}", view.key)
}

fn ask_stop(view: &PortView, side: &Side, window: &mut Window, cx: &mut Context<PortsPanel>) {
    let title = stop_title(view, side.machine());
    let question = Question::new(title, stop_warning(view, side.machine()), "Stop", Vec::new());
    confirm_stop(question, stop_entry(view, side), window, cx);
}

fn stop_entry(view: &PortView, side: &Side) -> StopOrder {
    let there = side.machine().map(|machine| format!(" on {machine}")).unwrap_or_default();
    let text = format!("Stopping {} on port {}{there}…", view.program, view.number);
    side.order(Busy::new(stopping(view), text), view.stop_command())
}

pub(super) fn ask_stop_many(views: &[&PortView], side: &Side, window: &mut Window, cx: &mut Context<PortsPanel>) {
    if let [one] = views {
        ask_stop(one, side, window, cx);
        return;
    }
    let title = stop_many_title(views.len(), side.machine());
    let question = Question::new(title, stop_many_warning(views, side.machine()), "Stop", Vec::new());
    confirm_stops(question, views.iter().map(|v| stop_entry(v, side)).collect(), window, cx);
}

fn stop_button(view: &PortView, side: &Side, stop: Stop<PortsPanel>) -> Option<Stateful<Div>> {
    let id = SharedString::from(format!("stop-{}", view.key));
    match stop {
        Stop::Never => None,
        Stop::Stopping => Some(ui::busy_button(id, "Stopping…", ui::Tone::Danger)),
        Stop::Ask(panel) => {
            let (stopped, side) = (view.clone(), side.clone());
            Some(ui::danger_button(id, "Stop").on_click(move |_: &ClickEvent, window, cx| {
                panel.update(cx, |_, cx| ask_stop(&stopped, &side, window, cx));
            }))
        }
    }
}

fn browse(url: String, cx: &mut App) {
    let workspace = super::shown_workspace(cx);
    Store::get(cx).update(cx, |s, cx| {
        s.close_sheet(cx);
        s.open_in_browser(url, workspace, cx);
    });
}

fn system_browser(id: SharedString, url: String) -> Stateful<Div> {
    ui::tool_button(id, Icon::External, 26., false)
        .tooltip(tip("Open in the system browser"))
        .on_click(move |_: &ClickEvent, _, cx| cx.open_url(&url))
}

fn copy(url: &str, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string(url.to_string()));
    let text = format!("Copied {url}");
    Store::get(cx).update(cx, |s, cx| s.toast(text, ToastKind::Info, cx));
}

fn address_line(view: &PortView, side: &Side, store: &Store) -> Div {
    let url = view.url.clone().unwrap_or_else(|| format!("port {}", view.number));
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(div().font_weight(FontWeight::SEMIBOLD).text_color(colours::accent()).child(url))
        .child(ui::dim(view.addresses.join(", ")))
        .children(tunnel_mark(view, &side.toward, store))
        .when(view.by_agent, |d| d.child(div().text_size(px(11.)).text_color(colours::waiting()).child("agent")))
}

fn reached_url(view: &PortView, side: &Side, store: &Store) -> Option<String> {
    let url = view.url.clone()?;
    match store.tunnel_local(&side.toward, view.number) {
        Some(local) => Some(rewrite(&url, local)),
        None if side.machine().is_some() => None,
        None => Some(url),
    }
}

fn go_button(id: SharedString, go: Go) -> Stateful<Div> {
    ui::button(id, "Go").on_click(move |_: &ClickEvent, _, cx| match &go {
        Go::Here(pane) => open_pane(*pane, cx),
        Go::There { machine, boot, pane } => {
            let there = NoticeThere {
                machine: machine.clone(),
                boot: boot.clone(),
                panes: vec![*pane],
                gone: format!("That pane is no longer on {machine}"),
            };
            Store::get(cx).update(cx, |s, cx| {
                s.go_there(&there, cx);
                s.close_sheet(cx);
            });
        }
    })
}

fn buttons(view: &PortView, side: &Side, stop: Stop<PortsPanel>, store: &Store) -> Div {
    let key = view.key.clone();
    let id = |what: &str| SharedString::from(format!("{what}-{key}"));
    let reached = reached_url(view, side, store);
    let tunnelled = store.tunnel_local(&side.toward, view.number).is_some();
    let direct = side.machine().is_none() && cli::host().is_none();
    let opened = view.url.clone().filter(|_| side.machine().is_none());
    let copied = reached.clone();
    let system = reached.filter(|_| direct || tunnelled).map(|url| system_browser(id("system"), url));
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .children(
            opened.map(|url| {
                ui::button(id("open"), "Open").on_click(move |_: &ClickEvent, _, cx| browse(url.clone(), cx))
            }),
        )
        .children(system)
        .children(tunnel_button(view, &side.toward, store))
        .children(
            copied.map(|url| ui::button(id("copy"), "Copy").on_click(move |_: &ClickEvent, _, cx| copy(&url, cx))),
        )
        .children(view.go.clone().map(|go| go_button(id("go"), go)))
        .children(stop_button(view, side, stop))
}

struct Row<'a> {
    store: &'a Store,
    side: &'a Side,
    stop: Stop<PortsPanel>,
    ticked: bool,
}

fn tick(view: &PortView, row: &Row<'_>) -> AnyElement {
    let Stop::Ask(panel) = &row.stop else { return div().w(px(TICK)).flex_none().into_any_element() };
    let (panel, key) = (panel.clone(), view.key.clone());
    ui::checkbox(SharedString::from(format!("tick-{key}")), row.ticked, "")
        .w(px(TICK))
        .flex_none()
        .on_click(move |_: &ClickEvent, _, cx| panel.update(cx, |this, cx| this.toggle(&key, cx)))
        .into_any_element()
}

fn port_row(view: &PortView, row: Row<'_>) -> AnyElement {
    let tick = tick(view, &row);
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .p(px(12.))
        .rounded(px(radius::card()))
        .bg(colours::terminal())
        .border_1()
        .border_color(colours::border())
        .child(tick)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(3.))
                .child(address_line(view, row.side, row.store))
                .child(super::command_text(view.command_line.clone()))
                .child(ui::dim(format!("pid {} · {}", view.pid, view.place))),
        )
        .child(buttons(view, row.side, row.stop, row.store))
        .into_any_element()
}

pub(super) fn rows(
    views: &[PortView],
    side: &Side,
    (_all, ticked): (bool, &HashSet<String>),
    watch: &Watch,
    cx: &mut Context<PortsPanel>,
) -> Vec<AnyElement> {
    let (panel, store) = (cx.entity(), Store::get(cx).read(cx));
    let stop = |p: &PortView| Stop::of(p.stoppable && side.can_stop(), watch.stopping(&stopping(p), store), &panel);
    let row = |p: &PortView| Row { store, side, stop: stop(p), ticked: ticked.contains(&p.key) };
    views.iter().map(|p| port_row(p, row(p))).collect()
}

pub(super) fn busy_stopping(view: &PortView, watch: &Watch, store: &Store) -> bool {
    watch.stopping(&stopping(view), store)
}
