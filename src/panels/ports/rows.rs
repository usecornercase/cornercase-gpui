use std::collections::HashSet;

use gpui::{
    AnyElement, App, ClickEvent, ClipboardItem, Context, Div, FontWeight, IntoElement, ParentElement as _,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use super::PortsPanel;
use super::data::{Port, stop_many_warning, stop_warning};
use crate::dialogs::Question;
use crate::model::Session;
use crate::notify::tip;
use crate::panels::poll::{Stop, Watch, confirm_stop, confirm_stops};
use crate::panels::queue::open_pane;
use crate::store::{Busy, Store, ToastKind};
use crate::theme::{colours, radius};
use crate::ui::{self, Icon};
use crate::{cli, desktop};

const TICK: f32 = 15.;

pub(super) fn remote_note(host: &str) -> String {
    let here = desktop::here();
    format!("These ports are on {host}: Open shows one in the Browser panel, through an ssh tunnel to this {here}.")
}

fn stopping(port: &Port) -> String {
    format!("stop-port:{}", port.key())
}

fn ask_stop(port: &Port, window: &mut Window, cx: &mut Context<PortsPanel>) {
    let title = format!("Stop {} on port {}?", port.program, port.number);
    let question = Question::new(title, stop_warning(port), "Stop", Vec::new());
    let (busy, command) = stop_entry(port);
    confirm_stop(question, busy, command, window, cx);
}

fn stop_entry(port: &Port) -> (Busy, Vec<String>) {
    let busy = Busy::new(stopping(port), format!("Stopping {} on port {}…", port.program, port.number));
    (busy, port.stop_command())
}

pub(super) fn ask_stop_many(ports: &[&Port], window: &mut Window, cx: &mut Context<PortsPanel>) {
    if let [one] = ports {
        ask_stop(one, window, cx);
        return;
    }
    let question =
        Question::new(format!("Stop {} programs?", ports.len()), stop_many_warning(ports), "Stop", Vec::new());
    confirm_stops(question, ports.iter().map(|p| stop_entry(p)).collect(), window, cx);
}

fn stop_button(port: &Port, stop: Stop<PortsPanel>) -> Option<Stateful<Div>> {
    let id = SharedString::from(format!("stop-{}", port.key()));
    match stop {
        Stop::Never => None,
        Stop::Stopping => Some(ui::busy_button(id, "Stopping…", ui::Tone::Danger)),
        Stop::Ask(panel) => {
            let stopped = port.clone();
            Some(ui::danger_button(id, "Stop").on_click(move |_: &ClickEvent, window, cx| {
                panel.update(cx, |_, cx| ask_stop(&stopped, window, cx));
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

fn address_line(port: &Port) -> Div {
    let url = port.url.clone().unwrap_or_else(|| format!("port {}", port.number));
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(div().font_weight(FontWeight::SEMIBOLD).text_color(colours::accent()).child(url))
        .child(ui::dim(port.addresses.join(", ")))
        .when(port.by_agent(), |d| d.child(div().text_size(px(11.)).text_color(colours::waiting()).child("agent")))
}

fn buttons(port: &Port, stop: Stop<PortsPanel>) -> Div {
    let key = port.key();
    let id = |what: &str| SharedString::from(format!("{what}-{key}"));
    let local = cli::host().is_none();
    let (opened, copied) = (port.url.clone(), port.url.clone());
    let elsewhere = port.url.clone().filter(|_| local).map(|url| system_browser(id("system"), url));
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .children(
            opened.map(|url| {
                ui::button(id("open"), "Open").on_click(move |_: &ClickEvent, _, cx| browse(url.clone(), cx))
            }),
        )
        .children(elsewhere)
        .children(
            copied.map(|url| ui::button(id("copy"), "Copy").on_click(move |_: &ClickEvent, _, cx| copy(&url, cx))),
        )
        .children(
            port.pane.map(|pane| ui::button(id("go"), "Go").on_click(move |_: &ClickEvent, _, cx| open_pane(pane, cx))),
        )
        .children(stop_button(port, stop))
}

struct Row<'a> {
    session: Option<&'a Session>,
    all: bool,
    stop: Stop<PortsPanel>,
    ticked: bool,
}

fn tick(port: &Port, row: &Row<'_>) -> AnyElement {
    let Stop::Ask(panel) = &row.stop else { return div().w(px(TICK)).flex_none().into_any_element() };
    let (panel, key) = (panel.clone(), port.key());
    ui::checkbox(SharedString::from(format!("tick-{key}")), row.ticked, "")
        .w(px(TICK))
        .flex_none()
        .on_click(move |_: &ClickEvent, _, cx| panel.update(cx, |this, cx| this.toggle(&key, cx)))
        .into_any_element()
}

fn port_row(port: &Port, row: Row<'_>) -> AnyElement {
    let tick = tick(port, &row);
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
                .child(address_line(port))
                .child(super::command_line(port))
                .child(ui::dim(format!("pid {} · {}", port.pid, port.place(row.session, row.all)))),
        )
        .child(buttons(port, row.stop))
        .into_any_element()
}

pub(super) fn rows(
    ports: &[Port],
    (all, ticked): (bool, &HashSet<String>),
    watch: &Watch,
    cx: &mut Context<PortsPanel>,
) -> Vec<AnyElement> {
    let (panel, store) = (cx.entity(), Store::get(cx).read(cx));
    let stop = |p: &Port| Stop::of(p.stoppable(), watch.stopping(&stopping(p), store), &panel);
    let row = |p: &Port| Row { session: store.session.as_ref(), all, stop: stop(p), ticked: ticked.contains(&p.key()) };
    ports.iter().map(|p| port_row(p, row(p))).collect()
}

pub(super) fn busy_stopping(port: &Port, watch: &Watch, store: &Store) -> bool {
    watch.stopping(&stopping(port), store)
}
