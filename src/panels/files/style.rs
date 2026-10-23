use std::ops::Range;
use std::path::{Path, PathBuf};

use gpui::{
    Div, ElementId, FontStyle, HighlightStyle, Hsla, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, Stateful, Styled as _, div, px,
};
use grep_matcher::Matcher as _;
use grep_regex::RegexMatcher;

use crate::theme::{ANSI, colours, hsla};
use crate::ui;

use super::highlight::Kind;

pub(super) type Styles = Vec<(Range<usize>, HighlightStyle)>;

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

fn extension_colour(name: &str) -> Hsla {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "rs" => hsla([222, 165, 132]),
        "ts" | "tsx" | "mts" | "cts" => hsla(ANSI[4]),
        "js" | "jsx" | "mjs" | "cjs" | "json" | "toml" | "yaml" | "yml" | "lock" => hsla(ANSI[3]),
        "py" | "go" => hsla(ANSI[6]),
        "md" | "markdown" | "txt" => hsla(ANSI[7]),
        "html" | "css" | "scss" | "svelte" | "vue" => hsla(ANSI[1]),
        "sh" | "zsh" | "bash" | "fish" => hsla(ANSI[2]),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico" => hsla(ANSI[5]),
        _ => colours::faint().into(),
    }
}

pub(super) fn file_icon(name: &str, dir: bool, open: bool) -> impl IntoElement + use<> {
    let (glyph, colour) = if dir {
        (if open { "▫" } else { "▪" }, Hsla::from(colours::accent()))
    } else {
        ("•", extension_colour(name))
    };
    div().w(px(12.)).flex_none().flex().justify_center().text_size(px(12.)).text_color(colour).child(glyph)
}

pub(super) fn ellipsis() -> Div {
    div().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis()
}

pub(super) fn faint_small() -> Div {
    ellipsis().text_size(px(11.)).text_color(colours::faint())
}

pub(super) fn result_row(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(8.))
        .h(px(24.))
        .px(px(8.))
        .rounded(px(4.))
        .cursor_pointer()
        .hover(|s| s.bg(colours::hover()))
}

pub(super) fn menu_entry(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id)
        .px(px(10.))
        .py(px(4.))
        .rounded(px(4.))
        .cursor_pointer()
        .whitespace_nowrap()
        .hover(|s| s.bg(colours::hover()))
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
        .child(ui::dim(detail))
}

#[cfg(test)]
mod tests;
