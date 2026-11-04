use anyhow::Context as _;
use serde::Deserialize;

use crate::app::{Msg, Root};
use crate::desktop::{self, Board};
use crate::model::PaneGrid;
use crate::model::link::row_cells;
use crate::platform::Os;
use crate::platform::paths::Style;

pub(super) fn class(cell: &str, style: Style) -> u8 {
    match cell.chars().next() {
        None | Some(' ') => 0,
        Some(c) if c.is_alphanumeric() || "_-./~:@%+=#?&".contains(c) || style.is_separator(c) => 1,
        Some(_) => 2,
    }
}

pub(super) fn word_at(cells: &[&str], x: usize, style: Style) -> (usize, usize) {
    if x >= cells.len() {
        return (x, x);
    }
    let of = |c: &str| class(c, style);
    let classes: Vec<u8> = cells
        .iter()
        .enumerate()
        .map(|(i, c)| if c.is_empty() && i > 0 && !cells[i - 1].is_empty() { of(cells[i - 1]) } else { of(c) })
        .collect();
    let at = classes[x];
    let start = (0..x).rev().take_while(|&i| classes[i] == at).last().unwrap_or(x);
    let end = (x + 1..cells.len()).take_while(|&i| classes[i] == at).last().unwrap_or(x);
    (start, end)
}

pub(super) fn selection_args(pane: u64, from: (u16, u16), to: (u16, u16)) -> Vec<String> {
    vec![
        "selection".into(),
        "--pane".into(),
        pane.to_string(),
        "--from".into(),
        format!("{},{}", from.0, from.1),
        "--to".into(),
        format!("{},{}", to.0, to.1),
    ]
}

pub(super) fn read_args(pane: u64, board: Board) -> Vec<String> {
    let mut args = vec!["selection".into(), "--pane".into(), pane.to_string()];
    if board == Board::Clipboard {
        args.push("--copy".into());
    }
    args.push("--json".into());
    args
}

pub(super) fn column(index: usize) -> u16 {
    u16::try_from(index).unwrap_or(u16::MAX)
}

#[derive(Deserialize)]
struct Selected {
    text: Option<String>,
}

pub(super) fn kept(board: Board, read: anyhow::Result<String>) -> Option<Msg> {
    let selected = read.and_then(|out| {
        serde_json::from_str::<Selected>(&out).context("could not read the selection cornercase answered with")
    });
    match selected {
        Ok(Selected { text: Some(text) }) if !text.is_empty() => Some(Msg::Copied(board, text)),
        Ok(_) => None,
        Err(e) => Some(Msg::Error(format!("{e:#}"))),
    }
}

impl Root {
    pub(crate) fn copy_selection(&self, pane: u64) {
        self.keep_selection(pane, Board::Clipboard);
    }

    pub(super) fn selection_made(&self, pane: u64) {
        self.keep_selection(pane, desktop::selection_board(Os::CURRENT));
    }

    fn keep_selection(&self, pane: u64, board: Board) {
        let tx = self.tx.clone();
        self.queue.cmd_then(read_args(pane, board), move |read| {
            if let Some(msg) = kept(board, read) {
                let _ = tx.unbounded_send(msg);
            }
        });
    }

    fn select(&self, pane: u64, from: (u16, u16), to: (u16, u16)) {
        self.queue.cmd(selection_args(pane, from, to));
        self.selection_made(pane);
    }

    pub(super) fn select_by_clicks(&self, pane: &PaneGrid, (x, y): (u16, u16), clicks: usize) {
        let cells = row_cells(pane, y);
        let x = usize::from(x);
        if clicks >= 3 {
            self.select(pane.pane, (0, y), (pane.size.cols.saturating_sub(1), y));
        } else if x < cells.len() && class(cells[x], self.pointer.links.style) != 0 {
            let (start, end) = word_at(&cells, x, self.pointer.links.style);
            self.select(pane.pane, (column(start), y), (column(end), y));
        }
    }
}
