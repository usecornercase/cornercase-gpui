use std::ops::Range;
use std::path::Path;

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px,
    uniform_list,
};

use crate::notify::tip;
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, glass};

use super::style::{chevron, ellipsis, file_icon, home_relative, small_f32};
use super::{FilesPanel, Target, tree};

const INDENT: f32 = 18.;

fn truncated_note() -> impl IntoElement {
    ui::dim(format!("Some folders hold more than {} entries; the rest are not listed", tree::MAX_ENTRIES)).px(px(12.))
}

fn root_name(root: &Path) -> impl IntoElement + use<> {
    let name = root.file_name().map_or_else(|| home_relative(root), |n| n.to_string_lossy().into_owned());
    div()
        .id("files-root")
        .min_w_0()
        .flex()
        .text_size(px(12.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(colours::muted())
        .tooltip(tip(home_relative(root)))
        .child(ui::one_line(name))
}

impl FilesPanel {
    fn tree_tools(&self, any_open: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (glyph, hint) =
            if self.show_hidden { (Icon::Eye, "Hide hidden files") } else { (Icon::EyeOff, "Show hidden files") };
        let hidden = ui::tool_button("files-hidden", glyph, 24., false).tooltip(tip(hint)).on_click(cx.listener(
            |this, _: &ClickEvent, _, cx| {
                this.show_hidden = !this.show_hidden;
                cx.notify();
            },
        ));
        let collapse = ui::tool_button("files-collapse", Icon::CollapseAll, 24., false)
            .tooltip(tip("Collapse all folders"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.collapse_all(cx)));
        div().flex().flex_none().gap(px(2.)).items_center().child(hidden).when(any_open, |d| d.child(collapse))
    }

    fn tree_header(&self, root: &Path, any_open: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .pl(px(12.))
            .pr(px(8.))
            .pt(px(6.))
            .pb(px(2.))
            .child(root_name(root))
            .child(self.tree_tools(any_open, cx))
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
        let root = place.root.clone();
        self.rows = rows;
        let count = self.rows.len();
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(self.tree_header(&root, any_open, cx))
            .when(loading, |d| d.child(ui::loading("Reading…")))
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
            .w_full()
            .flex()
            .items_center()
            .gap(px(4.))
            .h(px(26.))
            .pl(px(6. + small_f32(row.depth) * INDENT))
            .pr(px(8.))
            .rounded(px(radius::CONTROL))
            .cursor_pointer()
            .when(selected, glass::lit)
            .when(!selected, |d| d.hover(|s| s.bg(colours::hover())))
            .child(chevron(row.dir, row.open))
            .child(file_icon(&row.name, row.dir, row.open))
            .child(ellipsis(ui::one_line(row.name.clone())).ml(px(4.)).text_color(if row.name.starts_with('.') {
                colours::muted()
            } else {
                colours::text()
            }))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.click_row(rel.clone(), dir, cx)))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, e: &MouseDownEvent, _, cx| this.show_menu(e.position, target.clone(), cx)),
            )
    }
}
