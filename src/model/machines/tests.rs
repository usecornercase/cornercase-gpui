use serde_json::json;

use super::*;
use crate::test_util::fixture;

const SEEN: &str = "2026-11-06T01:23:55.000Z";

fn now() -> i64 {
    epoch(SEEN).expect("a time") + 120
}

fn machine(state: LinkState) -> Machine {
    Machine { name: "vps".into(), state, ..Machine::default() }
}

fn seen_two_minutes_ago() -> Machine {
    Machine { seen: Some(SEEN.to_string()), ..machine(LinkState::Unreachable) }
}

fn never_seen_trying_at(seconds_from_now: i64) -> Machine {
    let retry_at = format!("2026-11-06T01:25:{:02}.139Z", 55 + seconds_from_now);
    Machine { retry_at: Some(retry_at), ..machine(LinkState::Unreachable) }
}

#[test]
fn the_list_of_machines_reads_here_and_each_link() {
    let list = parse_list(fixture!("machines/list.json")).expect("parses");
    assert_eq!(list.here, Here { icon: MachineIcon::Laptop, version: "0.13.1".into() });
    assert_eq!(list.machines.len(), 2);
    let vps = &list.machines[0];
    assert_eq!(vps.name, "vps");
    assert_eq!(vps.destination, "ana@vps.example.com");
    assert_eq!(vps.folder, "~/code");
    assert_eq!((vps.icon, vps.state), (MachineIcon::Server, LinkState::Linked));
    let homelab = &list.machines[1];
    assert_eq!((homelab.icon, homelab.state), (MachineIcon::Desktop, LinkState::Unreachable));
    assert_eq!(homelab.error.as_deref(), Some("ssh: connect to host homelab.example.com port 22: Connection refused"));
}

#[test]
fn the_app_state_holds_the_same_machines() {
    let list = parse_list(fixture!("machines/list.json")).expect("parses");
    let answer: Value = serde_json::from_str(fixture!("machines/list.json")).expect("json");
    let app = json!({"revision": 3, "machines": answer["machines"]});
    assert_eq!(in_app(&app), list.machines);
}

#[test]
fn an_app_state_without_machines_lists_none() {
    let app = serde_json::from_str(fixture!("app/snapshot.json")).expect("json");
    assert_eq!(in_app(&app), Vec::new());
}

#[test]
fn an_icon_it_does_not_know_reads_as_the_first() {
    assert_eq!(MachineIcon::from("x".to_string()), MachineIcon::Server);
    assert_eq!(MachineIcon::from("⌂".to_string()), MachineIcon::HomeServer);
}

#[test]
fn each_choice_of_icon_is_drawn_apart() {
    let drawn: std::collections::HashSet<&str> = MachineIcon::ALL.into_iter().map(|icon| icon.icon().path()).collect();
    assert_eq!(drawn.len(), MachineIcon::ALL.len());
}

#[test]
fn a_state_it_does_not_know_reads_as_connecting() {
    let machine: Machine = serde_json::from_value(json!({"name": "vps", "state": "resting"})).expect("parses");
    assert_eq!(machine.state, LinkState::Connecting);
}

#[test]
fn a_linked_machine_says_its_version() {
    let linked = Machine { version: Some("0.13.1".into()), ..machine(LinkState::Linked) };
    assert_eq!(linked.words(now()), "linked · 0.13.1");
}

#[test]
fn a_connecting_machine_says_so() {
    assert_eq!(machine(LinkState::Connecting).words(now()), "connecting…");
}

#[test]
fn an_unreachable_machine_says_when_it_was_seen() {
    assert_eq!(seen_two_minutes_ago().words(now()), "unreachable · seen 2 min ago");
}

#[test]
fn an_unreachable_machine_never_seen_says_so() {
    assert_eq!(machine(LinkState::Unreachable).words(now()), "unreachable · not seen yet");
}

#[test]
fn how_long_ago_grows_from_seconds_to_days() {
    let cases = [
        (8, "8 s"),
        (59, "59 s"),
        (60, "1 min"),
        (3_599, "59 min"),
        (3_600, "1 h"),
        (86_399, "23 h"),
        (86_400, "1 day"),
        (172_800, "2 days"),
    ];
    for (seconds, words) in cases {
        assert_eq!(ago(seconds), words);
    }
}

#[test]
fn a_clock_behind_the_servers_never_reads_as_negative() {
    assert_eq!(ago(-5), "0 s");
}

#[test]
fn an_older_machine_says_the_version_it_runs() {
    let older = Machine { version: Some("0.12.200".into()), ..machine(LinkState::Older) };
    assert_eq!(older.words(now()), "older · 0.12.200");
    assert_eq!(machine(LinkState::Older).words(now()), "older");
}

#[test]
fn only_an_older_machine_that_is_not_updating_can_update() {
    assert!(machine(LinkState::Older).can_update());
    assert!(!Machine { updating: true, ..machine(LinkState::Older) }.can_update());
    assert!(!machine(LinkState::Linked).can_update());
}

#[test]
fn a_machine_updating_says_so_whatever_its_state() {
    assert_eq!(Machine { updating: true, ..machine(LinkState::Older) }.words(now()), "updating…");
}

#[test]
fn a_problem_wins_over_the_state_and_has_no_link() {
    let wrong = Machine { problem: Some("the name is required".into()), ..machine(LinkState::Unreachable) };
    assert_eq!(wrong.words(now()), "problem · no link");
    assert!(!wrong.linked());
    assert!(!wrong.can_retry());
    assert!(!wrong.can_update());
}

#[test]
fn retry_is_for_a_machine_whose_link_is_not_up_or_trying() {
    assert!(machine(LinkState::Unreachable).can_retry());
    assert!(machine(LinkState::Older).can_retry());
    assert!(!machine(LinkState::Linked).can_retry());
    assert!(!machine(LinkState::Connecting).can_retry());
}

#[test]
fn a_machine_whose_server_runs_another_version_says_so() {
    let behind =
        Machine { version: Some("0.13.1".into()), server_version: Some("0.13.0".into()), ..machine(LinkState::Linked) };
    assert_eq!(behind.server_behind().as_deref(), Some("Its server runs 0.13.0 until it restarts there"));
}

#[test]
fn a_machine_whose_server_runs_its_version_says_nothing_more() {
    let same =
        Machine { version: Some("0.13.1".into()), server_version: Some("0.13.1".into()), ..machine(LinkState::Linked) };
    assert_eq!(same.server_behind(), None);
}

#[test]
fn a_machine_connecting_or_updating_is_in_progress() {
    assert!(machine(LinkState::Connecting).in_progress());
    assert!(Machine { updating: true, ..machine(LinkState::Older) }.in_progress());
    assert!(!machine(LinkState::Linked).in_progress());
    assert!(!Machine { problem: Some("taken".into()), ..machine(LinkState::Connecting) }.in_progress());
}

#[test]
fn only_an_unreachable_machine_once_seen_counts_up() {
    assert!(seen_two_minutes_ago().counts_up());
    assert!(!machine(LinkState::Unreachable).counts_up());
    assert!(!Machine { seen: Some(SEEN.to_string()), ..machine(LinkState::Linked) }.counts_up());
}

#[test]
fn a_reason_names_the_machine_and_not_its_address() {
    let reason = "ssh: connect to host vps.example.com port 22: Connection refused";
    assert_eq!(
        hide_destination(reason, "vps", "ana@vps.example.com"),
        "ssh: connect to host vps port 22: Connection refused"
    );
    assert_eq!(hide_destination("ana@vps.example.com: denied.", "vps", "ana@vps.example.com"), "vps: denied.");
}

#[test]
fn a_word_that_only_holds_the_address_stays() {
    let reason = "myvps.example.com is not vps.example.com.au";
    assert_eq!(hide_destination(reason, "vps", "vps.example.com"), reason);
}

#[test]
fn a_link_that_fell_says_when_it_tries_next_and_one_that_is_up_does_not() {
    let list = parse_list(fixture!("machines/fallen.json")).expect("parses");
    assert_eq!(list.machines[0].next_try(now()), Some(NextTry::In(4)));
    assert_eq!(list.machines[1].next_try(now()), None);
}

#[test]
fn the_next_try_counts_down_in_the_words_of_upstreams_window() {
    assert_eq!(never_seen_trying_at(4).next_try(now()).map(NextTry::words).as_deref(), Some("reconnecting in 4 s"));
    assert_eq!(never_seen_trying_at(1).next_try(now()).map(NextTry::words).as_deref(), Some("reconnecting in 1 s"));
}

#[test]
fn a_next_try_far_off_reads_in_minutes() {
    assert_eq!(NextTry::In(90).words(), "reconnecting in 1 min");
}

#[test]
fn a_time_reached_or_passed_means_the_try_is_under_way() {
    for passed in [0, -1, -20] {
        let next = never_seen_trying_at(passed).next_try(now());
        assert_eq!(next, Some(NextTry::UnderWay));
        assert_eq!(next.map(NextTry::words).as_deref(), Some("reconnecting…"));
    }
}

#[test]
fn a_link_with_no_time_or_one_it_cannot_read_promises_no_try() {
    assert_eq!(machine(LinkState::Unreachable).next_try(now()), None);
    assert_eq!(Machine { retry_at: Some("soon".into()), ..machine(LinkState::Unreachable) }.next_try(now()), None);
}

#[test]
fn a_row_changes_by_itself_while_it_counts_down_or_up() {
    assert!(never_seen_trying_at(4).counts(now()));
    assert!(seen_two_minutes_ago().counts(now()));
}

#[test]
fn a_row_with_nothing_to_count_needs_no_timer() {
    assert!(!never_seen_trying_at(0).counts(now()));
    assert!(!machine(LinkState::Unreachable).counts(now()));
    assert!(!machine(LinkState::Linked).counts(now()));
    assert!(!machine(LinkState::Older).counts(now()));
    assert!(!machine(LinkState::Connecting).counts(now()));
}
