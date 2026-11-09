use std::path::PathBuf;

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, InteractiveElement as _, IntoElement, ListState, ObjectFit,
    ParentElement as _, Pixels, StatefulInteractiveElement as _, Styled as _, StyledImage as _, Window, div, img,
    list as list_of, prelude::FluentBuilder as _, px, relative,
};

use crate::keymap::{Command, with_keys};
use crate::notify::tip;
use crate::panels::PanelKind;
use crate::platform::paths::Style;
use crate::prefs::LongLines;
use crate::store::Store;
use crate::theme::{colours, radius};
use crate::ui::selection::{self, Fit, Lit, Styles, overlay};
use crate::ui::{self, Icon};

use super::disk::{self, Content};
use super::gutter_view::{mark_cell, old_lines};
use super::style::{
    centered_note, code_frame, faint_small, file_icon, image_label, number_cell, number_width, plural, span_styles,
};
use super::{FilesPanel, Target, Viewer};

const CODE_TOP: f32 = 4.;
const CODE_BOTTOM: f32 = 4.;

struct Heading {
    name: String,
    folder: String,
    back: &'static str,
    info: String,
    path: PathBuf,
    target: Target,
    text: bool,
    wraps: bool,
    full: bool,
}

fn content_info(viewer: &Viewer) -> Vec<String> {
    let size = viewer.stamp.map(|s| disk::human_size(s.len));
    let mut info: Vec<String> = Vec::new();
    match &viewer.content {
        None => info.push("Reading…".into()),
        Some(Content::Text(text)) => {
            let n = text.lines.len();
            info.push(format!("{n} {}", plural(n, "line", "lines")));
            info.extend(size);
            info.extend(viewer.language.map(str::to_string));
        }
        Some(Content::Image(image)) => {
            info.push(image_label(image.format));
            info.extend(size);
        }
        Some(_) => info.extend(size),
    }
    info
}

fn back_label(from_flow: bool, from_search: bool) -> &'static str {
    match (from_flow, from_search) {
        (true, _) => "‹ Flow",
        (false, true) => "‹ Results",
        (false, false) => "‹ Files",
    }
}

fn heading(viewer: &Viewer, style: Style, flow_shows: bool, full: bool) -> Heading {
    let rel = &viewer.rel;
    let shown = style.shown(rel);
    let (folder, name) = style.split(&shown);
    Heading {
        name: if name.is_empty() { shown.clone() } else { name.to_string() },
        folder: folder.to_string(),
        back: back_label(viewer.from_flow && flow_shows, viewer.from_search),
        info: content_info(viewer).join("  ·  "),
        path: viewer.path.clone(),
        text: matches!(viewer.content, Some(Content::Text(_))),
        wraps: viewer.long_lines == LongLines::Wrap,
        full,
        target: Target { rel: rel.clone(), dir: false, line: viewer.selecting.selection.map(|s| s.covered().0) },
    }
}

fn other_body(content: &Content) -> AnyElement {
    match content {
        Content::Image(image) => div()
            .id("files-image")
            .size_full()
            .p(px(16.))
            .flex()
            .items_center()
            .justify_center()
            .child(img(image.clone()).max_w_full().max_h_full().object_fit(ObjectFit::ScaleDown).id("files-picture"))
            .into_any_element(),
        Content::Binary => centered_note("Binary file", "Open it in another app to see it.").into_any_element(),
        Content::TooLarge => centered_note(
            "Too large to show",
            format!("Files over {} are not read here.", disk::human_size(disk::MAX_BYTES)),
        )
        .into_any_element(),
        Content::NotRegular => {
            centered_note("Not a regular file", "Devices, sockets and pipes are not read.").into_any_element()
        }
        Content::Gone(e) | Content::Lost(e) => centered_note("Cannot read this file", e.clone()).into_any_element(),
        Content::Text(_) => div().into_any_element(),
    }
}

impl FilesPanel {
    fn viewer_tools(heading: &Heading, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let glyph = if heading.full { Icon::Shrink } else { Icon::Enlarge };
        let full = ui::tool_button("files-full-screen", glyph, 24., heading.full)
            .tooltip(tip("Full screen: this button or Esc goes back"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.store.update(cx, Store::toggle_file_full_screen)));
        let wrap = selection::wrap_button("files-wrap", heading.wraps).on_click(|_: &ClickEvent, _, cx| {
            Store::get(cx).update(cx, Store::toggle_long_lines);
        });
        div().flex_none().flex().items_center().gap(px(2.)).when(heading.text, |d| d.child(wrap)).child(full)
    }

    fn viewer_title(heading: &Heading, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                ui::button("files-back", heading.back)
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_file(cx))),
            )
            .child(file_icon(&heading.name, false, false))
            .child(
                div()
                    .flex_none()
                    .flex()
                    .max_w(relative(0.6))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(ui::one_line(heading.name.clone())),
            )
            .child(faint_small(ui::one_line(heading.folder.clone())).flex_1())
            .child(Self::viewer_tools(heading, cx))
    }

    fn viewer_actions(heading: &Heading, opens: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let target = heading.target.clone();
        let path = heading.path.display().to_string();
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(ui::dim(ui::one_line(heading.info.clone())).flex_1())
            .when(heading.text, |d| {
                d.child(
                    ui::button("files-find", "Find")
                        .tooltip(tip(with_keys("Find in this file", Command::FindInFile)))
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_find(window, cx))),
                )
            })
            .when(opens, |d| {
                d.child(ui::button("files-open-in", "Open in…").on_click(
                    cx.listener(move |this, e: &ClickEvent, _, cx| this.show_menu(e.position(), target.clone(), cx)),
                ))
            })
            .child(
                ui::button("files-copy-path", "Copy path")
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.copy(path.clone(), "path", cx))),
            )
    }

    fn viewer_header(heading: &Heading, opens: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(8.))
            .px(px(12.))
            .py(px(10.))
            .border_b_1()
            .border_color(colours::border())
            .child(Self::viewer_title(heading, cx))
            .child(Self::viewer_actions(heading, opens, cx))
    }

    fn code_list(list: &ListState, cx: &mut Context<Self>) -> AnyElement {
        list_of(list.clone(), cx.processor(|this, ix: usize, _window, cx| this.code_item(ix, cx)))
            .size_full()
            .pb(px(CODE_BOTTOM))
            .into_any_element()
    }

    fn viewer_body(viewer: &Viewer, cx: &mut Context<Self>) -> AnyElement {
        match &viewer.content {
            None => ui::loading("Reading…").into_any_element(),
            Some(Content::Text(_)) => Self::code_list(&viewer.list, cx),
            Some(other) => other_body(other),
        }
    }

    fn follow_wrap(&mut self, cx: &Context<Self>) {
        let long_lines = self.store.read(cx).prefs.long_lines;
        let viewer = self.place_mut().and_then(|p| p.viewer.as_mut());
        if let Some(viewer) = viewer.filter(|viewer| viewer.long_lines != long_lines) {
            viewer.long_lines = long_lines;
            viewer.refit();
        }
    }

    pub(super) fn render_viewer(&mut self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        self.follow_wrap(cx);
        self.settle_reveal(window);
        let Some(viewer) = self.place().and_then(|p| p.viewer.as_ref()) else { return div().into_any_element() };
        viewer.selecting.laid().clear();
        let store = self.store.read(cx);
        let flow_shows = PanelKind::Flow.shows(store.session.as_ref());
        let heading = heading(viewer, self.style(), flow_shows, store.shows_file_full_screen());
        let room = viewer.room();
        let bar = self.line_bar(cx);
        let code = selection::body(cx)
            .rounded_b(px(radius::card() - 1.))
            .child(Self::viewer_body(viewer, cx))
            .children(self.find_bar(window, cx))
            .child(selection::layer(cx.weak_entity(), &viewer.selecting, bar, room));
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(Self::viewer_header(&heading, self.machine.is_none(), cx))
            .child(code)
            .into_any_element()
    }

    fn line_styles(viewer: &Viewer, (ix, line): (usize, &str), fit: Fit, lit: &Lit) -> Styles {
        let styles = span_styles(viewer.spans.as_ref(), ix);
        let styles = match &viewer.find {
            Some(find) => find.paint_line(styles, (ix, line), fit),
            None => styles,
        };
        match lit.drawn_range(line) {
            Some(range) => overlay(styles, &[range], selection::selected_chars()),
            None => styles,
        }
    }

    fn code_item(&mut self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(viewer) = self.place().and_then(|p| p.viewer.as_ref()) else { return div().into_any_element() };
        let Some(Content::Text(text)) = &viewer.content else { return div().into_any_element() };
        let Some(line) = text.lines.get(ix) else { return div().into_any_element() };
        let (fit, width) = (viewer.fit(), number_width(text.lines.len()));
        let (above, below) = (ix, Some(ix + 1).filter(|next| *next == text.lines.len()));
        div()
            .w_full()
            .flex()
            .flex_col()
            .when(ix == 0, |d| d.pt(px(CODE_TOP + viewer.room())))
            .children(viewer.margin.unfolded_before(above).map(|block| old_lines(block, fit, width)))
            .child(Self::code_line(viewer, (ix, line), (fit, width), cx))
            .children(below.and_then(|next| viewer.margin.unfolded_before(next)).map(|b| old_lines(b, fit, width)))
            .into_any_element()
    }

    fn code_line(
        viewer: &Viewer,
        (ix, line): (usize, &str),
        (fit, width): (Fit, Pixels),
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let lit = viewer.selecting.lit(ix, line);
        let selected = lit == Lit::Line;
        let styles = Self::line_styles(viewer, (ix, line), fit, &lit);
        let number = if selected { colours::accent() } else { colours::faint() };
        let mark = viewer.margin.gutter.as_ref().and_then(|gutter| gutter.at(ix)).map(|block| mark_cell(ix, block, cx));
        fit.row(code_frame(selected))
            .when(!selected, |d| d.hover(|s| s.bg(colours::panel())))
            .child(number_cell(width).relative().text_color(number).child((ix + 1).to_string()).children(mark))
            .child(selection::code_cell(viewer.selecting.laid(), (ix, line), styles, fit))
    }
}

#[cfg(test)]
mod tests;
