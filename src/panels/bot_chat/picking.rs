use std::borrow::Cow;
use std::collections::BTreeMap;
use std::rc::Rc;

use gpui::Context;

use super::entries::Entry;
use super::rows::blocks_of;
use super::{BotChat, Row};
use crate::store::Clip;
use crate::ui::markdown::Block;
use crate::ui::selection::{Edge, Selecting, Selection, Selects};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Spot {
    pub entry: u64,
    pub part: u32,
}

pub(crate) const ERROR_PART: u32 = u32::MAX;

pub(crate) type BlocksOf<'a> = &'a dyn Fn(u64, &str) -> Rc<Vec<Block>>;

fn spot(entry: u64, part: u32) -> Spot {
    Spot { entry, part }
}

fn answer_parts(entry: u64, (id, text): (u64, &str), blocks: BlocksOf<'_>) -> Vec<(Spot, String)> {
    let parsed = blocks(id, text);
    parsed
        .iter()
        .enumerate()
        .filter_map(|(at, block)| {
            let part = u32::try_from(at).ok()?;
            Some((spot(entry, part), block.picked_text()?.into_owned()))
        })
        .collect()
}

pub(crate) fn parts(entry: &Entry, blocks: BlocksOf<'_>) -> Vec<(Spot, String)> {
    match entry {
        Entry::You { id, text, .. } | Entry::Failed { id, text } => vec![(spot(*id, 0), text.clone())],
        Entry::Answer { id, text, .. } => answer_parts(*id, (*id, text), blocks),
        Entry::Run(block) => {
            let mut found = block
                .answer
                .as_ref()
                .map(|(id, text, _)| answer_parts(block.head, (*id, text), blocks))
                .unwrap_or_default();
            found.extend(block.error.as_ref().map(|error| (spot(block.head, ERROR_PART), error.clone())));
            found
        }
        Entry::Steps { .. } | Entry::Notice { .. } | Entry::Other { .. } => Vec::new(),
    }
}

pub(crate) fn selected_text<'a>(
    entries: impl Iterator<Item = &'a Entry>,
    selection: Selection<Spot>,
    blocks: BlocksOf<'_>,
) -> String {
    let (first, last) = selection.ends();
    let within = entries.filter(|entry| (first.entry..=last.entry).contains(&entry.id()));
    let found: BTreeMap<Spot, String> = within.flat_map(|entry| parts(entry, blocks)).collect();
    selection.text(found.iter().map(|(spot, text)| (*spot, text.as_str())), |spot, range| {
        found.get(&spot).and_then(|text| text.get(range)).unwrap_or_default()
    })
}

pub(crate) fn text_at<'a>(
    mut entries: impl Iterator<Item = &'a Entry>,
    at: Spot,
    blocks: BlocksOf<'_>,
) -> Option<String> {
    let entry = entries.find(|entry| entry.id() == at.entry)?;
    parts(entry, blocks).into_iter().find(|(spot, _)| *spot == at).map(|(_, text)| text)
}

fn entries_of(rows: &[Row]) -> impl Iterator<Item = &Entry> {
    rows.iter().filter_map(|row| match row {
        Row::Entry(entry) => Some(entry),
        Row::Earlier => None,
    })
}

impl Selects for BotChat {
    type Spot = Spot;
    const PROSE: bool = true;

    fn selecting(&mut self) -> Option<&mut Selecting<Spot>> {
        Some(&mut self.selecting)
    }

    fn scroll_a_row(&mut self, edge: Edge) {
        self.scroll_by(edge.a_row());
    }

    fn line_text(&self, _: Spot) -> Option<&str> {
        None
    }

    fn word_text(&self, spot: Spot) -> Option<Cow<'_, str>> {
        let blocks = |id: u64, text: &str| blocks_of(id, text, &self.blocks);
        text_at(entries_of(&self.rows), spot, &blocks).map(Cow::Owned)
    }

    fn selected_text(&self) -> Option<String> {
        let selection = self.selecting.selection.filter(|selection| !selection.is_empty())?;
        let blocks = |id: u64, text: &str| blocks_of(id, text, &self.blocks);
        Some(selected_text(entries_of(&self.rows), selection, &blocks))
    }
}

impl BotChat {
    pub(super) fn copy_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(text) = self.selected_text() else { return false };
        self.store.update(cx, |store, cx| store.copy(&Clip::of(text, "the selection"), cx));
        true
    }
}
