use std::ops::Range;
use std::path::{Path, PathBuf};

use gpui::{
    Div, ElementId, FontStyle, HighlightStyle, Hsla, ImageFormat, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, SharedString, Stateful, Styled as _, div, prelude::FluentBuilder as _, px,
};
use grep_matcher::Matcher as _;
use grep_regex::RegexMatcher;

use crate::theme::{ANSI, FONT_FAMILY, colours, hsla, radius};
use crate::ui::{self, Icon, OneLine, file_icons};

use super::highlight::Kind;
use super::{CODE_SIZE, Spans};

pub(super) type Styles = Vec<(Range<usize>, HighlightStyle)>;

pub(super) struct CodeRow {
    pub ix: usize,
    pub shown: String,
    pub styles: Styles,
    pub selected: bool,
    pub width: Pixels,
}

impl CodeRow {
    pub(super) fn new(ix: usize, line: &str, styles: Styles, selected: bool, width: Pixels) -> Self {
        let shown = if line.is_empty() { " ".to_string() } else { line.to_string() };
        Self { ix, shown, styles, selected, width }
    }
}

pub(super) fn image_label(format: ImageFormat) -> String {
    format!("{format:?} image").to_uppercase().replace(" IMAGE", " image")
}

pub(super) fn number_width(lines: usize) -> Pixels {
    let digits = lines.to_string().len().max(3);
    px(small_f32(digits) * 7.3 + 16.)
}

pub(super) fn span_styles(spans: Option<&Spans>, ix: usize) -> Styles {
    spans
        .and_then(|s| s.get(ix))
        .map(|spans| spans.iter().map(|(r, k)| (r.clone(), kind_style(*k))).collect())
        .unwrap_or_default()
}

pub(super) fn code_row(row: CodeRow) -> Div {
    div()
        .flex()
        .w_full()
        .h(px(18.))
        .font_family(FONT_FAMILY)
        .text_size(px(CODE_SIZE))
        .items_center()
        .whitespace_nowrap()
        .when(row.selected, |d| d.bg(colours::selected()))
        .child(
            div()
                .w(row.width)
                .flex_none()
                .pr(px(10.))
                .text_right()
                .text_color(if row.selected { colours::accent() } else { colours::faint() })
                .child((row.ix + 1).to_string()),
        )
        .child(div().flex_1().min_w_0().pr(px(8.)).child(ui::one_line(row.shown).highlights(row.styles)))
}

pub(super) fn clip(ranges: &[Range<usize>], within: Range<usize>) -> Vec<Range<usize>> {
    ranges
        .iter()
        .filter_map(|r| {
            let (start, end) = (r.start.max(within.start), r.end.min(within.end));
            (start < end).then(|| start - within.start..end - within.start)
        })
        .collect()
}

pub(super) fn matches(matcher: &RegexMatcher, line: &str) -> Vec<Range<usize>> {
    let mut found = Vec::new();
    let _ = matcher.find_iter(line.as_bytes(), |m| {
        found.push(m.start()..m.end());
        true
    });
    found
}

fn covers(range: &Range<usize>, piece: &Range<usize>) -> bool {
    range.start <= piece.start && piece.end <= range.end
}

pub(super) fn overlay(styles: Styles, lit: &[Range<usize>], bg: Hsla) -> Styles {
    if lit.is_empty() {
        return styles;
    }
    let mut cuts: Vec<usize> =
        styles.iter().flat_map(|(r, _)| [r.start, r.end]).chain(lit.iter().flat_map(|r| [r.start, r.end])).collect();
    cuts.sort_unstable();
    cuts.dedup();
    let mut out: Styles = Vec::new();
    for pair in cuts.windows(2) {
        let piece = pair[0]..pair[1];
        let base = styles.iter().find(|(r, _)| covers(r, &piece)).map(|(_, s)| *s);
        let in_lit = lit.iter().any(|r| covers(r, &piece));
        let style = match (base, in_lit) {
            (None, false) => continue,
            (Some(s), false) => s,
            (base, true) => HighlightStyle { background_color: Some(bg), ..base.unwrap_or_default() },
        };
        out.push((piece, style));
    }
    out
}

pub(super) fn kind_style(kind: Kind) -> HighlightStyle {
    let colour = |i: usize| HighlightStyle { color: Some(hsla(ANSI[i])), ..Default::default() };
    match kind {
        Kind::Keyword => colour(5),
        Kind::Function => colour(4),
        Kind::Type => colour(6),
        Kind::String => colour(2),
        Kind::Number => colour(3),
        Kind::Tag => colour(1),
        Kind::Comment => HighlightStyle {
            color: Some(colours::faint().into()),
            font_style: Some(FontStyle::Italic),
            ..Default::default()
        },
    }
}

pub(super) fn with_alpha(mut colour: Hsla, alpha: f32) -> Hsla {
    colour.a = alpha;
    colour
}

pub(super) fn lit_background(alpha: f32) -> Hsla {
    with_alpha(hsla(ANSI[3]), alpha)
}

pub(super) fn plural<'a>(n: usize, one: &'a str, many: &'a str) -> &'a str {
    if n == 1 { one } else { many }
}

pub(super) fn small_f32(n: usize) -> f32 {
    f32::from(u16::try_from(n).unwrap_or(u16::MAX))
}

pub(super) fn home_relative(path: &Path) -> String {
    match std::env::var_os("HOME").map(PathBuf::from) {
        Some(home) if path.starts_with(&home) => {
            format!("~/{}", path.strip_prefix(&home).map(|p| p.display().to_string()).unwrap_or_default())
        }
        _ => path.display().to_string(),
    }
}

pub(super) fn file_icon(name: &str, dir: bool, open: bool) -> impl IntoElement + use<> {
    file_icons::file_icon(name, dir, open, 16.)
}

pub(super) fn chevron(dir: bool, open: bool) -> impl IntoElement + use<> {
    let slot = div().w(px(16.)).flex_none().flex().items_center().justify_center();
    match (dir, open) {
        (false, _) => slot,
        (true, true) => slot.child(ui::icon(Icon::ChevronDown, 12.).text_color(colours::faint())),
        (true, false) => slot.child(ui::icon(Icon::ChevronRight, 12.).text_color(colours::faint())),
    }
}

pub(super) fn ellipsis(line: OneLine) -> Div {
    div().min_w_0().flex().child(line)
}

pub(super) fn faint_small(line: OneLine) -> Div {
    ellipsis(line).text_size(px(11.)).text_color(colours::faint())
}

pub(super) fn result_row(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .w_full()
        .flex()
        .items_center()
        .gap(px(8.))
        .h(px(26.))
        .px(px(8.))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .hover(|s| s.bg(colours::hover()))
}

pub(super) fn menu_entry(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id)
        .px(px(10.))
        .py(px(5.))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .whitespace_nowrap()
        .hover(|s| s.bg(ui::glass::cobalt()).text_color(colours::on_brand()))
        .child(label.into())
}

pub(super) fn centered_note(title: impl Into<SharedString>, detail: impl Into<SharedString>) -> Div {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(4.))
        .child(div().text_color(colours::muted()).child(title.into()))
        .child(ui::dim(detail.into()))
}

#[cfg(test)]
mod tests;
