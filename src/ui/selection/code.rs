use gpui::{
    AnyElement, CursorStyle, Div, ElementId, HighlightStyle, IntoElement, ParentElement as _, Pixels, Stateful,
    StatefulInteractiveElement as _, Styled as _, StyledText, div, px,
};

use crate::notify::tip;
use crate::ui::{self, Icon};

use super::pointer::{Laid, break_mark, row_probe};
use super::{Lit, Styles};

pub(crate) const CODE_CELL: f32 = 7.3;
pub(crate) const CODE_ROW: f32 = 18.;
const WRAP_ROWS: usize = 100;
const UNKNOWN_COLUMNS: usize = 200;
const LIT_PAST_THE_CUT: usize = 4096;
const ELLIPSIS: &str = "…";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fit {
    Wrap { columns: usize },
    Cut,
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a width in pixels, at least one cell")]
fn columns_in(width: Pixels) -> usize {
    let width = f32::from(width);
    if width < CODE_CELL { UNKNOWN_COLUMNS } else { (width / CODE_CELL) as usize }
}

impl Fit {
    pub(crate) fn of(wraps: bool, width: Pixels) -> Self {
        if wraps { Self::Wrap { columns: columns_in(width) } } else { Self::Cut }
    }

    pub(crate) fn kept(self, line: &str) -> Option<usize> {
        cut(line, self.cap()?)
    }

    fn cap(self) -> Option<usize> {
        match self {
            Self::Wrap { columns } => Some(WRAP_ROWS * columns),
            Self::Cut => None,
        }
    }

    pub(crate) fn lit_bytes(self) -> usize {
        self.cap().unwrap_or(0) + LIT_PAST_THE_CUT
    }

    pub(crate) fn rows_of(self, bytes: usize) -> usize {
        match self {
            Self::Wrap { columns } => bytes.div_ceil(columns).clamp(1, WRAP_ROWS),
            Self::Cut => 1,
        }
    }

    pub(crate) fn row_at(self, byte: usize) -> usize {
        match self {
            Self::Wrap { columns } => (byte / columns).min(WRAP_ROWS - 1),
            Self::Cut => 0,
        }
    }

    pub(crate) fn row(self, frame: Div) -> Div {
        match self {
            Self::Wrap { .. } => frame,
            Self::Cut => frame.h(px(CODE_ROW)).whitespace_nowrap(),
        }
    }
}

struct Capped {
    shown: String,
    styles: Styles,
    kept: Option<usize>,
}

fn cut(line: &str, bytes: usize) -> Option<usize> {
    (line.len() > bytes).then(|| line.floor_char_boundary(bytes))
}

fn capped(line: &str, mut styles: Styles, bytes: usize) -> Capped {
    let Some(kept) = cut(line, bytes) else {
        let shown = stand_in(line);
        styles.retain(|(range, _)| shown.get(range.clone()).is_some());
        return Capped { shown, styles, kept: None };
    };
    let tint = styles.iter().filter(|(range, _)| range.end > kept).find_map(|(_, style)| style.background_color);
    let before = styles.into_iter().filter(|(range, _)| range.start < kept);
    let mut cut: Styles = before.map(|(range, style)| (range.start..range.end.min(kept), style)).collect();
    cut.push((kept..kept + ELLIPSIS.len(), HighlightStyle { background_color: tint, ..HighlightStyle::default() }));
    Capped { shown: format!("{}{ELLIPSIS}", &line[..kept]), styles: cut, kept: Some(kept) }
}

fn stand_in(line: &str) -> String {
    if line.is_empty() { " ".to_string() } else { line.to_string() }
}

pub(super) fn past_the_cut(index: usize, kept: Option<usize>, len: usize) -> usize {
    if kept.is_some_and(|kept| index > kept) { len } else { index }
}

pub(super) fn break_index(len: usize, kept: Option<usize>, shown: usize) -> usize {
    if kept.is_some() { shown } else { len }
}

pub(super) fn break_x(len: usize, start: Pixels, end: Pixels) -> Pixels {
    if len == 0 { start } else { end }
}

pub(crate) fn code_cell<S: Copy + Ord + 'static>(
    laid: &Laid<S>,
    (spot, line): (S, &str),
    (styles, lit): (Styles, &Lit),
    fit: Fit,
) -> Div {
    let styles = lit.painted(styles);
    let mark = lit.draws_break().then(|| break_mark(laid, spot));
    let cell = div().relative().flex_1().min_w_0().pr(px(8.)).cursor(CursorStyle::IBeam);
    let Some(bytes) = fit.cap() else {
        let (laid, len) = (laid.clone(), line.len());
        let text = ui::one_line(stand_in(line)).highlights(styles);
        return cell.child(text.on_laid(move |out| laid.add_line(spot, len, out))).children(mark);
    };
    let capped = capped(line, styles, bytes);
    let text = StyledText::new(capped.shown).with_highlights(capped.styles);
    let probe = row_probe(laid, (spot, line.len(), capped.kept), text.layout().clone());
    cell.child(text).child(probe).children(mark)
}

pub(crate) fn plain_text(line: &str, fit: Fit) -> AnyElement {
    match fit.cap() {
        Some(bytes) => StyledText::new(capped(line, Vec::new(), bytes).shown).into_any_element(),
        None => ui::one_line(stand_in(line)).into_any_element(),
    }
}

pub(crate) fn wrap_button(id: impl Into<ElementId>, wraps: bool) -> Stateful<Div> {
    ui::tool_button(id, Icon::WrapText, 24., wraps).tooltip(tip("Wrap long lines, or cut them at the edge"))
}

#[cfg(test)]
mod tests;
