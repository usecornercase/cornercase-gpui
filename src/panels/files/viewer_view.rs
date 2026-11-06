use std::ops::Range;
use std::path::PathBuf;

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, InteractiveElement as _, IntoElement, ObjectFit, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, StyledImage as _, Window, div, img, prelude::FluentBuilder as _, px,
    relative, uniform_list,
};

use crate::keymap::{Command, with_keys};
use crate::panels::PanelKind;
use crate::platform::paths::Style;
use crate::theme::{colours, radius};
use crate::ui;
use crate::ui::selection::{self, Lit, Styles, overlay};

use super::disk::{self, Content};
use super::find;
use super::style::{
    CodeRow, centered_note, code_row, faint_small, file_icon, image_label, number_width, plural, span_styles,
};
use super::{FilesPanel, Target, Viewer};

struct Heading {
    name: String,
    folder: String,
    back: &'static str,
    info: String,
    path: PathBuf,
    target: Target,
    text: bool,
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

fn heading(viewer: &Viewer, style: Style, flow_shows: bool) -> Heading {
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
        Content::Gone(e) => centered_note("Cannot read this file", e.clone()).into_any_element(),
        Content::Text(_) => div().into_any_element(),
    }
}

impl FilesPanel {
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
            .child(faint_small(ui::one_line(heading.folder.clone())))
    }

    fn viewer_actions(heading: &Heading, cx: &mut Context<Self>) -> impl IntoElement + use<> {
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
                        .tooltip(crate::notify::tip(with_keys("Find in this file", Command::FindInFile)))
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_find(window, cx))),
                )
            })
            .child(ui::button("files-open-in", "Open in…").on_click(
                cx.listener(move |this, e: &ClickEvent, _, cx| this.show_menu(e.position(), target.clone(), cx)),
            ))
            .child(
                ui::button("files-copy-path", "Copy path")
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.copy(path.clone(), "path", cx))),
            )
    }

    fn viewer_header(heading: &Heading, cx: &mut Context<Self>) -> impl IntoElement + use<> {
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
            .child(Self::viewer_actions(heading, cx))
    }

    fn viewer_body(&self, content: Option<&Content>, room: f32, cx: &mut Context<Self>) -> AnyElement {
        match content {
            None => ui::loading("Reading…").into_any_element(),
            Some(Content::Text(text)) => uniform_list(
                "files-code",
                text.lines.len(),
                cx.processor(|this, range: Range<usize>, _window, _cx| this.code_items(range)),
            )
            .size_full()
            .pt(px(4. + room / 2.))
            .pb(px(4.))
            .track_scroll(self.code_scroll.clone())
            .into_any_element(),
            Some(other) => other_body(other),
        }
    }

    pub(super) fn render_viewer(&mut self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(viewer) = self.place().and_then(|p| p.viewer.as_ref()) else { return div().into_any_element() };
        viewer.selecting.laid().clear();
        let flow_shows = PanelKind::Flow.shows(self.store.read(cx).session.as_ref());
        let heading = heading(viewer, self.style(), flow_shows);
        let room = if viewer.find.is_some() { find::FIELD_ROOM } else { 0. };
        let bar = self.line_bar(cx);
        let code = selection::body(cx)
            .rounded_b(px(radius::CARD - 1.))
            .child(self.viewer_body(viewer.content.as_ref(), room, cx))
            .children(self.find_bar(window, cx))
            .child(selection::layer(cx.weak_entity(), &viewer.selecting, bar, room));
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(Self::viewer_header(&heading, cx))
            .child(code)
            .into_any_element()
    }

    fn line_styles(viewer: &Viewer, ix: usize, line: &str, lit: &Lit) -> Styles {
        let styles = span_styles(viewer.spans.as_ref(), ix);
        let styles = match &viewer.find {
            Some(find) => find.paint_line(styles, ix, line),
            None => styles,
        };
        match lit.drawn_range(line) {
            Some(range) => overlay(styles, &[range], selection::selected_chars()),
            None => styles,
        }
    }

    fn code_items(&mut self, range: Range<usize>) -> Vec<AnyElement> {
        let Some(viewer) = self.place().and_then(|p| p.viewer.as_ref()) else { return Vec::new() };
        let Some(Content::Text(text)) = &viewer.content else { return Vec::new() };
        let width = number_width(text.lines.len());
        range
            .filter_map(|ix| text.lines.get(ix).map(|line| (ix, line)))
            .map(|(ix, line)| Self::code_line(viewer, (ix, line), width))
            .collect()
    }

    fn code_line(viewer: &Viewer, (ix, line): (usize, &str), width: gpui::Pixels) -> AnyElement {
        let lit = viewer.selecting.lit(ix, line);
        let selected = lit == Lit::Line;
        let (laid, len) = (viewer.selecting.laid().clone(), line.len());
        let row = CodeRow::new(ix, line, Self::line_styles(viewer, ix, line, &lit), selected, width)
            .on_laid(move |out| laid.add_line(ix, len, out));
        code_row(row).when(!selected, |d| d.hover(|s| s.bg(colours::panel()))).into_any_element()
    }
}

#[cfg(test)]
mod tests;
