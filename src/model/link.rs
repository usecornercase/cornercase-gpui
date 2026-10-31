use serde::Deserialize;

use super::{Grid, PaneGrid};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub(crate) struct Lines {
    pub first: usize,
    pub last: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct Link {
    pub path: String,
    #[serde(default)]
    pub inside: bool,
    pub workspace: Option<u64>,
    pub relative: Option<String>,
    pub lines: Option<Lines>,
    pub row: u16,
    pub start: u16,
    pub end: u16,
}

#[derive(Deserialize)]
struct Answer {
    link: Option<Link>,
}

impl Link {
    pub(crate) fn parse(json: &str) -> Option<Self> {
        serde_json::from_str::<Answer>(json).ok()?.link.filter(|link| link.start < link.end)
    }

    pub(crate) fn covers(&self, (x, y): (u16, u16)) -> bool {
        y == self.row && (self.start..self.end).contains(&x)
    }

    pub(crate) fn shown_path(&self) -> &str {
        self.relative.as_deref().filter(|_| self.inside).unwrap_or(&self.path)
    }

    pub(crate) fn name(&self) -> &str {
        let path = self.path.trim_end_matches('/');
        path.rsplit('/').next().unwrap_or(path)
    }

    pub(crate) fn first_line(&self) -> Option<usize> {
        self.lines.map(|lines| lines.first.saturating_sub(1))
    }

    pub(crate) fn last_line(&self) -> Option<usize> {
        self.lines.map(|lines| lines.last.max(lines.first).saturating_sub(1))
    }

    pub(crate) fn same_file(&self, other: &Link) -> bool {
        self.path == other.path && self.lines == other.lines
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hovered {
    pub pane: u64,
    pub link: Link,
    pub text: String,
}

impl Hovered {
    pub(crate) fn new(pane: &PaneGrid, link: Link) -> Self {
        let text = text_between(&row_cells(pane, link.row), usize::from(link.start), usize::from(link.end));
        Self { pane: pane.pane, link, text }
    }

    pub(crate) fn shown_in(&self, grid: &Grid) -> bool {
        let at = (self.link.row, self.link.start, self.link.end);
        grid.pane(self.pane).is_some_and(|pane| shows(pane, at, &self.text))
    }

    pub(crate) fn at(&self, pane: u64, cell: (u16, u16)) -> bool {
        self.pane == pane && self.link.covers(cell)
    }
}

pub(crate) fn row_cells(pane: &PaneGrid, row: u16) -> Vec<&str> {
    pane.lines
        .get(usize::from(row))
        .map(|line| line.iter().flat_map(|run| run.cells.iter().map(String::as_str)).collect())
        .unwrap_or_default()
}

pub(crate) fn text_between(cells: &[&str], start: usize, end: usize) -> String {
    cells.get(start..end.min(cells.len())).map(<[&str]>::concat).unwrap_or_default()
}

pub(crate) fn shows(pane: &PaneGrid, (row, start, end): (u16, u16, u16), text: &str) -> bool {
    text_between(&row_cells(pane, row), usize::from(start), usize::from(end)) == text
}

#[cfg(test)]
mod tests;
