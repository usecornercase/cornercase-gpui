mod columns;
pub(crate) mod data;
mod rows;

use std::collections::HashSet;
use std::time::Duration;

use gpui::{
    AnyElement, AnyView, App, AppContext as _, ClickEvent, Context, Entity, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use crate::model::Session;
use crate::panels::PanelKind;
use crate::panels::poll::{Polled, Watch, parse};
use crate::store::{self, Store};
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent};
use columns::LIST_PAD;
use data::{Order, Sample, ordered, sample_command};

const NO_PANES: &str = "No pane runs anything here.";
const NO_MATCH: &str = "No pane, program or command matches the filter.";

pub(crate) struct ProcessesPanel {
    sample: Option<Sample>,
    error: Option<String>,
    order: Order,
    only_shown: bool,
    open: HashSet<u64>,
    held: Option<(u64, Vec<u64>)>,
    filter: Entity<TextField>,
    watch: Watch,
}

fn shown_workspace(cx: &App) -> Option<u64> {
    Store::get(cx).read(cx).session.as_ref().map(Session::shown).and_then(|s| s.workspace)
}

impl Polled for ProcessesPanel {
    const KIND: PanelKind = PanelKind::Processes;
    const EVERY: Duration = Duration::from_secs(2);

    fn watch(&mut self) -> &mut Watch {
        &mut self.watch
    }

    fn measure(&mut self, cx: &mut Context<Self>) {
        if !self.watch.begin() {
            return;
        }
        let only = self.only_shown;
        let workspace = if only { shown_workspace(cx) } else { None };
        store::run_json(cx, sample_command(workspace), move |this, result, cx| {
            this.watch.end();
            if this.only_shown != only {
                this.measure(cx);
                return;
            }
            match parse(result) {
                Ok(sample) => (this.sample, this.error) = (Some(sample), None),
                Err(e) => this.error = Some(e),
            }
            cx.notify();
        });
    }
}

impl ProcessesPanel {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| TextField::new("Filter by program, command or place", cx).clears_on_escape());
        cx.subscribe_in(&filter, window, |_, _, _: &TextFieldEvent, _, cx| cx.notify()).detach();
        let watch = Watch::start(cx);
        let mut this = Self {
            sample: None,
            error: None,
            order: Order::default(),
            only_shown: false,
            open: HashSet::new(),
            held: None,
            filter,
            watch,
        };
        this.measure(cx);
        this
    }

    fn toggle_open(&mut self, pane: u64, cx: &mut Context<Self>) {
        if !self.open.remove(&pane) {
            self.open.insert(pane);
        }
        cx.notify();
    }

    fn set_only_shown(&mut self, only: bool, cx: &mut Context<Self>) {
        if self.only_shown == only {
            return;
        }
        self.only_shown = only;
        self.sample = None;
        self.measure(cx);
        cx.notify();
    }

    fn set_order(&mut self, order: Order, cx: &mut Context<Self>) {
        self.order = order;
        self.held = None;
        cx.notify();
    }

    fn held(&self) -> Option<&[u64]> {
        let opening = self.watch.opening();
        self.held.as_ref().filter(|(at, _)| *at == opening).map(|(_, panes)| panes.as_slice())
    }

    fn hold(&mut self, hovered: bool, cx: &mut Context<Self>) {
        let shown = self.sample.as_ref().filter(|_| hovered).map(|s| {
            let panes: Vec<u64> = ordered(&s.panes, self.order, self.held()).iter().map(|p| p.pane).collect();
            (self.watch.opening(), panes)
        });
        self.held = shown;
        cx.notify();
    }

    fn scope_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let only = self.only_shown;
        let tab = |id: &'static str, label: &'static str, value: bool| {
            ui::segment(id, label, only == value)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_only_shown(value, cx)))
                .into_any_element()
        };
        ui::segments(vec![tab("scope-every", "Every pane", false), tab("scope-workspace", "This workspace", true)])
    }

    fn filter_box(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let clear = (!self.filter.read(cx).text().is_empty()).then(|| {
            ui::icon_button("processes-filter-clear", "✕")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.filter.update(cx, TextField::clear)))
        });
        let focused = self.filter.read(cx).focus_handle(cx).is_focused(window);
        div().flex_1().child(ui::search_field(self.filter.clone(), focused, clear))
    }

    fn toolbar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(LIST_PAD))
            .pt(px(14.))
            .pb(px(10.))
            .child(self.filter_box(window, cx))
            .child(self.scope_tabs(cx))
    }

    fn list(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        match (&self.sample, &self.error) {
            (_, Some(e)) => vec![ui::note(e.clone()).text_color(colours::failed()).into_any_element()],
            (None, None) => vec![ui::loading("Measuring…").into_any_element()],
            (Some(s), None) if s.panes.is_empty() => vec![ui::note(NO_PANES).into_any_element()],
            (Some(s), None) => {
                let cards = self.cards(s, cx);
                if cards.is_empty() { vec![ui::note(NO_MATCH).into_any_element()] } else { cards }
            }
        }
    }
}

impl Render for ProcessesPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let toolbar = self.toolbar(window, cx);
        let header = self.sample.as_ref().filter(|s| !s.panes.is_empty()).map(|_| self.column_header(cx));
        let footer = self.sample.as_ref().map(Self::footer);
        let list = self.list(cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(toolbar)
            .children(header)
            .child(
                div()
                    .id("processes")
                    .flex_1()
                    .overflow_y_scroll()
                    .px(px(LIST_PAD))
                    .pb(px(12.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| this.hold(*hovered, cx)))
                    .children(list),
            )
            .children(footer)
    }
}

pub(crate) fn build(window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| ProcessesPanel::new(window, cx)).into()
}

#[cfg(test)]
mod tests;
