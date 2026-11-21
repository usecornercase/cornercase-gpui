use std::collections::{HashMap, HashSet};

use super::super::agents::{AgentLine, Link, State, Stop, folds, link, linked, stops};
use super::super::entries::{Again, Entry, Give, Turn, entries, held, keys, retry, turn};
use crate::model::bots::{AgentRef, Home, Line, Runs, history_of};
use crate::test_util::fixture;

fn line(id: u64, kind: &str, text: &str) -> Line {
    super::runs::line(id, kind, text, None)
}

fn agent_line(id: u64, kind: &str, text: &str, body: Option<&str>) -> Line {
    let agent = Some(AgentRef { pane: id + 100, boot: "b-1".into() });
    Line { agent, body: body.map(str::to_string), ..line(id, kind, text) }
}

fn agents(entries: &[Entry]) -> Vec<&AgentLine> {
    entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::Agent(line) => Some(line),
            _ => None,
        })
        .collect()
}

fn all_shown(lines: &[Line]) -> HashSet<u64> {
    lines.iter().map(|line| line.id).collect()
}

#[test]
fn each_new_kind_is_one_entry_of_its_own() {
    let lines = vec![
        line(1, "you", "go"),
        line(2, "step", "a"),
        agent_line(3, "started", "▶ started claude in demo", None),
        line(4, "step", "b"),
        agent_line(5, "finished", "✓ claude finished in demo", Some("done")),
        agent_line(6, "finished", "✗ claude failed in demo", Some("broke")),
        agent_line(7, "waiting", "! claude needs you in demo", Some("Run it?")),
    ];
    let shown = entries(&lines);
    assert_eq!(shown.len(), 7);
    assert!(matches!(&shown[1], Entry::Steps { texts, .. } if texts.len() == 1));
    assert!(matches!(&shown[3], Entry::Steps { texts, .. } if texts.len() == 1));
    let states: Vec<(State, &str)> = agents(&shown).iter().map(|line| (line.state, line.words.as_str())).collect();
    assert_eq!(
        states,
        [
            (State::Started, "started claude in demo"),
            (State::Finished, "claude finished in demo"),
            (State::Failed, "claude failed in demo"),
            (State::Waiting, "claude needs you in demo"),
        ]
    );
}

#[test]
fn the_fixtures_lines_are_drawn_as_agent_rows() {
    let lines = history_of(fixture!("bots/history-follow.json")).expect("the fixture");
    let shown = entries(&lines);
    let kinds: Vec<State> = agents(&shown).iter().map(|line| line.state).collect();
    let want = [
        State::Started,
        State::Finished,
        State::Started,
        State::Waiting,
        State::Started,
        State::Failed,
        State::Started,
    ];
    assert_eq!(kinds, want);
}

#[test]
fn a_link_shows_only_for_a_line_the_store_calls_live() {
    let lines = vec![agent_line(1, "started", "▶ a", None), agent_line(2, "started", "▶ b", None)];
    let shown = linked(entries(&lines), &HashSet::from([2]));
    let panes: Vec<Option<u64>> = agents(&shown).iter().map(|line| line.pane).collect();
    assert_eq!(panes, [None, Some(102)]);
}

#[test]
fn a_machines_bot_says_where_its_agent_is() {
    assert_eq!(link(Some(7), &Home::Machine("vps".into())), Link::There("vps".into()));
    assert_eq!(link(None, &Home::Machine("vps".into())), Link::There("vps".into()));
    assert_eq!(link(Some(7), &Home::Here), Link::Show(7));
}

#[test]
fn a_gone_pane_reads_closed() {
    assert_eq!(link(None, &Home::Here), Link::Closed);
}

#[test]
fn only_a_finished_line_with_a_body_folds() {
    let lines = vec![
        agent_line(1, "finished", "✓ a", Some("text")),
        agent_line(2, "finished", "✗ b", Some("text")),
        agent_line(3, "finished", "✓ c", Some("  \n ")),
        agent_line(4, "finished", "✓ d", None),
        agent_line(5, "waiting", "! e", Some("text")),
        agent_line(6, "started", "▶ f", Some("text")),
    ];
    let folding: Vec<bool> = agents(&entries(&lines)).into_iter().map(folds).collect();
    assert_eq!(folding, [true, true, false, false, false, false]);
}

#[test]
fn the_stops_are_show_then_fold_in_the_lines_order() {
    let lines = vec![
        agent_line(1, "started", "▶ a", None),
        line(2, "bot", "ok"),
        agent_line(3, "finished", "✓ b", Some("text")),
        agent_line(4, "waiting", "! c", Some("?")),
    ];
    let shown = linked(entries(&lines), &all_shown(&lines));
    assert_eq!(stops(&shown), [Stop::Show(1), Stop::Show(3), Stop::Fold(3), Stop::Show(4)]);
}

#[test]
fn a_closed_line_without_a_body_has_no_stop() {
    let lines = vec![agent_line(1, "started", "▶ a", None), agent_line(2, "finished", "✓ b", Some("text"))];
    let shown = linked(entries(&lines), &HashSet::new());
    assert_eq!(stops(&shown), [Stop::Fold(2)]);
}

fn report_turn(last: Line) -> Vec<Line> {
    vec![
        line(1, "you", "go"),
        line(2, "bot", "started it"),
        agent_line(3, "finished", "✓ claude finished in demo", Some("done")),
        line(4, "step", "a"),
        last,
    ]
}

#[test]
fn a_finished_line_times_the_turn_it_opens() {
    let lines = report_turn(line(5, "step", "b"));
    assert_eq!(turn(&lines, true), Some(Turn { since: 1003, steps: 2 }));
}

#[test]
fn a_failed_report_turn_offers_no_retry() {
    let lines = report_turn(line(5, "error", "claude exited without answering"));
    assert_eq!(retry(&lines, false, true), None);
}

#[test]
fn a_message_held_during_a_report_turn_is_retried_as_before() {
    let lines = vec![
        agent_line(1, "finished", "✓ claude finished in demo", Some("done")),
        Line { held: true, ..line(2, "you", "and the other one?") },
        line(3, "bot", "reported"),
        line(4, "step", "a"),
        line(5, "error", "boom"),
    ];
    let again = retry(&lines, false, true).map(|retry| retry.again);
    assert_eq!(again, Some(Again::Say("and the other one?".into())));
}

#[test]
fn a_started_line_ends_no_turn_and_holds_nothing() {
    let lines = vec![
        line(1, "you", "go"),
        agent_line(2, "started", "▶ started claude in demo", None),
        agent_line(3, "waiting", "! claude needs you in demo", Some("?")),
        Line { held: true, ..line(4, "you", "later") },
    ];
    assert_eq!(held(&lines), HashSet::from([4]));
    assert_eq!(turn(&lines, true), Some(Turn { since: 1001, steps: 0 }));
}

fn key_of(entries: &[Entry], opened: &HashSet<u64>) -> Vec<super::super::entries::EntryKey> {
    keys(entries, opened, None, (&HashMap::new(), Give::No), &Runs::default())
}

#[test]
fn a_link_that_goes_measures_its_row_again() {
    let lines = vec![line(1, "you", "go"), agent_line(2, "started", "▶ a", None)];
    let live = key_of(&linked(entries(&lines), &all_shown(&lines)), &HashSet::new());
    let gone = key_of(&linked(entries(&lines), &HashSet::new()), &HashSet::new());
    assert_eq!(live[0], gone[0]);
    assert_ne!(live[1], gone[1]);
}

#[test]
fn a_fold_measures_its_row_again() {
    let lines = vec![agent_line(1, "finished", "✓ a", Some("text"))];
    let shown = entries(&lines);
    assert_ne!(key_of(&shown, &HashSet::new()), key_of(&shown, &HashSet::from([1])));
}

#[test]
fn a_runs_block_keeps_a_line_of_a_new_kind() {
    let lines = vec![Line { routine: Some(9), ..agent_line(1, "started", "▶ started claude in demo", None) }];
    let shown = entries(&lines);
    assert!(matches!(&shown[0], Entry::Run(block) if block.others == ["▶ started claude in demo"]));
}
