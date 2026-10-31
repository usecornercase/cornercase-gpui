use gpui::{Bounds, Pixels, Point, point, px, size};

use crate::model::{Area, Branch, Grid, Node, PaneGrid};

pub(crate) const PADDING: f32 = 12.;
pub(crate) const GAP: f32 = 8.;
const GRIP: f32 = 4.;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Card {
    pub pane: u64,
    pub rect: Area,
    pub bounds: Bounds<Pixels>,
    pub cells: Point<Pixels>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Along {
    start: f32,
    before: f32,
    cells: f32,
    cell: f32,
}

impl Along {
    pub(crate) fn ratio(self, at: f32) -> f64 {
        let first = (at - self.start - GAP / 2. - self.before) / self.cell;
        f64::from(first / self.cells.max(1.))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Gap {
    pub path: Vec<bool>,
    pub down: bool,
    pub bounds: Bounds<Pixels>,
    pub along: Along,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Hit {
    Pane(u64, (i32, i32)),
    Gap(Gap),
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Layout {
    pub cards: Vec<Card>,
    pub gaps: Vec<Gap>,
    pub at: Bounds<Pixels>,
    cell: (f32, f32),
}

struct Cut {
    first: Bounds<Pixels>,
    gap: Bounds<Pixels>,
    second: Bounds<Pixels>,
}

struct Builder<'a> {
    panes: &'a [PaneGrid],
    cell: (f32, f32),
    layout: Layout,
}

fn down(branch: &Branch) -> bool {
    branch.dir == "down"
}

fn with(path: &[bool], side: bool) -> Vec<bool> {
    path.iter().copied().chain([side]).collect()
}

fn union(a: Area, b: Area) -> Area {
    let (x, y) = (a.x.min(b.x), a.y.min(b.y));
    let (right, bottom) = ((a.x + a.cols).max(b.x + b.cols), (a.y + a.rows).max(b.y + b.rows));
    Area { x, y, cols: right - x, rows: bottom - y }
}

pub(crate) fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(w.max(0.)), px(h.max(0.))))
}

fn cut(at: Bounds<Pixels>, first: f32, down: bool) -> Cut {
    let (x, y) = (f32::from(at.origin.x), f32::from(at.origin.y));
    let (w, h) = (f32::from(at.size.width), f32::from(at.size.height));
    if down {
        let first = first.round().min(h);
        return Cut {
            first: rect(x, y, w, first),
            gap: rect(x, y + first - GRIP, w, GAP + GRIP * 2.),
            second: rect(x, y + first + GAP, w, h - first - GAP),
        };
    }
    let first = first.round().min(w);
    Cut {
        first: rect(x, y, first, h),
        gap: rect(x + first - GRIP, y, GAP + GRIP * 2., h),
        second: rect(x + first + GAP, y, w - first - GAP, h),
    }
}

#[expect(clippy::cast_possible_truncation, reason = "a pointer's offset in cells is far inside i32")]
fn floor_to_cell(cells: f32) -> i32 {
    cells.floor() as i32
}

impl Builder<'_> {
    fn rect(&self, pane: u64) -> Area {
        self.panes.iter().find(|p| p.pane == pane).map_or_else(Area::default, |p| p.rect)
    }

    fn span(&self, node: &Node) -> Area {
        match node {
            Node::Leaf(pane) => self.rect(*pane),
            Node::Split(branch) => union(self.span(&branch.first), self.span(&branch.second)),
        }
    }

    fn need(&self, node: &Node) -> (f32, f32) {
        let Node::Split(branch) = node else {
            let rect = self.span(node);
            let cells = (f32::from(rect.cols) * self.cell.0, f32::from(rect.rows) * self.cell.1);
            return (cells.0 + PADDING * 2., cells.1 + PADDING * 2.);
        };
        let (first, second) = (self.need(&branch.first), self.need(&branch.second));
        if down(branch) {
            (first.0.max(second.0), first.1 + GAP + second.1)
        } else {
            (first.0 + GAP + second.0, first.1.max(second.1))
        }
    }

    fn extra(&self, node: &Node) -> (f32, f32) {
        let (need, span) = (self.need(node), self.span(node));
        (need.0 - f32::from(span.cols) * self.cell.0, need.1 - f32::from(span.rows) * self.cell.1)
    }

    fn place(&mut self, node: &Node, at: Bounds<Pixels>, path: &[bool]) {
        match node {
            Node::Leaf(pane) => self.leaf(*pane, at),
            Node::Split(branch) => self.split(branch, at, path),
        }
    }

    fn leaf(&mut self, pane: u64, at: Bounds<Pixels>) {
        let cells = point(at.origin.x + px(PADDING), at.origin.y + px(PADDING));
        self.layout.cards.push(Card { pane, rect: self.rect(pane), bounds: at, cells });
    }

    fn split(&mut self, branch: &Branch, at: Bounds<Pixels>, path: &[bool]) {
        let down = down(branch);
        let need = self.need(&branch.first);
        let parts = cut(at, if down { need.1 } else { need.0 }, down);
        let along = self.along(branch, at, down);
        self.layout.gaps.push(Gap { path: path.to_vec(), down, bounds: parts.gap, along });
        self.place(&branch.first, parts.first, &with(path, false));
        self.place(&branch.second, parts.second, &with(path, true));
    }

    fn along(&self, branch: &Branch, at: Bounds<Pixels>, down: bool) -> Along {
        let (first, second) = (self.span(&branch.first), self.span(&branch.second));
        let extra = self.extra(&branch.first);
        if down {
            let cells = f32::from(first.rows + second.rows);
            return Along { start: f32::from(at.origin.y), before: extra.1, cells, cell: self.cell.1 };
        }
        let cells = f32::from(first.cols + second.cols);
        Along { start: f32::from(at.origin.x), before: extra.0, cells, cell: self.cell.0 }
    }

    fn flat(&mut self, at: Bounds<Pixels>) {
        let inner = point(at.origin.x + px(PADDING), at.origin.y + px(PADDING));
        for pane in self.panes {
            let (x, y) = (f32::from(pane.rect.x) * self.cell.0, f32::from(pane.rect.y) * self.cell.1);
            let cells = point(inner.x + px(x), inner.y + px(y));
            let area = size(px(f32::from(pane.rect.cols) * self.cell.0), px(f32::from(pane.rect.rows) * self.cell.1));
            self.layout.cards.push(Card { pane: pane.pane, rect: pane.rect, bounds: Bounds::new(cells, area), cells });
        }
    }
}

impl Layout {
    pub(crate) fn new(grid: &Grid, cell: (f32, f32), at: Bounds<Pixels>) -> Self {
        let mut builder = Builder { panes: &grid.panes, cell, layout: Self { at, cell, ..Self::default() } };
        match (&grid.layout, grid.panes.as_slice()) {
            (Some(node), _) => builder.place(node, at, &[]),
            (None, [pane]) => builder.leaf(pane.pane, at),
            (None, _) => builder.flat(at),
        }
        builder.layout
    }

    pub(crate) fn extra(grid: &Grid, cell: (f32, f32)) -> (f32, f32) {
        let builder = Builder { panes: &grid.panes, cell, layout: Self::default() };
        grid.layout.as_ref().map_or((PADDING * 2., PADDING * 2.), |node| builder.extra(node))
    }

    pub(crate) fn card(&self, pane: u64) -> Option<&Card> {
        self.cards.iter().find(|c| c.pane == pane)
    }

    pub(crate) fn hit(&self, at: Point<Pixels>) -> Option<Hit> {
        if let Some(gap) = self.gaps.iter().find(|g| g.bounds.contains(&at)) {
            return Some(Hit::Gap(gap.clone()));
        }
        let card = self.cards.iter().find(|c| c.bounds.contains(&at))?;
        Some(Hit::Pane(card.pane, self.cell_of(card, at)))
    }

    pub(crate) fn span(&self, pane: u64, row: u16, (start, end): (u16, u16)) -> Option<Bounds<Pixels>> {
        let card = self.card(pane)?;
        let (cw, lh) = self.cell;
        let x = f32::from(card.cells.x) + cw * f32::from(start);
        let y = f32::from(card.cells.y) + lh * f32::from(row);
        Some(rect(x, y, cw * f32::from(end.saturating_sub(start)), lh))
    }

    pub(crate) fn cell_in(&self, pane: u64, at: Point<Pixels>) -> Option<(i32, i32)> {
        self.card(pane).map(|card| self.cell_of(card, at))
    }

    fn cell_of(&self, card: &Card, at: Point<Pixels>) -> (i32, i32) {
        let x = f32::from(at.x - card.cells.x) / self.cell.0;
        let y = f32::from(at.y - card.cells.y) / self.cell.1;
        (i32::from(card.rect.x) + floor_to_cell(x), i32::from(card.rect.y) + floor_to_cell(y))
    }
}

#[cfg(test)]
mod tests;
