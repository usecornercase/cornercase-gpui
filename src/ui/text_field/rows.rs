use std::ops::{Range, RangeInclusive};

use gpui::{Pixels, px};
use unicode_segmentation::UnicodeSegmentation;

const SHORTEST_THUMB: Pixels = px(16.);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Row {
    pub(super) line: usize,
    pub(super) line_start: usize,
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) ends_line: bool,
}

impl Row {
    pub(super) fn last_caret(self, text: &str) -> usize {
        if self.ends_line { self.end } else { previous_grapheme(text, self.end).max(self.start) }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Span {
    pub(super) row: usize,
    pub(super) range: Range<usize>,
    pub(super) line_break: bool,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Rows(Vec<Row>);

impl Rows {
    pub(super) fn new(lines: impl IntoIterator<Item = (usize, Vec<usize>)>) -> Self {
        let mut rows = Vec::new();
        let mut line_start = 0;
        for (line, (len, starts)) in lines.into_iter().enumerate() {
            let ends = starts.iter().skip(1).copied().chain([len]);
            rows.extend(starts.iter().zip(ends).map(|(&start, end)| Row {
                line,
                line_start,
                start: line_start + start,
                end: line_start + end,
                ends_line: end == len,
            }));
            line_start += len + 1;
        }
        Self(rows)
    }

    pub(super) fn len(&self) -> usize {
        self.0.len()
    }

    pub(super) fn get(&self, row: usize) -> Option<Row> {
        self.0.get(row).copied()
    }

    pub(super) fn row_of(&self, offset: usize) -> usize {
        self.0.partition_point(|row| row.start <= offset).saturating_sub(1)
    }

    pub(super) fn selected(&self, selected: &Range<usize>) -> Vec<Span> {
        self.0
            .iter()
            .enumerate()
            .filter_map(|(row, line)| {
                let range = selected.start.max(line.start)..selected.end.min(line.end);
                let line_break = line.ends_line && selected.start <= line.end && selected.end > line.end;
                (range.start < range.end || line_break).then_some(Span { row, range, line_break })
            })
            .collect()
    }
}

pub(super) fn previous_grapheme(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true).rev().find_map(|(index, _)| (index < offset).then_some(index)).unwrap_or(0)
}

pub(super) fn line_breaks(pasted: &str) -> String {
    pasted.replace("\r\n", "\n").replace('\r', "\n")
}

pub(super) fn one_line(pasted: &str) -> String {
    pasted.replace("\r\n", " ").replace(['\r', '\n'], " ")
}

pub(super) fn shown_without_controls(text: &str) -> Option<String> {
    text.contains(['\t', '\r']).then(|| text.replace(['\t', '\r'], " "))
}

pub(super) fn rows_shown(rows: usize, limits: Option<&RangeInclusive<usize>>) -> usize {
    limits.map_or(rows, |limits| rows.clamp(*limits.start(), *limits.end()))
}

pub(super) fn scroll_to_show(scroll: Pixels, top: Pixels, bottom: Pixels, height: Pixels) -> Pixels {
    if top < scroll {
        top
    } else if bottom > scroll + height {
        bottom - height
    } else {
        scroll
    }
}

pub(super) fn scroll_to_show_with_margins(
    scroll: Pixels,
    (caret, line_height): (Pixels, Pixels),
    (above, below): (Pixels, Pixels),
    height: Pixels,
) -> Pixels {
    let free = (height - line_height).max(px(0.));
    let above = above.min(free / 2.);
    let below = below.min(free - above);
    scroll_to_show(scroll, caret - above, caret + line_height + below, height)
}

pub(super) fn clamp_scroll(scroll: Pixels, content: Pixels, height: Pixels) -> Pixels {
    scroll.min(content - height).max(px(0.))
}

pub(super) fn thumb(scroll: Pixels, content: Pixels, height: Pixels) -> Option<(Pixels, Pixels)> {
    (content > height).then(|| {
        let length = (height * (height / content)).max(SHORTEST_THUMB);
        ((height - length) * (scroll / (content - height)), length)
    })
}

#[cfg(test)]
mod tests;
