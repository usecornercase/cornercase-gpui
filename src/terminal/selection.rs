use crate::app::{Msg, Root};
use crate::model::PaneGrid;
use crate::model::link::row_cells;

pub(super) fn class(cell: &str) -> u8 {
    match cell.chars().next() {
        None | Some(' ') => 0,
        Some(c) if c.is_alphanumeric() || "_-./~:@%+=#?&".contains(c) => 1,
        Some(_) => 2,
    }
}

pub(super) fn word_at(cells: &[&str], x: usize) -> (usize, usize) {
    if x >= cells.len() {
        return (x, x);
    }
    let classes: Vec<u8> = cells
        .iter()
        .enumerate()
        .map(|(i, c)| if c.is_empty() && i > 0 && !cells[i - 1].is_empty() { class(cells[i - 1]) } else { class(c) })
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

pub(super) fn column(index: usize) -> u16 {
    u16::try_from(index).unwrap_or(u16::MAX)
}

fn text_of_selection(out: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(out).ok()?;
    value.get("text")?.as_str().map(String::from)
}

impl Root {
    pub(crate) fn copy_selection(&self, pane: u64) {
        let tx = self.tx.clone();
        let args = ["selection", "--pane", &pane.to_string(), "--copy", "--json"].map(String::from);
        self.queue.cmd_then(args.to_vec(), move |r| {
            let text = r.ok().and_then(|out| text_of_selection(&out));
            if let Some(text) = text.filter(|t| !t.is_empty()) {
                let _ = tx.unbounded_send(Msg::Copied(text));
            }
        });
    }

    fn select(&self, pane: u64, from: (u16, u16), to: (u16, u16)) {
        self.queue.cmd(selection_args(pane, from, to));
        self.copy_selection(pane);
    }

    pub(super) fn select_by_clicks(&self, pane: &PaneGrid, (x, y): (u16, u16), clicks: usize) {
        let cells = row_cells(pane, y);
        let x = usize::from(x);
        if clicks >= 3 {
            self.select(pane.pane, (0, y), (pane.size.cols.saturating_sub(1), y));
        } else if x < cells.len() && class(cells[x]) != 0 {
            let (start, end) = word_at(&cells, x);
            self.select(pane.pane, (column(start), y), (column(end), y));
        }
    }
}
