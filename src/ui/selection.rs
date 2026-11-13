mod caret;
mod code;
mod pointer;

use std::fmt::Display;
use std::ops::Range;

use gpui::{Bounds, HighlightStyle, Pixels, Point, Size, point, px};
use unicode_segmentation::UnicodeSegmentation;

use crate::theme::colours;

pub(crate) use caret::{ShownLines, Step};
pub(crate) use code::{CODE_CELL, CODE_ROW, Fit, code_cell, plain_text, wrap_button};
pub(crate) use pointer::{Laid, Off, Selecting, Selects, action, action_bar, body, keep_selection, layer, to_primary};

pub(crate) const BREAK_WIDTH: Pixels = px(6.);
const BAR_GAP: f32 = 4.;
const BAR_MARGIN: f32 = 8.;
const TAB_WIDTH: usize = 4;

pub(crate) type Styles = Vec<(Range<usize>, HighlightStyle)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Caret<S> {
    pub(crate) spot: S,
    pub(crate) at: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Selection<S> {
    Lines { anchor: S, head: S },
    Chars { anchor: Caret<S>, head: Caret<S> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Lit {
    Nothing,
    Line,
    Chars { range: Range<usize>, line_break: bool },
}

impl Lit {
    pub(crate) fn covers(&self) -> bool {
        match self {
            Self::Nothing => false,
            Self::Line => true,
            Self::Chars { range, line_break } => *line_break || !range.is_empty(),
        }
    }

    fn drawn_range(&self) -> Option<Range<usize>> {
        match self {
            Self::Chars { range, .. } if !range.is_empty() => Some(range.clone()),
            _ => None,
        }
    }

    fn draws_break(&self) -> bool {
        matches!(self, Self::Chars { line_break: true, .. })
    }

    fn painted(&self, styles: Styles) -> Styles {
        match self.drawn_range() {
            Some(range) => overlay(styles, &[range], selected_chars()),
            None => styles,
        }
    }
}

fn ordered<S: Copy + Ord>(a: Caret<S>, b: Caret<S>) -> (Caret<S>, Caret<S>) {
    if (a.spot, a.at) <= (b.spot, b.at) { (a, b) } else { (b, a) }
}

fn chars_lit<S: Copy + Ord>(anchor: Caret<S>, head: Caret<S>, spot: S, line: &str) -> Lit {
    let (from, to) = ordered(anchor, head);
    if spot < from.spot || spot > to.spot {
        return Lit::Nothing;
    }
    let start = if spot == from.spot { line.floor_char_boundary(from.at) } else { 0 };
    let end = if spot == to.spot { line.floor_char_boundary(to.at) } else { line.len() };
    Lit::Chars { range: start..end, line_break: spot < to.spot }
}

impl<S: Copy + Ord> Selection<S> {
    pub(crate) fn line(spot: S) -> Self {
        Self::Lines { anchor: spot, head: spot }
    }

    fn anchor(self) -> S {
        match self {
            Self::Lines { anchor, .. } => anchor,
            Self::Chars { anchor, .. } => anchor.spot,
        }
    }

    pub(crate) fn is_empty(self) -> bool {
        matches!(self, Self::Chars { anchor, head } if anchor == head)
    }

    pub(crate) fn head_caret(self) -> Caret<S> {
        match self {
            Self::Lines { head, .. } => Caret { spot: head, at: 0 },
            Self::Chars { head, .. } => head,
        }
    }

    pub(crate) fn head(self) -> S {
        match self {
            Self::Lines { head, .. } => head,
            Self::Chars { head, .. } => head.spot,
        }
    }

    pub(crate) fn ends(self) -> (S, S) {
        let (anchor, head) = (self.anchor(), self.head());
        if anchor <= head { (anchor, head) } else { (head, anchor) }
    }

    pub(crate) fn lit(self, spot: S, line: &str) -> Lit {
        match self {
            Self::Lines { .. } => {
                let (first, last) = self.ends();
                if first <= spot && spot <= last { Lit::Line } else { Lit::Nothing }
            }
            Self::Chars { anchor, head } => chars_lit(anchor, head, spot, line),
        }
    }

    fn parts<'a>(self, lines: impl Iterator<Item = (S, &'a str)>) -> Vec<(S, Range<usize>)> {
        lines
            .filter_map(|(spot, line)| match self.lit(spot, line) {
                Lit::Line => Some((spot, 0..line.len())),
                Lit::Chars { range, line_break } if line_break || !range.is_empty() => Some((spot, range)),
                _ => None,
            })
            .collect()
    }

    pub(crate) fn text<'a, 'b>(
        self,
        lines: impl Iterator<Item = (S, &'a str)>,
        own: impl Fn(S, Range<usize>) -> &'b str,
    ) -> String {
        let parts = self.parts(lines);
        let ends_on_a_break =
            matches!(self, Self::Chars { .. }) && parts.last().is_some_and(|(spot, _)| *spot < self.ends().1);
        let text = parts.into_iter().map(|(spot, range)| own(spot, range)).collect::<Vec<_>>().join("\n");
        if ends_on_a_break { text + "\n" } else { text }
    }

    pub(crate) fn one_line_chars(self) -> Option<(S, Range<usize>)> {
        match self {
            Self::Chars { anchor, head } if anchor.spot == head.spot => {
                let (from, to) = ordered(anchor, head);
                Some((from.spot, from.at..to.at))
            }
            _ => None,
        }
    }
}

impl Selection<usize> {
    pub(crate) fn covered(self) -> (usize, usize) {
        let (first, last) = self.ends();
        match self {
            Self::Chars { anchor, head } if last > first && ordered(anchor, head).1.at == 0 => (first, last - 1),
            _ => (first, last),
        }
    }

    pub(crate) fn fit(self, lines: usize) -> Option<Self> {
        let last = lines.checked_sub(1)?;
        if self.ends().0 > last {
            return None;
        }
        Some(match self {
            Self::Lines { anchor, head } => Self::Lines { anchor: anchor.min(last), head: head.min(last) },
            Self::Chars { anchor, head } => {
                let fitted = |caret: Caret<usize>| Caret { spot: caret.spot.min(last), at: caret.at };
                Self::Chars { anchor: fitted(anchor), head: fitted(head) }
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Zone {
    Numbers,
    Code,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Press<S> {
    from: Caret<S>,
    whole_lines: Option<S>,
    before: Option<Selection<S>>,
    shift: bool,
    moved: bool,
}

impl<S: Copy + Ord> Press<S> {
    pub(crate) fn down(before: Option<Selection<S>>, from: Caret<S>, zone: Zone, shift: bool) -> Self {
        let whole_lines = match (shift, zone) {
            (true, _) => Some(before.map_or(from.spot, Selection::anchor)),
            (false, Zone::Numbers) => Some(from.spot),
            (false, Zone::Code) => None,
        };
        Self { from, whole_lines, before, shift, moved: false }
    }

    pub(crate) fn on_word(before: Option<Selection<S>>, spot: S, word: Range<usize>) -> (Self, Selection<S>) {
        let anchor = Caret { spot, at: word.start };
        let press = Self { from: anchor, whole_lines: None, before, shift: false, moved: true };
        (press, Selection::Chars { anchor, head: Caret { spot, at: word.end } })
    }

    pub(crate) fn at_once(&self) -> Option<Selection<S>> {
        match self.whole_lines {
            Some(anchor) => Some(Selection::Lines { anchor, head: self.from.spot }),
            None => self.before,
        }
    }

    pub(crate) fn moved(&self) -> bool {
        self.moved
    }

    pub(crate) fn dragged(&mut self, to: Caret<S>) -> Selection<S> {
        self.moved = true;
        match self.whole_lines {
            Some(anchor) => Selection::Lines { anchor, head: to.spot },
            None if to == self.from => Selection::line(to.spot),
            None => Selection::Chars { anchor: self.from, head: to },
        }
    }

    pub(crate) fn released(&self, shown: Option<Selection<S>>) -> Option<Selection<S>> {
        if self.moved || self.shift {
            return shown;
        }
        let clicked = Selection::line(self.from.spot);
        if self.before == Some(clicked) { None } else { Some(clicked) }
    }
}

fn segments(line: &str) -> impl Iterator<Item = (Range<usize>, bool)> {
    line.split_word_bound_indices()
        .map(|(start, word)| (start..start + word.len(), word.chars().any(|c| c.is_alphanumeric() || c == '_')))
}

pub(crate) fn word_at(line: &str, at: usize) -> Range<usize> {
    let mut before: Option<(Range<usize>, bool)> = None;
    for (range, is_word) in segments(line) {
        if range.contains(&at) {
            return match before {
                Some((word, true)) if !is_word && range.start == at => word,
                _ => range,
            };
        }
        before = Some((range, is_word));
    }
    before.map_or(line.len()..line.len(), |(range, _)| range)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Edge {
    Top,
    Bottom,
}

impl Edge {
    pub(crate) fn a_row(self) -> Pixels {
        match self {
            Self::Top => px(-CODE_ROW),
            Self::Bottom => px(CODE_ROW),
        }
    }
}

pub(crate) fn edge_at(y: Pixels, body: Bounds<Pixels>) -> Option<Edge> {
    if y < body.top() {
        Some(Edge::Top)
    } else if y >= body.bottom() {
        Some(Edge::Bottom)
    } else {
        None
    }
}

pub(crate) fn row_at<S>(mut rows: impl Iterator<Item = (S, Bounds<Pixels>)>, y: Pixels) -> Option<S> {
    rows.find(|(_, bounds)| bounds.top() <= y && y < bounds.bottom()).map(|(spot, _)| spot)
}

pub(crate) fn row_near<S>(rows: impl Iterator<Item = (S, Bounds<Pixels>)>, y: Pixels) -> Option<S> {
    let mut found = None;
    for (spot, bounds) in rows {
        if found.is_some() && bounds.top() > y {
            break;
        }
        found = Some(spot);
    }
    found
}

pub(crate) fn host<S: Copy + Ord>(selection: Selection<S>, drawn: impl DoubleEndedIterator<Item = S>) -> Option<S> {
    let (first, last) = selection.ends();
    let mut inside = drawn.filter(|spot| first <= *spot && *spot <= last);
    if selection.head() == first { inside.next() } else { inside.next_back() }
}

pub(crate) fn off_centre(y: Pixels, body: Bounds<Pixels>, inset: Pixels) -> Option<Pixels> {
    if body.top() + inset <= y && y + px(CODE_ROW) <= body.bottom() {
        return None;
    }
    Some(y - (body.top() + inset + (body.size.height - inset) / 2.))
}

pub(crate) type Span = (Pixels, Pixels, Pixels);

pub(crate) fn off_edge((line_top, row_top, line_bottom): Span, body: Bounds<Pixels>, inset: Pixels) -> Option<Pixels> {
    let row = px(CODE_ROW);
    let (top, bottom) = (body.top() + inset + row, body.bottom() - row);
    let line_bottom = line_bottom.max(row_top + row);
    let (from, to) =
        if line_bottom - line_top <= bottom - top { (line_top, line_bottom) } else { (row_top, row_top + row) };
    if from < top {
        Some(from - top)
    } else if to > bottom {
        Some(to - bottom)
    } else {
        None
    }
}

pub(crate) fn row_from<S>(mut rows: impl Iterator<Item = (S, Bounds<Pixels>)>, top: Pixels) -> Option<S> {
    rows.find(|(_, bounds)| bounds.bottom() > top).map(|(spot, _)| spot)
}

pub(crate) fn bar_origin(host: Bounds<Pixels>, bar: Size<Pixels>, room: Bounds<Pixels>) -> Point<Pixels> {
    let x = (room.right() - bar.width - px(BAR_MARGIN)).max(room.left() + px(BAR_MARGIN));
    let below = host.bottom() + px(BAR_GAP);
    let y = if below + bar.height <= room.bottom() - px(BAR_MARGIN) {
        below
    } else {
        (host.top() - px(BAR_GAP) - bar.height).max(room.top() + px(BAR_MARGIN))
    };
    point(x, y.max(room.top() + px(BAR_MARGIN)))
}

fn selected_chars() -> HighlightStyle {
    HighlightStyle { background_color: Some(colours::selection().into()), ..HighlightStyle::default() }
}

fn covers(range: &Range<usize>, piece: &Range<usize>) -> bool {
    range.start <= piece.start && piece.end <= range.end
}

fn covering<'a, T>(items: &'a [T], range_of: impl Fn(&T) -> &Range<usize>, piece: &Range<usize>) -> Option<&'a T> {
    let at = items.partition_point(|item| range_of(item).end <= piece.start);
    items.get(at).filter(|item| covers(range_of(item), piece))
}

pub(crate) fn overlay(styles: Styles, lit: &[Range<usize>], paint: HighlightStyle) -> Styles {
    if lit.is_empty() {
        return styles;
    }
    let mut cuts: Vec<usize> =
        styles.iter().flat_map(|(r, _)| [r.start, r.end]).chain(lit.iter().flat_map(|r| [r.start, r.end])).collect();
    cuts.sort_unstable();
    cuts.dedup();
    let mut out = Vec::with_capacity(cuts.len());
    for pair in cuts.windows(2) {
        let piece = pair[0]..pair[1];
        let base = covering(&styles, |(range, _)| range, &piece).map(|(_, style)| *style);
        let in_lit = covering(lit, |range| range, &piece).is_some();
        let style = match (base, in_lit) {
            (None, false) => continue,
            (Some(style), false) => style,
            (base, true) => {
                let base = base.unwrap_or_default();
                HighlightStyle { background_color: paint.background_color, color: paint.color.or(base.color), ..base }
            }
        };
        out.push((piece, style));
    }
    out
}

pub(crate) fn tabs_drawn(own: &str) -> String {
    own.replace('\t', &" ".repeat(TAB_WIDTH))
}

pub(crate) fn own_if_drawn<'a>(raw: Option<&'a str>, drawn: &str) -> Option<&'a str> {
    raw.filter(|raw| tabs_drawn(raw) == drawn)
}

pub(crate) fn own_part(own: &str, shown: Range<usize>) -> &str {
    let (mut at, mut start, mut end) = (0, None, own.len());
    for (ix, c) in own.char_indices() {
        let width = if c == '\t' { TAB_WIDTH } else { c.len_utf8() };
        if start.is_none() && at + width > shown.start {
            start = Some(ix);
        }
        if at >= shown.end {
            end = ix;
            break;
        }
        at += width;
    }
    start.map_or("", |start| &own[start..end.max(start)])
}

pub(crate) fn reference<N: Display + PartialEq>(path: &str, (first, last): (N, N)) -> String {
    if first == last { format!("{path}:{first} ") } else { format!("{path}:{first}-{last} ") }
}

pub(crate) fn lines_label<N: Display + PartialEq>((first, last): (N, N)) -> String {
    if first == last { format!("line {first}") } else { format!("lines {first}-{last}") }
}

#[cfg(test)]
mod tests;
