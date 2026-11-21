use serde_json::Value;

use super::*;
use crate::cli::Version;
use crate::model::bots::{Ask, in_app};
use crate::model::{MachineSeen, ThereId};
use crate::panels::queue::entries;
use crate::test_util::fixture;

fn full() -> Bot {
    Bot {
        id: "prs".into(),
        name: "PRs".into(),
        agent: "claude".into(),
        model: Some("opus".into()),
        effort: Some("high".into()),
        account: Some("work".into()),
        access: "full".into(),
        secrets: 2,
        asks: vec![Ask { id: 3, names: vec!["a".into(), "b".into()], ..Ask::default() }],
        ..Bot::default()
    }
}

fn read(rows: &[CardRow]) -> Vec<(&'static str, &str, Tone)> {
    rows.iter().map(|row| (row.label, row.value.as_str(), row.tone)).collect()
}

fn labels(bot: &Bot) -> Vec<&'static str> {
    card_rows(bot, None).iter().map(|row| row.label).collect()
}

fn queue() -> Vec<Entry> {
    entries(serde_json::from_str::<Value>(fixture!("queue/bots.json")).ok().as_ref())
}

#[test]
fn a_bot_with_everything_set_shows_every_row_in_the_cards_order() {
    let rows = card_rows(&full(), Some("2026-11-14".into()));
    assert_eq!(
        read(&rows),
        [
            ("Agent", "claude", Tone::Plain),
            ("Model", "opus", Tone::Plain),
            ("Effort", "high", Tone::Plain),
            ("Account", "work", Tone::Plain),
            ("Access", "full access", Tone::Danger),
            ("Secrets", "2 secrets", Tone::Plain),
            ("Asks", "for a secret: a, b", Tone::Waiting),
            ("Made", "2026-11-14", Tone::Plain),
        ]
    );
}

#[test]
fn a_bot_with_no_model_shows_the_agents_own() {
    let bot = Bot { model: None, ..full() };
    assert_eq!(card_rows(&bot, None)[1].value, "the agent's own");
    let empty = Bot { model: Some(String::new()), ..full() };
    assert_eq!(card_rows(&empty, None)[1].value, "the agent's own");
}

#[test]
fn rows_that_are_not_set_are_left_out() {
    let bot = Bot { agent: "claude".into(), effort: Some(String::new()), ..Bot::default() };
    assert_eq!(labels(&bot), ["Agent", "Model"]);
}

#[test]
fn the_access_is_said_in_words_and_full_access_is_the_danger_one() {
    let through = Bot { access: "cornercase".into(), ..Bot::default() };
    let unknown = Bot { access: "sandbox".into(), ..Bot::default() };
    let access =
        |bot: &Bot| card_rows(bot, None).into_iter().find(|row| row.label == "Access").map(|r| (r.value, r.tone));
    assert_eq!(access(&full()), Some(("full access".to_string(), Tone::Danger)));
    assert_eq!(access(&through), Some(("through cornercase".to_string(), Tone::Plain)));
    assert_eq!(access(&unknown), Some(("sandbox".to_string(), Tone::Plain)));
    assert_eq!(access(&Bot::default()), None);
}

#[test]
fn the_secrets_are_counted_and_unreadable_ones_are_the_danger_tone() {
    assert_eq!(secrets_words(0, false), None);
    assert_eq!(secrets_words(1, false), Some(("1 secret".to_string(), false)));
    assert_eq!(secrets_words(2, false), Some(("2 secrets".to_string(), false)));
    assert_eq!(secrets_words(3, true), Some(("secrets cannot be read".to_string(), true)));
    let unreadable = Bot { secrets_unreadable: true, ..full() };
    let row = card_rows(&unreadable, None).into_iter().find(|row| row.label == "Secrets").expect("a secrets row");
    assert_eq!((row.value.as_str(), row.tone), ("secrets cannot be read", Tone::Danger));
    assert!(!labels(&Bot { secrets: 0, ..full() }).contains(&"Secrets"));
}

#[test]
fn each_waiting_ask_has_a_row_of_its_own() {
    let list = in_app(&serde_json::from_str(fixture!("bots/app-asks.json")).expect("the fixture"));
    let asks: Vec<String> =
        card_rows(&list[0], None).into_iter().filter(|row| row.label == "Asks").map(|row| row.value).collect();
    assert_eq!(asks, ["for a secret: campus-user, campus-password", "for a secret: campus-token"]);
}

#[test]
fn the_day_it_was_made_is_left_out_without_one() {
    assert!(!labels(&full()).contains(&"Made"));
    assert_eq!(card_rows(&full(), Some("2026-11-14".into())).last().map(|row| row.label), Some("Made"));
}

#[test]
fn a_bot_reads_the_day_it_was_made() {
    let list = in_app(&serde_json::from_str(fixture!("bots/app-routines.json")).expect("the fixture"));
    assert_eq!(list[0].created, Some(1_795_268_098));
}

#[test]
fn the_routines_are_the_bots_repeating_entries_in_the_queues_order() {
    let rows = routine_rows(&queue(), "prs", |at| format!("at {at}"));
    let read: Vec<(u64, &str, &str)> =
        rows.iter().map(|row| (row.id, row.name.as_str(), row.detail.as_str())).collect();
    assert_eq!(
        read,
        [
            (2, "Review PRs", "next at 1795269905 · ↻ every 30 minutes"),
            (3, "Broken check", "next at 1795269907 · ↻ every 30 minutes"),
        ]
    );
    assert_eq!(routine_rows(&queue(), "ops", |_| String::new()), []);
}

#[test]
fn a_paused_routine_says_so_in_place_of_its_next_time() {
    let mut list = queue();
    if let Some(entry) = list.iter_mut().find(|entry| entry.id == 2) {
        entry.paused = true;
    }
    let rows = routine_rows(&list, "prs", |_| "never".into());
    assert_eq!(rows[0].detail, "paused · ↻ every 30 minutes");
}

#[test]
fn what_it_is_for_is_asked_with_the_bot_after_a_double_dash() {
    assert_eq!(about_args("-prs"), ["bot", "about", "--json", "--", "-prs"]);
}

#[test]
fn what_it_is_for_is_read_from_the_answer() {
    assert_eq!(
        about_of(r#"{"bot":"prs","about":"Review my PRs.\nDaily."}"#).ok().as_deref(),
        Some("Review my PRs.\nDaily.")
    );
    assert_eq!(about_of(r#"{"bot":"prs","about":""}"#).ok().as_deref(), Some(""));
    assert!(about_of(r#"{"bot":"prs","about":3}"#).is_err());
    assert!(about_of(r#"{"bot":"prs"}"#).is_err());
    assert!(about_of("no json").is_err());
    assert_eq!(about_failed("it broke"), "Could not read what it is for: it broke");
}

#[test]
fn what_it_is_for_is_read_hidden_or_says_this_computer_needs_a_newer_cornercase() {
    let old = Version::new(0, 13, 95);
    assert_eq!(about_gate(None, Os::Mac), AboutGate::Read);
    assert_eq!(about_gate(Some(Lacks::Server(Some(old))), Os::Mac), AboutGate::Hidden);
    assert_eq!(about_gate(Some(Lacks::Machine("devbox", Some(old))), Os::Mac), AboutGate::Hidden);
    assert_eq!(
        about_gate(Some(Lacks::ThisComputer(Some(old))), Os::Mac),
        AboutGate::Lacks("Needs cornercase 0.13.96 or newer on this Mac (it has 0.13.95).".into())
    );
}

fn session(text: &str) -> Session {
    serde_json::from_str(text).expect("a session")
}

#[test]
fn a_bots_card_lists_the_agents_it_started_and_not_a_shell_it_tagged() {
    let view = session(fixture!("session/view-bot.json"));
    let ops = started_rows(&view, "ops");
    let found: Vec<(u64, &str, &str, Option<&str>)> = ops
        .iter()
        .map(|started| {
            (started.row.id, started.row.name.as_str(), started.row.detail.as_str(), started.status.as_deref())
        })
        .collect();
    assert_eq!(found, [(12, "bash", "claude · proj", Some("idle"))]);
    assert_eq!(started_rows(&view, "prs"), []);
    assert_eq!(started_rows(&view, "nobody"), []);
}

#[test]
fn a_started_agent_reads_its_title_and_its_workspace_when_the_project_has_several() {
    let view = session(
        r#"{"projects":[{"id":1,"name":"shop","workspaces":[{"id":2,"name":"main","tabs":[]},
        {"id":3,"name":"fix/ci-482","tabs":[{"id":4,"name":"claude","panes":[
        {"id":5,"agent":"claude","bot":"prs","title":"✳ Fix the flaky test","status":"working"}]}]}]}]}"#,
    );
    let rows = started_rows(&view, "prs");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (rows[0].row.name.as_str(), rows[0].row.detail.as_str()),
        ("Fix the flaky test", "claude · shop › fix/ci-482")
    );
}

fn vps() -> MachineSeen {
    let view = session(fixture!("session/view-machine-bot.json"));
    view.machines.into_iter().next().expect("the machine vps")
}

#[test]
fn a_machines_agents_are_those_tagged_with_the_bot_that_run_an_agent() {
    let rows = started_rows_there(&vps(), "notes");
    let found: Vec<(u64, &str, &str, Option<&str>)> = rows
        .iter()
        .map(|started| {
            (started.row.id, started.row.name.as_str(), started.row.detail.as_str(), started.status.as_deref())
        })
        .collect();
    assert_eq!(found, [(0, "bash", "claude · there-proj", Some("idle"))]);
    assert_eq!(started_rows_there(&vps(), "nobody"), []);
}

#[test]
fn a_machines_agent_shows_its_own_tab() {
    let seen = vps();
    let rows = started_rows_there(&seen, "notes");
    let ShowAt::There(place) = &rows[0].show else { panic!("a place on the machine, not {:?}", rows[0].show) };
    assert_eq!((place.machine.as_str(), place.boot.as_str()), ("vps", "b-there"));
    assert_eq!(place.tab, Some(ThereId::heard(10)));
    assert_eq!(place.workspace, ThereId::heard(3));
}

#[test]
fn a_machines_stop_is_named_apart_from_a_pane_here() {
    let there = started_rows_there(&vps(), "notes");
    let here = started_rows(&session(fixture!("session/view-bot.json")), "ops");
    assert_eq!((there[0].stop.as_str(), here[0].stop.as_str()), ("there-9", "12"));
    assert_eq!(here[0].show, ShowAt::Pane(12));
}

#[test]
fn this_computers_bot_keeps_routines_and_edit() {
    assert_eq!(controls(&Home::Here, true), Controls { routines: true, edit: true, note: None });
    assert!(!controls(&Home::Here, false).routines);
}

#[test]
fn a_machines_bot_is_read_only_and_says_where_to_change_it() {
    let there = controls(&Home::Machine("vps".into()), true);
    assert!(!there.routines && !there.edit);
    assert_eq!(
        there.note.as_deref(),
        Some("Change it, its secrets and its routines on vps: this window only reads a machine's bot for now.")
    );
}

#[test]
fn a_machines_folder_is_shown_as_it_comes() {
    let path = "/Users/me/.cornercase/bots/notes";
    assert_eq!(folder_shown(path, &Home::Here, "/Users/me"), "~/.cornercase/bots/notes");
    assert_eq!(folder_shown(path, &Home::Machine("vps".into()), "/Users/me"), path);
}
