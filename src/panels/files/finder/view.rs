use std::ops::Range;

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, Focusable as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div, px, uniform_list,
};

use crate::theme::colours;
use crate::ui;

use super::super::results_view::{name_hit, names_summary};
use super::super::style::centered_note;
use super::{Finder, ROW, Status};

fn hint(key: &'static str, what: &'static str) -> Div {
    div().flex().items_center().gap(px(5.)).child(div().text_color(colours::muted()).child(key)).child(ui::dim(what))
}

fn hints() -> Div {
    div().flex_none().flex().gap(px(14.)).child(hint("↑↓", "move")).child(hint("↵", "open")).child(hint("esc", "close"))
}

fn note(status: &Status) -> AnyElement {
    let (title, detail) = match status {
        Status::Nowhere => ("No workspace shown".to_string(), "Open a project to find its files".to_string()),
        Status::Hint => {
            ("Type to find a file by its name or path".into(), "A few of its letters, in order, are enough".into())
        }
        Status::Failed(e) => ("Cannot search".into(), e.clone()),
        Status::Found(found) => (found.clone(), "Try fewer letters, or another part of the path".into()),
        Status::Reading => return ui::loading("Reading the files…").into_any_element(),
        Status::Searching => return ui::loading("Searching…").into_any_element(),
    };
    centered_note(title, detail).into_any_element()
}

impl Finder {
    pub(super) fn top(&self, window: &Window, cx: &App) -> impl IntoElement + use<> {
        let focused = self.field.read(cx).focus_handle(cx).is_focused(window);
        div()
            .flex_none()
            .px(px(14.))
            .pt(px(14.))
            .pb(px(12.))
            .border_b_1()
            .border_color(colours::border())
            .child(ui::search_field(self.field.clone(), focused, None))
    }

    pub(super) fn body(&self, status: &Status, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let body = div().flex_1().min_h_0();
        if self.pick.hits.is_empty() {
            return body.child(note(status));
        }
        let rows = cx.processor(|this, range: Range<usize>, _window, cx| this.rows(range, cx));
        body.px(px(8.)).py(px(6.)).opacity(if busy(status) { 0.55 } else { 1. }).child(
            uniform_list("finder-hits", self.pick.hits.len(), rows).size_full().track_scroll(self.scroll.clone()),
        )
    }

    fn rows(&mut self, range: Range<usize>, cx: &mut Context<Self>) -> Vec<AnyElement> {
        range.filter_map(|ix| self.row(ix, cx)).collect()
    }

    fn row(&self, ix: usize, cx: &mut Context<Self>) -> Option<AnyElement> {
        let hit = self.pick.hits.get(ix)?;
        let row = ui::row(("finder-hit", ix), ix == self.pick.selected).w_full().h(px(ROW));
        let open = cx.listener(move |this, _: &ClickEvent, _, cx| this.open(ix, cx));
        Some(name_hit(row, hit).on_click(open).into_any_element())
    }

    pub(super) fn footer(&self, status: &Status) -> impl IntoElement + use<> {
        div()
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .px(px(16.))
            .py(px(9.))
            .border_t_1()
            .border_color(colours::border())
            .text_size(px(11.))
            .child(div().flex_1().min_w_0().child(self.summary(status)))
            .child(hints())
    }

    fn summary(&self, status: &Status) -> AnyElement {
        let label = self.spot.as_ref().map(|s| s.label.clone()).unwrap_or_default();
        if self.pick.hits.is_empty() {
            return ui::dim(ui::one_line(label)).into_any_element();
        }
        match status {
            Status::Reading => ui::busy("finder-busy", "Reading the files…").into_any_element(),
            Status::Searching => ui::busy("finder-busy", "Searching…").into_any_element(),
            _ => ui::dim(ui::one_line(format!("{} in {label}", names_summary(&self.pick.hits)))).into_any_element(),
        }
    }
}

fn busy(status: &Status) -> bool {
    matches!(status, Status::Reading | Status::Searching)
}
