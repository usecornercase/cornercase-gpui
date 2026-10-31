use std::collections::HashMap;
use std::sync::Arc;

use super::surface::{
    Area, Colours, Copy, Cursor, Frame, Hyperlink, Modes, Node, PopupLabel, Scroll, Size, Style, WireLines, WirePane,
    WireRun,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Run {
    pub style: Style,
    pub link: Option<Arc<str>>,
    pub cells: Vec<String>,
}

pub(crate) type Line = Vec<Run>;

#[derive(Debug, Clone, Default)]
pub(crate) struct PaneGrid {
    pub pane: u64,
    pub active: bool,
    pub rect: Area,
    pub size: Size,
    pub cursor: Option<Cursor>,
    pub modes: Modes,
    pub scroll: Scroll,
    pub colours: Colours,
    pub lines: Vec<Line>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Grid {
    pub boot: String,
    pub surface: u64,
    pub revision: u64,
    pub tab: u64,
    pub cols: u16,
    pub rows: u16,
    pub layout: Option<Node>,
    pub popup: Option<PopupLabel>,
    pub panes: Vec<PaneGrid>,
}

pub(crate) enum Applied {
    Ok,
    NeedRedraw,
}

struct Tables {
    styles: Vec<Style>,
    links: Vec<Arc<str>>,
}

impl Tables {
    fn new(styles: Vec<Style>, hyperlinks: Vec<Hyperlink>) -> Self {
        Self { styles, links: hyperlinks.into_iter().map(|h| Arc::from(h.uri)).collect() }
    }

    fn line(&self, runs: Vec<WireRun>) -> Line {
        runs.into_iter()
            .map(|r| Run {
                style: self.styles.get(r.style).copied().unwrap_or_default(),
                link: r.hyperlink.and_then(|h| self.links.get(h).cloned()),
                cells: r.cells,
            })
            .collect()
    }

    fn lines(&self, wire: Vec<Vec<WireRun>>) -> Vec<Line> {
        wire.into_iter().map(|runs| self.line(runs)).collect()
    }
}

fn clamp_axis(at: i32, start: u16, len: u16) -> u16 {
    let last = len.saturating_sub(1);
    u16::try_from((at - i32::from(start)).clamp(0, i32::from(last))).unwrap_or(last)
}

impl PaneGrid {
    pub(crate) fn cell(&self, x: u16, y: u16) -> Option<(&str, &Run)> {
        let line = self.lines.get(usize::from(y))?;
        let x = usize::from(x);
        let mut col = 0usize;
        for run in line {
            if x < col + run.cells.len() {
                return Some((&run.cells[x - col], run));
            }
            col += run.cells.len();
        }
        None
    }

    pub(crate) fn clamp_cell(&self, x: i32, y: i32) -> (u16, u16) {
        (clamp_axis(x, self.rect.x, self.size.cols), clamp_axis(y, self.rect.y, self.size.rows))
    }

    fn from_wire(wire: WirePane, tables: &Tables) -> Self {
        Self {
            pane: wire.pane,
            active: wire.active.unwrap_or(false),
            rect: wire.rect.unwrap_or_default(),
            size: wire.size.unwrap_or_default(),
            cursor: wire.cursor,
            modes: wire.modes.unwrap_or_default(),
            scroll: wire.scroll.unwrap_or_default(),
            colours: wire.colours.unwrap_or_default(),
            lines: match wire.lines {
                Some(WireLines::All(all)) => tables.lines(all),
                _ => Vec::new(),
            },
        }
    }

    fn patch(&mut self, wire: WirePane, tables: &Tables) {
        self.cursor = wire.cursor;
        self.active = wire.active.unwrap_or(self.active);
        self.modes = wire.modes.unwrap_or(self.modes);
        self.scroll = wire.scroll.unwrap_or(self.scroll);
        if let Some(colours) = wire.colours {
            self.colours = colours;
        }
        if let Some(copy) = wire.copy {
            self.copy_lines(copy);
        }
        match wire.lines {
            Some(WireLines::Changed(changed)) => self.replace_rows(changed, tables),
            Some(WireLines::All(all)) => self.lines = tables.lines(all),
            None => {}
        }
    }

    fn copy_lines(&mut self, copy: Copy) {
        let held = self.lines.clone();
        for i in 0..usize::from(copy.count) {
            let (from, to) = (usize::from(copy.from) + i, usize::from(copy.to) + i);
            if let (Some(source), Some(target)) = (held.get(from), self.lines.get_mut(to)) {
                target.clone_from(source);
            }
        }
    }

    fn replace_rows(&mut self, changed: HashMap<String, Vec<WireRun>>, tables: &Tables) {
        for (row, runs) in changed {
            let Ok(row) = row.parse::<usize>() else { continue };
            if row >= self.lines.len() {
                self.lines.resize(row + 1, Vec::new());
            }
            self.lines[row] = tables.line(runs);
        }
    }
}

impl Grid {
    pub(crate) fn active_pane(&self) -> Option<&PaneGrid> {
        self.panes.iter().find(|p| p.active).or(self.panes.first())
    }

    pub(crate) fn pane(&self, id: u64) -> Option<&PaneGrid> {
        self.panes.iter().find(|p| p.pane == id)
    }

    pub(crate) fn apply(&mut self, mut frame: Frame) -> Applied {
        let tables = Tables::new(std::mem::take(&mut frame.styles), std::mem::take(&mut frame.hyperlinks));
        match frame.base {
            None => {
                self.replace(frame, &tables);
                Applied::Ok
            }
            Some(base) => self.patch(base, frame, &tables),
        }
    }

    fn replace(&mut self, frame: Frame, tables: &Tables) {
        self.boot = frame.boot;
        self.surface = frame.surface;
        self.revision = frame.revision;
        self.tab = frame.tab;
        self.cols = frame.cols;
        self.rows = frame.rows;
        self.layout = frame.layout;
        self.popup = frame.popup;
        self.panes = frame.panes.into_iter().map(|p| PaneGrid::from_wire(p, tables)).collect();
    }

    fn patch(&mut self, base: u64, frame: Frame, tables: &Tables) -> Applied {
        if base != self.revision || frame.surface != self.surface || frame.boot != self.boot {
            return Applied::NeedRedraw;
        }
        for wire in frame.panes {
            let Some(grid) = self.panes.iter_mut().find(|g| g.pane == wire.pane) else {
                return Applied::NeedRedraw;
            };
            grid.patch(wire, tables);
        }
        self.revision = frame.revision;
        Applied::Ok
    }
}
