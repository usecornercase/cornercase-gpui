use gpui::{
    AnyElement, App, ClickEvent, ClipboardItem, Context, Div, FontWeight, IntoElement, ParentElement as _,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use super::PortsPanel;
use super::data::{Port, stop_warning};
use crate::cli;
use crate::dialogs::Question;
use crate::model::Session;
use crate::notify::tip;
use crate::panels::poll::{Stop, Watch, confirm_stop};
use crate::panels::queue::open_pane;
use crate::store::{Busy, Store, ToastKind};
use crate::theme::{colours, radius};
use crate::ui::{self, Icon};

pub(super) fn remote_note(host: &str) -> String {
    format!("These ports are on {host}: Open shows one in the Browser panel, through an ssh tunnel to this Mac.")
}

fn stopping(port: &Port) -> String {
    format!("stop-port:{}", port.key())
}

fn ask_stop(port: &Port, window: &mut Window, cx: &mut Context<PortsPanel>) {
    let title = format!("Stop {} on port {}?", port.program, port.number);
    let question = Question::new(title, stop_warning(port), "Stop", Vec::new());
    let busy = Busy::new(stopping(port), format!("Stopping {} on port {}…", port.program, port.number));
    confirm_stop(question, busy, port.stop_command(), window, cx);
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

fn port_row(port: &Port, session: Option<&Session>, all: bool, stop: Stop<PortsPanel>) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .p(px(12.))
        .rounded(px(radius::CARD))
        .bg(colours::terminal())
        .border_1()
        .border_color(colours::border())
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(3.))
                .child(address_line(port))
                .child(super::command_line(port))
                .child(ui::dim(format!("pid {} · {}", port.pid, port.place(session, all)))),
        )
        .child(buttons(port, stop))
        .into_any_element()
}

pub(super) fn rows(ports: &[Port], all: bool, watch: &Watch, cx: &mut Context<PortsPanel>) -> Vec<AnyElement> {
    let (panel, store) = (cx.entity(), Store::get(cx).read(cx));
    let stop = |p: &Port| Stop::of(p.stoppable(), watch.stopping(&stopping(p), store), &panel);
    ports.iter().map(|p| port_row(p, store.session.as_ref(), all, stop(p))).collect()
}
