pub(crate) mod data;
mod rows;
mod there;
mod tunnel;
mod view;

use std::collections::HashSet;
use std::time::Duration;

use gpui::{
    AnyElement, AnyView, App, AppContext as _, ClickEvent, Context, Div, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use crate::cli;
use crate::model::tunnels::Toward;
use crate::model::{Session, ThereId};
use crate::panels::PanelKind;
use crate::panels::on_machine::{self, Retry};
use crate::panels::poll::{Polled, Watch, parse};
use crate::store::{self, Store};
use crate::theme::{FONT_FAMILY, colours};
use crate::ui;
use crate::ui::counting::Counting;
use data::{List, Port, list_command};
use rows::Side;
use there::{Scope, Source, machine_notes, scope_of, source_of, views};
use view::{PortView, stop_label, to_stop};

const NO_PORTS: &str = "Nothing in this workspace listens on a TCP port.";
const NO_WORKSPACE: &str = "No workspace on screen. Show one, or list every port.";
const DOCKER_NOTE: &str = "Ports Docker publishes are held by Docker, so they show only under every port.";

pub(crate) enum Listed {
    None,
    Here(Vec<Port>),
    There(Vec<Port<ThereId>>),
}

pub(crate) struct PortsPanel {
    listed: Listed,
    error: Option<String>,
    all: bool,
    scope: Option<Scope>,
    ticked: HashSet<String>,
    watch: Watch,
    retry: Retry,
    counting: Counting,
}

fn shown_workspace(cx: &App) -> Option<u64> {
    Store::get(cx).read(cx).session.as_ref().map(Session::shown).and_then(|s| s.workspace)
}

fn counting(panel: &mut PortsPanel) -> &mut Counting {
    &mut panel.counting
}

impl Polled for PortsPanel {
    const KIND: PanelKind = PanelKind::Ports;
    const EVERY: Duration = Duration::from_secs(2);

    fn watch(&mut self) -> &mut Watch {
        &mut self.watch
    }

    fn measure(&mut self, cx: &mut Context<Self>) {
        let source = source_of(Store::get(cx).read(cx).machine_read(), shown_workspace(cx));
        let scope = scope_of(&source, self.all);
        if self.scope.as_ref() != Some(&scope) {
            self.scope = Some(scope);
            self.listed = Listed::None;
            self.error = None;
            self.retry = Retry::default();
            self.ticked.clear();
        }
        match source {
            Source::Here(workspace) => self.measure_here(workspace, cx),
            Source::There(read) => self.measure_there(&read, cx),
        }
    }
}

impl PortsPanel {
    fn new(cx: &mut Context<Self>) -> Self {
        let watch = Watch::start(cx);
        let mut this = Self {
            listed: Listed::None,
            error: None,
            all: false,
            scope: None,
            ticked: HashSet::new(),
            watch,
            retry: Retry::default(),
            counting: Counting::default(),
        };
        this.measure(cx);
        this
    }

    fn measure_here(&mut self, workspace: Option<u64>, cx: &mut Context<Self>) {
        if !self.all && workspace.is_none() {
            self.listed = Listed::Here(Vec::new());
            cx.notify();
            return;
        }
        if !self.watch.begin() {
            return;
        }
        let scope = self.scope.clone();
        store::run_json(cx, list_command(self.all, workspace), move |this, result, cx| {
            this.watch.end();
            if this.scope != scope {
                this.measure(cx);
                return;
            }
            match parse::<List>(result) {
                Ok(list) => this.listed_here(list.ports),
                Err(e) => this.error = Some(e),
            }
            cx.notify();
        });
    }

    fn listed_here(&mut self, ports: Vec<Port>) {
        let key = |p: &Port| format!("{}-{}", p.number, p.pid);
        self.ticked.retain(|ticked| ports.iter().any(|p| key(p) == *ticked));
        self.listed = Listed::Here(ports);
        self.error = None;
    }

    fn toggle(&mut self, key: &str, cx: &mut Context<Self>) {
        if !self.ticked.remove(key) {
            self.ticked.insert(key.to_string());
        }
        cx.notify();
    }

    fn stop_many(&self, views: &[PortView], side: &Side, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let store = Store::get(cx);
        let listed: Vec<PortView> = to_stop(views, &self.ticked, self.all)
            .into_iter()
            .filter(|p| side.can_stop() && !rows::busy_stopping(p, &self.watch, store.read(cx)))
            .cloned()
            .collect();
        let side = side.clone();
        (!listed.is_empty()).then(|| {
            let label = stop_label(listed.len(), !self.ticked.is_empty());
            ui::danger_button("ports-stop-many", label).on_click(cx.listener(move |_, _: &ClickEvent, window, cx| {
                rows::ask_stop_many(&listed.iter().collect::<Vec<_>>(), &side, window, cx);
            }))
        })
    }

    fn set_all(&mut self, all: bool, cx: &mut Context<Self>) {
        self.all = all;
        self.listed = Listed::None;
        self.measure(cx);
        cx.notify();
    }

    fn toolbar(&self, views: Option<&[PortView]>, side: &Side, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let all = self.all;
        let tabs = vec![
            ui::segment("scope-workspace", "This workspace", !all)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_all(false, cx)))
                .into_any_element(),
            ui::segment("scope-all", "Every port", all)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_all(true, cx)))
                .into_any_element(),
        ];
        let count = views.map(|v| format!("{} listening", v.len()));
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(ui::segments(tabs))
            .child(div().flex_1())
            .children(count.map(ui::dim))
            .children(views.and_then(|v| self.stop_many(v, side, cx)))
    }

    fn list(&self, views: &[PortView], source: &Source, side: &Side, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let loaded = !matches!(self.listed, Listed::None);
        match source {
            Source::There(read) => match (loaded, on_machine::may_ask(read, &self.retry)) {
                (false, Err(_)) => vec![ui::note(on_machine::nothing_read(read)).into_any_element()],
                (false, Ok(_)) => vec![ui::loading("Looking for ports…").into_any_element()],
                (true, _) if views.is_empty() => vec![ui::note(NO_PORTS).into_any_element()],
                (true, _) => rows::rows(views, side, (self.all, &self.ticked), &self.watch, cx),
            },
            Source::Here(workspace) => match (loaded, &self.error) {
                (_, Some(e)) => vec![ui::note(e.clone()).text_color(colours::failed()).into_any_element()],
                (false, None) => vec![ui::loading("Looking for ports…").into_any_element()],
                (true, None) if !self.all && workspace.is_none() => vec![ui::note(NO_WORKSPACE).into_any_element()],
                (true, None) if views.is_empty() => vec![ui::note(NO_PORTS).into_any_element()],
                (true, None) => rows::rows(views, side, (self.all, &self.ticked), &self.watch, cx),
            },
        }
    }

    fn notes(source: &Source, cx: &Context<Self>) -> Vec<String> {
        let store = Store::get(cx).read(cx);
        match source {
            Source::There(read) => machine_notes(&store.machine_tunnel_gate(&read.aim.machine), &read.aim.machine),
            Source::Here(_) => {
                cli::host().map(|host| rows::remote_notes(&host, &store.tunnel_way())).unwrap_or_default()
            }
        }
    }
}

fn side_of(source: &Source) -> Side {
    match source {
        Source::Here(_) => Side { toward: Toward::Server, target: None },
        Source::There(read) => {
            Side { toward: Toward::Machine(read.aim.machine.clone()), target: read.reach.target().cloned() }
        }
    }
}

struct Drawn {
    source: Source,
    side: Side,
    views: Vec<PortView>,
    fresh: bool,
}

impl PortsPanel {
    fn drawn(&self, cx: &Context<Self>) -> Drawn {
        let source = source_of(Store::get(cx).read(cx).machine_read(), shown_workspace(cx));
        let fresh = self.scope.as_ref() == Some(&scope_of(&source, self.all));
        let aim = match &source {
            Source::There(read) => Some(read.aim.clone()),
            Source::Here(_) => None,
        };
        let session = Store::get(cx).read(cx).session.clone();
        let views = if fresh { views(&self.listed, session.as_ref(), aim.as_ref(), self.all) } else { Vec::new() };
        Drawn { side: side_of(&source), source, views, fresh }
    }

    fn body(&self, drawn: &Drawn, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let loaded = drawn.fresh && !matches!(self.listed, Listed::None);
        let toolbar = self.toolbar(loaded.then_some(drawn.views.as_slice()), &drawn.side, cx);
        let notes = Self::notes(&drawn.source, cx);
        let list = if drawn.fresh {
            self.list(&drawn.views, &drawn.source, &drawn.side, cx)
        } else {
            vec![ui::loading("Looking for ports…").into_any_element()]
        };
        div()
            .id("ports")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(toolbar)
            .children(notes.into_iter().map(ui::dim))
            .children(list)
            .child(ui::dim(DOCKER_NOTE))
    }
}

impl Render for PortsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let drawn = self.drawn(cx);
        let strip = match &drawn.source {
            Source::There(read) => Some(on_machine::strip(read, &self.retry, &self.counting, false)),
            Source::Here(_) => None,
        };
        let panel = div().size_full().flex().flex_col().children(strip).child(self.body(&drawn, cx));
        self.counting.keep(counting, cx);
        panel
    }
}

pub(crate) fn command_line(port: &Port) -> Div {
    command_text(port.command_line())
}

fn command_text(text: String) -> Div {
    div().font_family(FONT_FAMILY).text_size(px(12.)).text_color(colours::soft()).child(ui::one_line(text))
}

pub(crate) fn build(_: &mut Window, cx: &mut App) -> AnyView {
    cx.new(PortsPanel::new).into()
}

#[cfg(test)]
mod tests;
