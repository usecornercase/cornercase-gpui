pub(crate) mod data;
mod rows;

use std::time::Duration;

use gpui::{
    AnyElement, AnyView, App, AppContext as _, ClickEvent, Context, Div, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use crate::cli;
use crate::model::Session;
use crate::panels::PanelKind;
use crate::panels::poll::{Polled, Watch, parse};
use crate::store::{self, Store};
use crate::theme::{FONT_FAMILY, colours};
use crate::ui;
use data::{List, Port, list_command};

const NO_PORTS: &str = "Nothing in this workspace listens on a TCP port.";
const NO_WORKSPACE: &str = "No workspace on screen. Show one, or list every port.";
const DOCKER_NOTE: &str = "Ports Docker publishes are held by Docker, so they show only under every port.";

pub(crate) struct PortsPanel {
    ports: Option<Vec<Port>>,
    error: Option<String>,
    all: bool,
    workspace: Option<u64>,
    watch: Watch,
}

fn shown_workspace(cx: &App) -> Option<u64> {
    Store::get(cx).read(cx).session.as_ref().map(Session::shown).and_then(|s| s.workspace)
}

impl Polled for PortsPanel {
    const KIND: PanelKind = PanelKind::Ports;
    const EVERY: Duration = Duration::from_secs(2);

    fn watch(&mut self) -> &mut Watch {
        &mut self.watch
    }

    fn measure(&mut self, cx: &mut Context<Self>) {
        let workspace = shown_workspace(cx);
        if workspace != self.workspace {
            self.workspace = workspace;
            self.ports = None;
        }
        if !self.all && workspace.is_none() {
            self.ports = Some(Vec::new());
            cx.notify();
            return;
        }
        if !self.watch.begin() {
            return;
        }
        let scope = (self.all, workspace);
        store::run_json(cx, list_command(self.all, workspace), move |this, result, cx| {
            this.watch.end();
            if (this.all, this.workspace) != scope {
                this.measure(cx);
                return;
            }
            match parse::<List>(result) {
                Ok(list) => (this.ports, this.error) = (Some(list.ports), None),
                Err(e) => this.error = Some(e),
            }
            cx.notify();
        });
    }
}

impl PortsPanel {
    fn new(cx: &mut Context<Self>) -> Self {
        let watch = Watch::start(cx);
        let mut this = Self { ports: None, error: None, all: false, workspace: None, watch };
        this.measure(cx);
        this
    }

    fn set_all(&mut self, all: bool, cx: &mut Context<Self>) {
        self.all = all;
        self.ports = None;
        self.measure(cx);
        cx.notify();
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let all = self.all;
        let tabs = vec![
            ui::segment("scope-workspace", "This workspace", !all)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_all(false, cx)))
                .into_any_element(),
            ui::segment("scope-all", "Every port", all)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_all(true, cx)))
                .into_any_element(),
        ];
        let count = self.ports.as_ref().map(|p| format!("{} listening", p.len()));
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(ui::segments(tabs))
            .child(div().flex_1())
            .children(count.map(ui::dim))
    }

    fn list(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        match (&self.ports, &self.error) {
            (_, Some(e)) => vec![ui::note(e.clone()).text_color(colours::failed()).into_any_element()],
            (None, None) => vec![ui::loading("Looking for ports…").into_any_element()],
            (Some(_), None) if !self.all && self.workspace.is_none() => vec![ui::note(NO_WORKSPACE).into_any_element()],
            (Some(ports), None) if ports.is_empty() => vec![ui::note(NO_PORTS).into_any_element()],
            (Some(ports), None) => rows::rows(ports, self.all, &self.watch, cx),
        }
    }
}

impl Render for PortsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let toolbar = self.toolbar(cx);
        let remote = cli::host().map(|host| ui::dim(rows::remote_note(&host)));
        div()
            .id("ports")
            .size_full()
            .overflow_y_scroll()
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(toolbar)
            .children(remote)
            .children(self.list(cx))
            .child(ui::dim(DOCKER_NOTE))
    }
}

pub(crate) fn command_line(port: &Port) -> Div {
    let text = ui::one_line(port.command_line());
    div().font_family(FONT_FAMILY).text_size(px(12.)).text_color(colours::soft()).child(text)
}

pub(crate) fn build(_: &mut Window, cx: &mut App) -> AnyView {
    cx.new(PortsPanel::new).into()
}

#[cfg(test)]
mod tests;
