use super::entries::{Block, Entry};
use crate::model::bots::{Kind, Line, Runs};

pub(crate) const NO_TITLE: &str = "↻ a routine";
const ENDS: [&str; 2] = ["stopped", "interrupted by a restart"];

fn title_of(first: Option<&&Line>) -> String {
    let named = |line: &&&Line| line.kind == Kind::Notice && line.text.starts_with("↻ ") && line.text.contains(" · ");
    first.filter(named).map_or_else(|| NO_TITLE.to_string(), |line| line.text.clone())
}

pub(super) fn block(run: u64, members: &[&Line]) -> Entry {
    let of = |kind: Kind| members.iter().find(move |line| line.kind == kind);
    let end = members.iter().find(|line| line.kind == Kind::Notice && ENDS.contains(&line.text.as_str()));
    Entry::Run(Block {
        run,
        head: members.first().map_or(0, |line| line.id),
        tail: members.last().map_or(0, |line| line.id),
        lines: members.len(),
        title: title_of(members.first()),
        steps: members.iter().filter(|line| line.kind == Kind::Step).map(|line| line.text.clone()).collect(),
        answer: of(Kind::Bot).map(|line| (line.id, line.text.clone(), line.at)),
        error: of(Kind::Error).map(|line| line.text.clone()),
        end: end.map(|line| line.text.clone()),
        others: members
            .iter()
            .filter(|line| matches!(line.kind, Kind::Other(_) | Kind::Started | Kind::Finished | Kind::Waiting))
            .map(|line| line.text.clone())
            .collect(),
    })
}

pub(crate) fn offers_again(block: &Block, runs: &Runs) -> bool {
    block.error.is_some() && runs.again(block.run).is_some()
}
