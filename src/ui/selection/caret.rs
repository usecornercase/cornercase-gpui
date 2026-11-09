use gpui::Keystroke;
use unicode_segmentation::GraphemeCursor;

use super::{Caret, Fit, Selection};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    Left,
    Right,
    Up,
    Down,
}

impl Step {
    pub(crate) fn of(keystroke: &Keystroke) -> Option<Self> {
        let held = &keystroke.modifiers;
        if !held.shift || held.control || held.alt || held.platform {
            return None;
        }
        match keystroke.key.as_str() {
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "up" => Some(Self::Up),
            "down" => Some(Self::Down),
            _ => None,
        }
    }
}

pub(crate) trait ShownLines<S> {
    fn text(&self, spot: S) -> Option<&str>;
    fn after(&self, spot: S) -> Option<S>;
    fn before(&self, spot: S) -> Option<S>;
}

impl ShownLines<usize> for [String] {
    fn text(&self, spot: usize) -> Option<&str> {
        self.get(spot).map(String::as_str)
    }

    fn after(&self, spot: usize) -> Option<usize> {
        (spot + 1 < self.len()).then_some(spot + 1)
    }

    fn before(&self, spot: usize) -> Option<usize> {
        spot.checked_sub(1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Keyed<S> {
    pub(super) selection: Selection<S>,
    pub(super) column: Option<usize>,
}

fn end_of<S: Copy, L: ShownLines<S> + ?Sized>(lines: &L, spot: S) -> Caret<S> {
    Caret { spot, at: lines.text(spot).map_or(0, str::len) }
}

fn as_chars<S: Copy + Ord, L: ShownLines<S> + ?Sized>(lines: &L, selection: Selection<S>) -> (Caret<S>, Caret<S>) {
    match selection {
        Selection::Chars { anchor, head } => (anchor, head),
        Selection::Lines { anchor, head } if head >= anchor => (Caret { spot: anchor, at: 0 }, end_of(lines, head)),
        Selection::Lines { anchor, head } => (end_of(lines, anchor), Caret { spot: head, at: 0 }),
    }
}

pub(super) fn extended<S: Copy + Ord, L: ShownLines<S> + ?Sized>(
    lines: &L,
    fit: Fit,
    from: Keyed<S>,
    step: Step,
) -> Keyed<S> {
    if let (Selection::Lines { anchor, head }, Step::Up | Step::Down) = (from.selection, step) {
        let moved = if step == Step::Up { lines.before(head) } else { lines.after(head) };
        return Keyed { selection: Selection::Lines { anchor, head: moved.unwrap_or(head) }, column: None };
    }
    let (anchor, head) = as_chars(lines, from.selection);
    let (head, column) = caret_step(lines, fit, (head, from.column), step);
    Keyed { selection: Selection::Chars { anchor, head }, column }
}

fn next_grapheme(line: &str, at: usize) -> usize {
    GraphemeCursor::new(at, line.len(), true)
        .next_boundary(line, 0)
        .ok()
        .flatten()
        .unwrap_or_else(|| line.ceil_char_boundary(at + 1))
}

fn previous_grapheme(line: &str, at: usize) -> usize {
    GraphemeCursor::new(at, line.len(), true)
        .prev_boundary(line, 0)
        .ok()
        .flatten()
        .unwrap_or_else(|| line.floor_char_boundary(at.saturating_sub(1)))
}

fn next_stop(line: &str, at: usize, kept: Option<usize>) -> Option<usize> {
    if at >= line.len() {
        return None;
    }
    match kept {
        Some(cut) if at >= cut => Some(line.len()),
        Some(cut) => Some(next_grapheme(line, at).min(cut)),
        None => Some(next_grapheme(line, at)),
    }
}

fn previous_stop(line: &str, at: usize, kept: Option<usize>) -> Option<usize> {
    if at == 0 {
        return None;
    }
    match kept {
        Some(cut) if at > cut => Some(cut),
        _ => Some(previous_grapheme(line, at)),
    }
}

fn column_of(line: &str, at: usize) -> usize {
    line[..at].chars().count()
}

fn at_column(line: &str, column: usize, kept: Option<usize>) -> usize {
    let at = line.char_indices().nth(column).map_or(line.len(), |(at, _)| at);
    if kept.is_some_and(|cut| at > cut) { line.len() } else { at }
}

fn sideways<S: Copy, L: ShownLines<S> + ?Sized>(
    lines: &L,
    (caret, line): (Caret<S>, &str),
    kept: Option<usize>,
    right: bool,
) -> Caret<S> {
    let stop = if right { next_stop(line, caret.at, kept) } else { previous_stop(line, caret.at, kept) };
    if let Some(at) = stop {
        return Caret { spot: caret.spot, at };
    }
    let neighbour = if right { lines.after(caret.spot) } else { lines.before(caret.spot) };
    match (neighbour, right) {
        (Some(spot), true) => Caret { spot, at: 0 },
        (Some(spot), false) => end_of(lines, spot),
        (None, _) => caret,
    }
}

fn vertical<S: Copy, L: ShownLines<S> + ?Sized>(
    lines: &L,
    fit: Fit,
    (caret, line): (Caret<S>, &str),
    (column, down): (usize, bool),
) -> Caret<S> {
    let neighbour = if down { lines.after(caret.spot) } else { lines.before(caret.spot) };
    let Some((spot, text)) = neighbour.and_then(|spot| Some((spot, lines.text(spot)?))) else {
        return Caret { spot: caret.spot, at: if down { line.len() } else { 0 } };
    };
    Caret { spot, at: at_column(text, column, fit.kept(text)) }
}

fn caret_step<S: Copy, L: ShownLines<S> + ?Sized>(
    lines: &L,
    fit: Fit,
    (caret, column): (Caret<S>, Option<usize>),
    step: Step,
) -> (Caret<S>, Option<usize>) {
    let Some(line) = lines.text(caret.spot) else { return (caret, column) };
    let at = line.floor_char_boundary(caret.at);
    let caret = Caret { spot: caret.spot, at };
    match step {
        Step::Left | Step::Right => (sideways(lines, (caret, line), fit.kept(line), step == Step::Right), None),
        Step::Up | Step::Down => {
            let column = column.unwrap_or_else(|| column_of(line, at));
            (vertical(lines, fit, (caret, line), (column, step == Step::Down)), Some(column))
        }
    }
}

#[cfg(test)]
mod tests;
