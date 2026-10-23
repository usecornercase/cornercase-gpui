use std::ops::Range;
use std::path::Path;

use gpui::{
    AnyElement, ClickEvent, Context, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px,
    uniform_list,
};

use crate::theme::colours;
use crate::ui;

use super::style::{ellipsis, file_icon, home_relative, small_f32};
use super::{FilesPanel, Target, tree};

fn chevron(row: &tree::Row) -> &'static str {
    match (row.dir, row.open) {
        (false, _) => "",
        (true, true) => "▾",
        (true, false) => "▸",
    }
}

fn truncated_note() -> impl IntoElement {
    ui::dim(format!("Some folders hold more than {} entries; the rest are not listed", tree::MAX_ENTRIES)).px(px(12.))
}

impl FilesPanel {
    fn tree_header(root_label: String, any_open: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let collapse = ui::icon_button("files-collapse", "⊟")
            .text_size(px(12.))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.collapse_all(cx)));
        div()
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .px(px(12.))
            .pt(px(6.))
            .pb(px(4.))
            .child(ui::dim(root_label).min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis())
            .child(div().flex().flex_none().gap(px(4.)).items_center().when(any_open, |d| d.child(collapse)))
    }

    fn tree_list(&self, count: usize, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div().flex_1().min_h_0().px(px(6.)).pb(px(6.)).child(
            uniform_list(
                "files-tree",
                count,
                cx.processor(|this, range: Range<usize>, _window, cx| this.tree_items(range, cx)),
            )
            .size_full()
            .track_scroll(self.tree_scroll.clone()),
        )
    }

    pub(super) fn render_tree(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(place) = self.place() else { return ui::note("No workspace shown").into_any_element() };
        if let Some(error) = &place.error {
            return ui::note(format!("Cannot read {}: {error}", place.root.display())).into_any_element();
        }
        let rows = tree::rows(&place.listings, &place.expanded, self.show_hidden);
        let loading = !place.listings.contains_key(Path::new(""));
        let truncated = place.listings.values().any(|l| l.truncated);
        let any_open = !place.expanded.is_empty();
        let root_label = home_relative(&place.root);
        self.rows = rows;
        let count = self.rows.len();
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(Self::tree_header(root_label, any_open, cx))
            .when(loading, |d| d.child(ui::note("Reading…")))
            .when(!loading && count == 0, |d| d.child(ui::note("Empty folder")))
            .when(truncated, |d| d.child(truncated_note()))
            .child(self.tree_list(count, cx))
            .into_any_element()
    }

    fn tree_items(&mut self, range: Range<usize>, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let last = self.place().and_then(|p| p.last.clone());
        let menu_rel = self.menu.as_ref().map(|m| m.target.rel.clone());
        range
            .filter_map(|ix| self.rows.get(ix).cloned().map(|row| (ix, row)))
            .map(|(ix, row)| {
                let selected = last.as_ref() == Some(&row.path) || menu_rel.as_ref() == Some(&row.path);
                Self::tree_row(ix, &row, selected, cx).into_any_element()
            })
            .collect()
    }

    fn tree_row(ix: usize, row: &tree::Row, selected: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (rel, dir) = (row.path.clone(), row.dir);
        let target = Target { rel: row.path.clone(), dir, line: None };
        div()
            .id(("files-row", ix))
            .flex()
            .items_center()
            .h(px(24.))
            .pl(px(6. + small_f32(row.depth) * 14.))
            .pr(px(8.))
            .rounded(px(4.))
            .cursor_pointer()
            .when(selected, |d| d.bg(colours::selected()))
            .hover(|s| s.bg(colours::hover()))
            .child(div().w(px(14.)).flex_none().text_size(px(12.)).text_color(colours::muted()).child(chevron(row)))
            .child(file_icon(&row.name, row.dir, row.open))
            .child(
                ellipsis()
                    .ml(px(6.))
                    .text_color(if row.name.starts_with('.') { colours::muted() } else { colours::text() })
                    .child(row.name.clone()),
            )
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.click_row(rel.clone(), dir, cx)))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, e: &MouseDownEvent, _, cx| this.show_menu(e.position, target.clone(), cx)),
            )
    }
}
