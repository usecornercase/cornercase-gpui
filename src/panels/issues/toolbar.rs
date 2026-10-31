use gpui::{
    AnyElement, ClickEvent, Context, Div, Focusable as _, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, ParentElement as _, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window,
    div, prelude::FluentBuilder as _, px,
};

use super::data::{SourceInfo, SourceState, Sources, Who, capitalise};
use super::{IssuesPanel, Tab, error_note, state_dot};
use crate::notify::tip;
use crate::theme::colours;
use crate::ui::{self, Icon};

impl IssuesPanel {
    pub(super) fn toolbar(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(10.))
            .px(px(16.))
            .pt(px(12.))
            .pb(px(12.))
            .border_b_1()
            .border_color(colours::border())
            .child(self.tabs_row(cx))
    }

    pub(super) fn toolbar_with_filters(
        &self,
        source: Option<&SourceInfo>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let failing = source.filter(|s| s.state == SourceState::Failing).and_then(|s| s.reason.clone());
        self.toolbar(cx).child(self.filters_row(window, cx)).children(failing.map(error_note))
    }

    fn tabs_row(&self, cx: &mut Context<Self>) -> Div {
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

    fn filters_row(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let mut row = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(div().flex_1().min_w(px(160.)).child(self.search_box(window, cx)))
            .child(
                ui::checkbox("issues-closed", self.closed, "Include closed")
                    .flex_none()
                    .text_size(px(12.))
                    .text_color(colours::muted())
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_closed(cx))),
            );
        for (index, field) in self.fields().iter().enumerate() {
            row = row.child(self.who_slot(index, field, window, cx));
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

    fn who_slot(&self, index: usize, field: &str, window: &Window, cx: &mut Context<Self>) -> Div {
        let open = self.picker == Some(index);
        div()
            .relative()
            .flex_none()
            .child(self.who_button(index, field, cx))
            .when(open, |d| d.child(self.picker_popover(index, field, window, cx)))
    }

    fn who_button(&self, index: usize, field: &str, cx: &mut Context<Self>) -> Stateful<Div> {
        let who = self.who(&self.tab)[index].clone();
        let active = who != Who::Anyone;
        let colour = if active { colours::accent() } else { colours::soft() };
        ui::button_coloured(("issues-who", index), format!("{}: {}", capitalise(field), who.label()), colour)
            .flex()
            .items_center()
            .gap(px(6.))
            .child(ui::icon(Icon::ChevronDown, 12.).text_color(colour))
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
