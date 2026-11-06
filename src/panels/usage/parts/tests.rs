use super::super::data::{Limit, MachineUsage, parse_utc};
use super::*;
use crate::test_util::fixture;

const HERE: &str = "this computer";
const IDENTITY: &str = "fb7d2488e2805f6ffb5c66df3d51c0a7";
const READ: &str = "2026-11-06T03:08:16.947Z";
const REFUSED: &str = "ssh: connect to host vps port 22: Connection refused";
const DOWN: &str = "ssh: connect to host down port 22: Connection refused";

fn read() -> i64 {
    parse_utc(READ).expect("a time")
}

fn answer(json: &str, now: i64) -> Vec<Part> {
    parts(&serde_json::from_str(json).expect("usage parses"), HERE, now)
}

fn all(now: i64) -> Vec<Part> {
    answer(fixture!("usage/limits_all.json"), now)
}

fn titles(part: &Part) -> Vec<String> {
    part.rows.iter().map(|row| row.plan.title.clone()).collect()
}

fn names(parts: &[Part]) -> Vec<Option<&str>> {
    parts.iter().map(|part| part.name.as_deref()).collect()
}

fn claude(identity: Option<&str>, percent: f64, age: u64) -> Plan {
    Plan {
        agent: "claude".into(),
        title: "Claude Code".into(),
        age: Some(age),
        limits: vec![Limit { label: "week".into(), percent, ..Limit::default() }],
        identity: identity.map(str::to_string),
        ..Plan::default()
    }
}

fn answered_now(name: &str, agents: Vec<Plan>) -> MachineUsage {
    let machine = MachineRead {
        name: name.into(),
        state: LinkState::Linked,
        read_at: Some(READ.to_string()),
        ..MachineRead::default()
    };
    MachineUsage { machine, agents }
}

fn here_read_at(plan: Plan) -> Plan {
    let second_of_the_day = 3 * 3_600 + 8 * 60 + 16 - plan.age.unwrap_or_default();
    let (hour, minute, second) = (second_of_the_day / 3_600, second_of_the_day / 60 % 60, second_of_the_day % 60);
    Plan { read_at: Some(format!("2026-11-06T{hour:02}:{minute:02}:{second:02}.000Z")), ..plan }
}

fn every(here: Vec<Plan>, there: Vec<Plan>) -> Vec<Part> {
    let here = here.into_iter().map(here_read_at).collect();
    let plans = Plans { agents: here, machines: vec![answered_now("vps", there)], ..Plans::default() };
    parts(&plans, HERE, read())
}

fn on(parts: &[Part]) -> Vec<Vec<Vec<&str>>> {
    parts.iter().map(|part| part.rows.iter().map(|row| row.on.iter().map(String::as_str).collect()).collect()).collect()
}

#[test]
fn this_computer_alone_has_no_heading() {
    let mut plans: Plans = serde_json::from_str(fixture!("usage/limits_all_cached.json")).expect("usage parses");
    plans.machines.clear();
    let parts = parts(&plans, HERE, read());
    assert_eq!(names(&parts), [None]);
    assert_eq!(titles(&parts[0]), ["Claude Code", "Codex"]);
    assert_eq!(parts[0].rows[0].on_line(), None);
}

#[test]
fn one_machine_picked_is_headed_by_its_name() {
    let parts = answer(fixture!("usage/limits_one.json"), read());
    assert_eq!(names(&parts), [Some("vps")]);
    assert_eq!(titles(&parts[0]), ["Claude Code", "Codex"]);
    assert_eq!((parts[0].status.as_str(), parts[0].why.as_deref()), ("", None));
}

#[test]
fn all_has_a_part_per_machine_with_this_computer_first() {
    assert_eq!(names(&all(read())), [Some(HERE), Some("vps"), Some("down")]);
}

#[test]
fn each_part_lists_the_accounts_of_its_machine() {
    let parts = every(vec![claude(None, 40., 0)], vec![claude(None, 55., 0)]);
    assert_eq!(
        (titles(&parts[0]), titles(&parts[1])),
        (vec!["Claude Code".to_string()], vec!["Claude Code".to_string()])
    );
    assert!((parts[1].rows[0].plan.limits[0].percent - 55.).abs() < 1e-9);
}

#[test]
fn rows_with_one_identity_become_one_under_the_first_that_gave_it() {
    let parts = all(read());
    assert_eq!(parts[0].rows[0].plan.identity.as_deref(), Some(IDENTITY));
    assert_eq!(titles(&parts[0]), ["Claude Code", "Codex"]);
    assert_eq!(parts[0].rows[0].on_line().as_deref(), Some("on this computer, vps"));
    assert_eq!((titles(&parts[1]), parts[1].joined), (vec!["Codex".to_string()], 1));
}

#[test]
fn rows_without_an_identity_never_merge() {
    let parts = all(read());
    assert_eq!(parts[0].rows[1].on_line(), None);
    assert_eq!(parts[1].rows[0].on_line(), None);
    assert_eq!(on(&every(vec![claude(None, 40., 0)], vec![claude(None, 55., 0)])), [[["this computer"]], [["vps"]]]);
}

#[test]
fn a_row_with_an_identity_stays_apart_from_one_without() {
    let one_side = every(vec![claude(Some("a"), 40., 0)], vec![claude(None, 55., 0)]);
    let other_side = every(vec![claude(None, 40., 0)], vec![claude(Some("a"), 55., 0)]);
    assert_eq!(on(&one_side), [[["this computer"]], [["vps"]]]);
    assert_eq!(on(&other_side), [[["this computer"]], [["vps"]]]);
}

#[test]
fn rows_with_different_identities_stay_apart() {
    let parts = every(vec![claude(Some("a"), 40., 0)], vec![claude(Some("b"), 55., 0)]);
    assert_eq!(on(&parts), [[["this computer"]], [["vps"]]]);
}

#[test]
fn two_rows_of_one_machine_never_merge() {
    let twice = vec![claude(Some("a"), 40., 0), claude(Some("a"), 55., 0)];
    let plans = Plans { machine: Some(answered_now("vps", Vec::new()).machine), agents: twice, ..Plans::default() };
    assert_eq!(on(&parts(&plans, HERE, read())), [[["vps"], ["vps"]]]);
}

#[test]
fn two_rows_of_one_machine_stay_two_under_all() {
    let parts = every(Vec::new(), vec![claude(Some("a"), 40., 0), claude(Some("a"), 55., 0)]);
    assert_eq!(on(&parts), [Vec::new(), vec![vec!["vps"], vec!["vps"]]]);
}

#[test]
fn each_of_two_rows_of_one_machine_joins_its_own_row_of_another() {
    let parts = every(vec![claude(Some("a"), 10., 0)], vec![claude(Some("a"), 40., 0), claude(Some("a"), 55., 0)]);
    assert_eq!(on(&parts), [vec![vec!["this computer", "vps"]], vec![vec!["vps"]]]);
}

#[test]
fn a_merged_row_keeps_the_numbers_of_the_latest_reading_without_adding_them() {
    let parts = every(vec![claude(Some("a"), 40., 300)], vec![claude(Some("a"), 55., 60)]);
    let row = &parts[0].rows[0];
    assert!((row.plan.limits[0].percent - 55.).abs() < 1e-9);
    assert_eq!(row.status(read()), "updated 1m ago");
}

#[test]
fn a_merged_row_does_not_take_older_numbers() {
    let parts = every(vec![claude(Some("a"), 40., 60)], vec![claude(Some("a"), 55., 300)]);
    assert!((parts[0].rows[0].plan.limits[0].percent - 40.).abs() < 1e-9);
}

#[test]
fn a_merged_row_keeps_the_label_and_mark_of_the_first_machine() {
    let labelled = Plan { label: Some("work".into()), active: true, ..claude(Some("a"), 40., 300) };
    let other = Plan { label: Some("home".into()), ..claude(Some("a"), 55., 60) };
    let parts = every(vec![labelled], vec![other]);
    assert_eq!(parts[0].rows[0].plan.heading(), "● Claude Code · work");
    assert!((parts[0].rows[0].plan.limits[0].percent - 55.).abs() < 1e-9);
}

#[test]
fn a_machine_whose_clock_is_off_neither_wins_nor_loses() {
    let future = Plan { read_at: Some("2031-01-01T00:00:00.000Z".into()), ..claude(Some("a"), 55., 300) };
    let parts = every(vec![claude(Some("a"), 40., 60)], vec![future]);
    assert!((parts[0].rows[0].plan.limits[0].percent - 40.).abs() < 1e-9);
    assert_eq!(parts[0].rows[0].status(read()), "updated 1m ago");
}

#[test]
fn a_machine_whose_rows_all_went_up_keeps_its_heading() {
    let parts = every(vec![claude(Some("a"), 40., 0)], vec![claude(Some("a"), 55., 0)]);
    assert_eq!((names(&parts), parts[1].rows.len(), parts[1].joined), (vec![Some(HERE), Some("vps")], 0, 1));
}

#[test]
fn a_rows_age_counts_from_when_this_server_read_it() {
    let parts = answer(fixture!("usage/limits_one_unreachable.json"), read());
    assert_eq!(parts[0].rows[0].status(read() + 300), "updated 5m ago");
}

#[test]
fn a_machine_not_linked_shows_its_last_copy_and_when_it_was_seen() {
    let seen = parse_utc("2026-11-06T03:08:42.000Z").expect("a time");
    let parts = answer(fixture!("usage/limits_all_unreachable.json"), seen + 300);
    let vps = &parts[1];
    assert_eq!((vps.state, vps.status.as_str(), vps.why.as_deref()), (LinkState::Unreachable, "seen 5 min ago", None));
    assert_eq!(titles(vps), ["Codex"]);
    assert_eq!(parts[0].rows[0].on_line().as_deref(), Some("on this computer, vps"));
}

#[test]
fn a_machine_with_no_copy_says_why_instead_of_rows() {
    let parts = all(read());
    let down = &parts[2];
    assert_eq!((down.why.as_deref(), down.unread, down.rows.len()), (Some(DOWN), true, 0));
    assert_eq!(down.status, "not seen yet");
}

#[test]
fn a_linked_machine_whose_last_ask_failed_keeps_its_numbers_under_why() {
    let parts = answer(fixture!("usage/limits_all_failed.json"), read() + 300);
    let vps = &parts[1];
    let why = format!("{REFUSED} · numbers read 5 min ago");
    assert_eq!((vps.why.as_deref(), vps.unread, vps.status.as_str()), (Some(why.as_str()), false, ""));
    assert_eq!(titles(vps), ["Codex"]);
}

#[test]
fn a_linked_machine_not_read_yet_says_so_in_the_cached_answer() {
    let parts = answer(fixture!("usage/limits_all_cached.json"), read());
    let vps = &parts[1];
    assert_eq!((vps.why.as_deref(), vps.unread, vps.state), (Some("not read yet"), true, LinkState::Linked));
}

#[test]
fn an_agent_that_could_not_be_asked_keeps_its_error_on_its_row() {
    let parts = all(read());
    assert_eq!(parts[0].rows[1].plan.error.as_deref(), Some("codex exited without answering"));
}
