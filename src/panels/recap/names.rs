use std::collections::BTreeMap;
use std::ops::Range;

use super::data::Place;
use crate::panels::issues::markdown::{self, Block, Inline, Mark};

type Places = BTreeMap<String, Place>;

pub(super) fn blocks(text: &str, places: &Places) -> Vec<Block> {
    let mut blocks = markdown::parse(&apart(text, places));
    for block in &mut blocks {
        match block {
            Block::Heading(_, text) | Block::Paragraph(text) | Block::Item(_, _, text) | Block::Quote(text) => {
                linked(text, places);
            }
            Block::Code(code) => *code = named(code, places),
            Block::Table(rows) => rows.iter_mut().flatten().for_each(|cell| *cell = named(cell, places)),
            Block::Rule => {}
        }
    }
    blocks
}

pub(super) fn label(id: &str, place: &Place) -> String {
    let name = match (place.is_pull(), place.number) {
        (true, Some(number)) => format!("#{number}"),
        _ => place.name(),
    };
    if name.is_empty() { id.to_string() } else { name }
}

fn found<'a>(text: &'a str, places: &'a Places) -> Vec<(Range<usize>, &'a str, String)> {
    text.match_indices('[')
        .filter_map(|(open, _)| {
            let inner = &text[open + 1..];
            let id = &inner[..inner.find(']')?];
            places.get(id).map(|place| (open..open + id.len() + 2, id, label(id, place)))
        })
        .collect()
}

fn apart(text: &str, places: &Places) -> String {
    let mut out = text.to_string();
    for (at, _, _) in found(text, places).into_iter().rev() {
        if text[at.end..].starts_with(['(', '[']) {
            out.insert(at.end, ' ');
        }
    }
    out
}

fn named(text: &str, places: &Places) -> String {
    let mut out = text.to_string();
    for (at, _, name) in found(text, places).into_iter().rev() {
        out.replace_range(at, &name);
    }
    out
}

fn linked(text: &mut Inline, places: &Places) {
    let source = text.text.clone();
    for (at, id, name) in found(&source, places).into_iter().rev() {
        link(text, at, &name, id);
    }
}

fn link(text: &mut Inline, at: Range<usize>, name: &str, target: &str) {
    let moved = |x: usize| if x >= at.end { x - at.len() + name.len() } else { x };
    let ranges = text.marks.iter_mut().map(|(range, _)| range).chain(text.links.iter_mut().map(|(range, _)| range));
    for range in ranges {
        *range = moved(range.start)..moved(range.end);
    }
    text.text.replace_range(at.clone(), name);
    let shown = at.start..at.start + name.len();
    text.marks.push((shown.clone(), Mark::Link));
    text.links.push((shown, target.to_string()));
}

#[cfg(test)]
mod tests;
