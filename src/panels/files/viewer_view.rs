use std::ops::Range;
use std::path::PathBuf;

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, InteractiveElement as _, IntoElement, ObjectFit, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, StyledImage as _, div, img, px, relative, uniform_list,
};

use crate::theme::{colours, radius};
use crate::ui;

use super::disk::{self, Content};
use super::style::{
    CodeRow, Styles, centered_note, code_row, faint_small, file_icon, image_label, lit_background, matches,
    number_width, overlay, plural, span_styles,
};
use super::{FilesPanel, Target, Viewer};

struct Heading {
    name: String,
    folder: String,
    back: &'static str,
    info: String,
    path: PathBuf,
    target: Target,
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
            info.extend(viewer.selected.map(|line| format!("line {}", line + 1)));
        }
        Some(Content::Image(image)) => {
            info.push(image_label(image.format));
            info.extend(size);
        }
        Some(_) => info.extend(size),
    }
    info
}

fn heading(viewer: &Viewer) -> Heading {
    let rel = &viewer.rel;
    Heading {
        name: rel.file_name().map_or_else(|| rel.display().to_string(), |n| n.to_string_lossy().into_owned()),
        folder: rel.parent().map(|p| p.display().to_string()).unwrap_or_default(),
        back: if viewer.from_search { "‹ Results" } else { "‹ Files" },
        info: content_info(viewer).join("  ·  "),
        path: viewer.path.clone(),
        target: Target { rel: rel.clone(), dir: false, line: viewer.selected },
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

    fn viewer_body(&self, content: Option<&Content>, cx: &mut Context<Self>) -> AnyElement {
        match content {
            None => ui::loading("Reading…").into_any_element(),
            Some(Content::Text(text)) => uniform_list(
                "files-code",
                text.lines.len(),
                cx.processor(|this, range: Range<usize>, _window, cx| this.code_items(range, cx)),
            )
            .size_full()
            .py(px(4.))
            .track_scroll(self.code_scroll.clone())
            .into_any_element(),
            Some(other) => other_body(other),
        }
    }

    pub(super) fn render_viewer(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(viewer) = self.place().and_then(|p| p.viewer.as_ref()) else { return div().into_any_element() };
        let heading = heading(viewer);
        let body = self.viewer_body(viewer.content.as_ref(), cx);
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(Self::viewer_header(&heading, cx))
            .child(div().flex_1().min_h_0().rounded_b(px(radius::CARD - 1.)).bg(colours::terminal()).child(body))
            .into_any_element()
    }

    fn line_styles(viewer: &Viewer, ix: usize, line: &str) -> Styles {
        let styles = span_styles(viewer.spans.as_ref(), ix);
        match &viewer.lit {
            Some(matcher) => overlay(styles, &matches(matcher, line), lit_background(0.3)),
            None => styles,
        }
    }

    fn code_items(&mut self, range: Range<usize>, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(viewer) = self.place().and_then(|p| p.viewer.as_ref()) else { return Vec::new() };
        let Some(Content::Text(text)) = &viewer.content else { return Vec::new() };
        let width = number_width(text.lines.len());
        range
            .filter_map(|ix| text.lines.get(ix).map(|line| (ix, line)))
            .map(|(ix, line)| {
                let row =
                    CodeRow::new(ix, line, Self::line_styles(viewer, ix, line), viewer.selected == Some(ix), width);
                Self::code_line(row, cx)
            })
            .collect()
    }

    fn code_line(row: CodeRow, cx: &mut Context<Self>) -> AnyElement {
        let ix = row.ix;
        code_row(row)
            .id(("files-line", ix))
            .hover(|s| s.bg(colours::panel()))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.select_line(ix, cx)))
            .into_any_element()
    }
}
