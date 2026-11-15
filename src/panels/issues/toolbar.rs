use gpui::{
    AnyElement, ClickEvent, Context, Div, Focusable as _, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, ParentElement as _, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, TextRun,
    Window, div, prelude::FluentBuilder as _, px,
};

use super::data::{SourceInfo, SourceState, Sources, Who, capitalise};
use super::{IssuesPanel, Tab, error_note, state_dot};
use crate::notify::tip;
use crate::theme::colours;
use crate::ui::{self, Icon};

const SIDES: f32 = 16.;
const GAP: f32 = 8.;
const FILTER_LEAST: f32 = 160.;
const TEXT: f32 = 12.;
const CHEVRON: f32 = 12.;
const PEOPLE_FRAME: f32 = 26. + 6. + CHEVRON;
const CLOSED_FRAME: f32 = ui::TICK_BOX + 8.;
const CLOSED: &str = "Include closed";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    OneRow,
    ClosedBesideTabs,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Label {
    whole: f32,
    anyone: f32,
}

impl Label {
    fn least(self) -> f32 {
        self.whole.min(self.anyone)
    }
}

impl Shape {
    fn of(sheet: f32, closed: f32, people: [Label; 2]) -> Self {
        let buttons: f32 = people.iter().map(|label| PEOPLE_FRAME + label.least()).sum();
        let row = FILTER_LEAST + CLOSED_FRAME + closed + buttons + 3. * GAP;
        if sheet - 2. * SIDES >= row { Self::OneRow } else { Self::ClosedBesideTabs }
    }
}

fn who_label(field: &str, who: &Who) -> String {
    format!("{}: {}", capitalise(field), who.label())
}

fn text_width(text: &str, window: &Window) -> f32 {
    let style = window.text_style();
    let run = TextRun {
        len: text.len(),
        font: style.font(),
        color: style.color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window.text_system().shape_line(SharedString::from(text.to_string()), px(TEXT), &[run], None);
    f32::from(line.width).ceil()
}

fn frame() -> Div {
    div()
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(10.))
        .px(px(SIDES))
        .pt(px(12.))
        .pb(px(12.))
        .border_b_1()
        .border_color(colours::border())
}

impl IssuesPanel {
    pub(super) fn toolbar(&self, cx: &mut Context<Self>) -> Div {
        frame().child(self.tabs_row(None, cx))
    }

    fn labels(&self, window: &Window) -> [Label; 2] {
        let who = self.who(&self.tab);
        let fields = self.fields();
        std::array::from_fn(|i| Label {
            whole: text_width(&who_label(&fields[i], &who[i]), window),
            anyone: text_width(&who_label(&fields[i], &Who::Anyone), window),
        })
    }

    pub(super) fn toolbar_with_filters(
        &self,
        source: Option<&SourceInfo>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let failing = source.filter(|s| s.state == SourceState::Failing).and_then(|s| s.reason.clone());
        let labels = self.labels(window);
        let shape = Shape::of(self.sheet.get(), text_width(CLOSED, window), labels);
        let closed = self.closed_box(cx);
        let (beside, within) = match shape {
            Shape::ClosedBesideTabs => (Some(closed), None),
            Shape::OneRow => (None, Some(closed)),
        };
        frame()
            .child(self.tabs_row(beside, cx))
            .child(self.filters_row(within, labels, window, cx))
            .children(failing.map(error_note))
    }

    fn closed_box(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        ui::checkbox("issues-closed", self.closed, CLOSED)
            .flex_none()
            .text_size(px(TEXT))
            .text_color(colours::muted())
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_closed(cx)))
    }

    fn tabs_row(&self, closed: Option<Stateful<Div>>, cx: &mut Context<Self>) -> Div {
        let any = self.sources.as_ref().is_some_and(Sources::any_connected);
        let mut items: Vec<AnyElement> = vec![
            ui::segment("tab-all", "All", self.tab == Tab::All)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_tab(Tab::All, cx)))
                .when(!any, |d| d.text_color(colours::faint()))
                .into_any_element(),
        ];
        for source in self.sources.as_ref().map(Sources::tabs).unwrap_or_default() {
            let selected = self.tab == Tab::One(source.source.clone());
            items.push(tab_segment(source, selected, cx));
        }
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(ui::segments(items).flex_wrap())
            .child(div().flex_1())
            .children(closed)
            .child(self.refresh_button(cx))
    }

    fn refresh_button(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.loading_sources || self.current().is_some_and(|l| l.loading) {
            return div()
                .size(px(28.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .child(ui::spinner("issues-loading", 14., colours::muted()))
                .into_any_element();
        }
        ui::tool_button("issues-refresh", Icon::Refresh, 28., false)
            .tooltip(tip("Ask the trackers again"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.refresh(cx)))
            .into_any_element()
    }

    fn filters_row(
        &self,
        closed: Option<Stateful<Div>>,
        labels: [Label; 2],
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut row = div()
            .flex()
            .items_center()
            .gap(px(GAP))
            .child(self.search_box(window, cx).flex_1().min_w(px(FILTER_LEAST)))
            .children(closed);
        for (index, field) in self.fields().iter().enumerate() {
            row = row.child(self.who_slot((index, labels[index].least()), field, window, cx));
        }
        row
    }

    fn search_box(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let focused = self.search.read(cx).focus_handle(cx).is_focused(window);
        let clear = (!self.search.read(cx).text().is_empty()).then(|| {
            ui::icon_button("issues-search-clear", "✕")
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.clear_search(window, cx)))
        });
        ui::search_field(self.search.clone(), focused, clear)
    }

    fn who_slot(&self, (index, least): (usize, f32), field: &str, window: &Window, cx: &mut Context<Self>) -> Div {
        let open = self.picker == Some(index);
        div()
            .relative()
            .flex()
            .child(self.who_button(index, field, least, cx))
            .when(open, |d| d.child(self.picker_popover(index, field, window, cx)))
    }

    fn who_button(&self, index: usize, field: &str, least: f32, cx: &mut Context<Self>) -> Stateful<Div> {
        let who = self.who(&self.tab)[index].clone();
        let active = who != Who::Anyone;
        let colour = if active { colours::accent() } else { colours::soft() };
        let (label, id) = (who_label(field, &who), ("issues-who", index));
        let button = if who.is_person() {
            ui::cut_button(id, label).text_color(colour).min_w(px(PEOPLE_FRAME + least))
        } else {
            ui::button_coloured(id, label, colour).flex().items_center().gap(px(6.))
        };
        button
            .child(ui::icon(Icon::ChevronDown, CHEVRON).text_color(colour))
            .when(active || self.picker == Some(index), |d| d.border_color(colours::accent()))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.toggle_picker(index, window, cx);
                }),
            )
    }
}

fn tab_segment(source: &SourceInfo, selected: bool, cx: &mut Context<IssuesPanel>) -> AnyElement {
    let tab = Tab::One(source.source.clone());
    ui::segment(SharedString::from(format!("tab-{}", source.source)), source.name.clone(), selected)
        .flex()
        .items_center()
        .gap(px(6.))
        .when(!source.connected() && !selected, |d| d.text_color(colours::faint()))
        .child(state_dot(source.state))
        .tooltip(tip(source.status_line()))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_tab(tab.clone(), cx)))
        .into_any_element()
}

#[cfg(test)]
mod tests;
