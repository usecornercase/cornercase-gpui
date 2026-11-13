use std::ops::Range;

use gpui::{SharedString, TextRun, UnderlineStyle, px};
use unicode_segmentation::UnicodeSegmentation;

use super::rows::Row;

pub(super) const BULLET: char = '•';

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) enum Shown {
    #[default]
    Itself,
    Bullets(Vec<usize>),
}

impl Shown {
    pub(super) fn for_content(content: &SharedString, masked: bool) -> (SharedString, Self) {
        if !masked {
            return (shown_without_controls(content).map_or_else(|| content.clone(), SharedString::from), Self::Itself);
        }
        let mut starts: Vec<usize> = content.grapheme_indices(true).map(|(at, _)| at).collect();
        let text = BULLET.to_string().repeat(starts.len());
        starts.push(content.len());
        (text.into(), Self::Bullets(starts))
    }

    pub(super) fn to_shown(&self, offset: usize) -> usize {
        match self {
            Self::Itself => offset,
            Self::Bullets(starts) => {
                starts.partition_point(|&start| start <= offset).saturating_sub(1) * BULLET.len_utf8()
            }
        }
    }

    pub(super) fn to_content(&self, index: usize) -> usize {
        match self {
            Self::Itself => index,
            Self::Bullets(starts) => starts.get(index / BULLET.len_utf8()).or(starts.last()).copied().unwrap_or(0),
        }
    }

    pub(super) fn range_to_shown(&self, range: &Range<usize>) -> Range<usize> {
        self.to_shown(range.start)..self.to_shown(range.end)
    }

    pub(super) fn row_ends(&self, row: Row, text: &str) -> (usize, usize) {
        let row = Row { start: self.to_content(row.start), end: self.to_content(row.end), ..row };
        (row.start, row.last_caret(text))
    }

    pub(super) fn runs(&self, run: TextRun, marked: Option<&Range<usize>>) -> Vec<TextRun> {
        let Some(marked) = marked.map(|marked| self.range_to_shown(marked)) else { return vec![run] };
        let total = run.len;
        let end = marked.end.min(total);
        let start = marked.start.min(end);
        vec![
            TextRun { len: start, ..run.clone() },
            TextRun {
                len: end - start,
                underline: Some(UnderlineStyle { color: Some(run.color), thickness: px(1.0), wavy: false }),
                ..run.clone()
            },
            TextRun { len: total - end, ..run },
        ]
        .into_iter()
        .filter(|run| run.len > 0)
        .collect()
    }
}

pub(super) fn shown_without_controls(text: &str) -> Option<String> {
    text.contains(['\t', '\r']).then(|| text.replace(['\t', '\r'], " "))
}

#[cfg(test)]
mod tests;
