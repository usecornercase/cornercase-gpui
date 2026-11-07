use super::*;
use crate::test_util::fixture;

const SEEN: &str = "2026-11-06T03:08:42.000Z";
const READ: &str = "2026-11-06T03:08:17.013Z";
const REFUSED: &str = "ssh: connect to host vps port 22: Connection refused";
const DOWN: &str = "ssh: connect to host down port 22: Connection refused";

fn stats(json: &str) -> Stats {
    serde_json::from_str(json).expect("stats parse")
}

fn plans(json: &str) -> Plans {
    serde_json::from_str(json).expect("usage parses")
}

fn five_minutes_after(stamp: &str) -> i64 {
    parse_utc(stamp).expect("a time") + 300
}

fn link(name: &str) -> Link {
    (name.to_string(), LinkState::Linked)
}

fn vps() -> Pick {
    Pick::Machine("vps".to_string())
}

#[test]
fn without_a_machine_the_row_is_not_offered() {
    assert_eq!(choices(&[], "this computer"), Vec::new());
}

#[test]
fn the_row_offers_this_computer_each_machine_and_all() {
    let offered: Vec<String> =
        choices(&[link("vps"), link("down")], "this computer").into_iter().map(|(label, _)| label).collect();
    assert_eq!(offered, ["this computer", "vps", "down", "all"]);
}

#[test]
fn on_another_server_the_first_choice_is_that_server() {
    assert_eq!(choices(&[link("vps")], "devbox")[0], ("devbox".to_string(), Pick::Here));
}

#[test]
fn a_machine_with_a_problem_has_no_link_to_ask() {
    let wrong = Machine { name: "all".into(), problem: Some("reserved".into()), ..Machine::default() };
    let right = Machine { name: "vps".into(), state: LinkState::Linked, ..Machine::default() };
    assert_eq!(links(&[wrong, right]), [link("vps")]);
}

#[test]
fn the_row_starts_on_every_machine() {
    assert_eq!(Pick::default().shown(&[link("vps")]), Pick::All);
}

#[test]
fn with_no_machine_left_any_pick_shows_this_computer() {
    assert_eq!((vps().shown(&[]), Pick::All.shown(&[])), (Pick::Here, Pick::Here));
}

#[test]
fn a_picked_machine_that_leaves_the_list_shows_all() {
    assert_eq!(vps().shown(&[link("pi")]), Pick::All);
}

#[test]
fn a_picked_machine_still_listed_stays_picked() {
    assert_eq!(vps().shown(&[link("pi"), link("vps")]), vps());
}

#[test]
fn this_computer_asks_as_it_always_did() {
    let here = Pick::Here;
    assert_eq!(here.usage(false), ["usage", "--json"]);
    assert_eq!(here.usage(true), ["usage", "--cached", "--json"]);
    assert_eq!(here.stats(), ["stats", "--json"]);
    assert_eq!(here.share(), ["stats", "--share"]);
}

#[test]
fn a_machine_is_asked_by_its_name() {
    assert_eq!(vps().usage(false), ["usage", "--machine", "vps", "--json"]);
    assert_eq!(vps().usage(true), ["usage", "--cached", "--machine", "vps", "--json"]);
    assert_eq!(vps().stats(), ["stats", "--machine", "vps", "--json"]);
    assert_eq!(vps().share(), ["stats", "--machine", "vps", "--share"]);
}

#[test]
fn all_asks_every_machine() {
    assert_eq!(Pick::All.usage(true), ["usage", "--cached", "--machine", "all", "--json"]);
    assert_eq!(Pick::All.stats(), ["stats", "--machine", "all", "--json"]);
}

#[test]
fn what_is_awaited_names_the_pick() {
    assert_eq!(vps().asking_limits(), "Asking vps for its limits…");
    assert!(vps().asked_on().starts_with("Asked on vps with no prompt"));
}

#[test]
fn this_computer_has_no_cached_stats_to_ask_for_since_its_stats_never_wait() {
    assert_eq!(Pick::Here.cached_stats(), None);
}

#[test]
fn a_machines_last_stats_are_asked_for_by_its_name() {
    assert_eq!(vps().cached_stats().expect("asked"), ["stats", "--cached", "--machine", "vps", "--json"]);
}

#[test]
fn every_machines_last_stats_are_asked_for_with_all() {
    assert_eq!(Pick::All.cached_stats().expect("asked"), ["stats", "--cached", "--machine", "all", "--json"]);
}

#[test]
fn the_sum_is_the_servers_with_the_largest_peak_of_one_machine() {
    let sum = stats(fixture!("usage/stats_all.json")).window("total");
    assert_eq!((sum.counts.agents, sum.counts.prompts, sum.most_at_once), (5, 16, 5));
    assert!((sum.busy_seconds - 3_600.).abs() < 1e-9);
}

#[test]
fn the_sum_says_the_peak_is_not_added_then_what_each_machine_gave() {
    let notes = stats(fixture!("usage/stats_all.json")).notes(five_minutes_after(SEEN));
    assert_eq!(notes, [PEAK.to_string(), "vps: seen 5 min ago".to_string(), format!("down: {NOT_SUMMED} · {DOWN}"),]);
}

#[test]
fn one_machine_picked_has_no_note_of_the_peak() {
    let notes = stats(fixture!("usage/stats_one.json")).notes(five_minutes_after(READ));
    assert_eq!(notes, Vec::<String>::new());
}

#[test]
fn one_machine_whose_last_ask_failed_says_why_its_numbers_are_old() {
    let notes = stats(fixture!("usage/stats_one_failed.json")).notes(five_minutes_after(READ));
    assert_eq!(notes, [format!("vps: {REFUSED} · numbers read 5 min ago")]);
}

#[test]
fn this_computer_alone_has_no_notes() {
    assert_eq!(Stats::default().notes(0), Vec::<String>::new());
}

#[test]
fn a_machine_never_seen_says_so() {
    let machine = MachineRead { name: "pi".into(), state: LinkState::Unreachable, ..MachineRead::default() };
    assert_eq!(machine.seen(0).as_deref(), Some(NOT_SEEN));
    assert_eq!(machine.unread(), Some(NOT_SEEN));
}

#[test]
fn a_linked_machine_read_without_a_failure_is_not_old() {
    assert!(!stats(fixture!("usage/stats_one.json")).old());
    assert!(!plans(fixture!("usage/limits_one.json")).old());
    assert!(!Stats::default().old());
}

#[test]
fn an_answer_with_a_machine_down_or_failing_is_old() {
    assert!(stats(fixture!("usage/stats_all.json")).old());
    assert!(stats(fixture!("usage/stats_one_failed.json")).old());
    assert!(plans(fixture!("usage/limits_all.json")).old());
    assert!(plans(fixture!("usage/limits_one_unreachable.json")).old());
}

#[test]
fn a_cached_sum_leaves_out_a_machine_not_read_yet_and_says_so() {
    let notes = stats(fixture!("usage/stats_all_cached.json")).notes(five_minutes_after(SEEN));
    let vps = format!("vps: {NOT_SUMMED} · not read yet");
    assert_eq!(notes, [PEAK.to_string(), vps, format!("down: {NOT_SUMMED} · {DOWN}")]);
}

#[test]
fn a_cached_sum_with_a_machine_not_read_yet_is_old() {
    assert!(stats(fixture!("usage/stats_all_cached.json")).old());
}

#[test]
fn one_machines_cached_copy_says_when_it_was_seen() {
    let notes = stats(fixture!("usage/stats_one_cached.json")).notes(five_minutes_after("2026-11-07T01:49:00.000Z"));
    assert_eq!(notes, ["vps: seen 5 min ago".to_string()]);
}

#[test]
fn one_machines_cached_copy_keeps_its_numbers() {
    assert_eq!(stats(fixture!("usage/stats_one_cached.json")).window("total").counts.agents, 3);
}
