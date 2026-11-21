use serde_json::Value;

use super::super::data::Click;
use super::super::data::{
    Entry, Group, Place, State, bot_click, chat_block, entries, offers_detail, place_label, remove_tip, scoped,
    sections, status_label,
};
use super::super::draft::{Draft, Target, When};
use super::super::form::{saved_text, start};
use super::draft;
use crate::model::accounts::pick::AccountPick;
use crate::test_util::fixture;

fn queue() -> Vec<Entry> {
    let app: Value = serde_json::from_str(fixture!("queue/bots.json")).expect("the fixture");
    entries(Some(&app))
}

fn found(id: u64) -> Entry {
    queue().into_iter().find(|entry| entry.id == id).expect("the fixture's entry")
}

fn of_bot(id: u64, state: &str, routine: Option<u64>) -> Entry {
    let place = Place { bot: Some("prs".into()), ..Place::default() };
    Entry { id, state: state.into(), routine, place, label: format!("entry {id}"), ..Entry::default() }
}

#[test]
fn a_bots_entries_read_their_bot_and_the_answered_state() {
    let answered = found(5);
    assert_eq!((answered.place.bot.as_deref(), answered.state.as_str()), (Some("prs"), "answered"));
    assert_eq!(State::of(&answered), State::Answered);
    assert_eq!(found(9).place.bot, None);
}

#[test]
fn an_answered_run_falls_under_started_reads_answered_and_counts_as_clearable() {
    let answered = found(5);
    assert_eq!(State::Answered.group(), Group::Started);
    assert_eq!(status_label(&answered, None), "Answered");
    assert!(answered.clearable());
    assert!(of_bot(1, "answered", None).clearable());
}

#[test]
fn a_bots_run_that_answers_reads_answering() {
    assert_eq!(status_label(&of_bot(1, "starting", Some(2)), None), "Answering…");
    assert_eq!(status_label(&Entry { state: "starting".into(), ..Entry::default() }, None), "Starting…");
}

#[test]
fn a_bots_place_reads_bot_and_its_id() {
    assert_eq!(place_label(&found(2).place, None), "bot prs");
    assert_eq!(place_label(&found(9).place, None), "proj");
}

#[test]
fn a_bots_entry_that_can_run_offers_edit_and_no_detail() {
    for id in [2, 4, 5, 7] {
        assert!(!offers_detail(&found(id)), "{id}");
    }
    assert!(found(2).runnable() && found(4).runnable());
    assert!(!found(5).runnable() && !found(7).runnable());
}

#[test]
fn a_routine_whose_newest_run_answered_goes_to_its_block() {
    let list = queue();
    let routine = found(2);
    let runs: Vec<&Entry> = list.iter().filter(|entry| entry.routine == Some(2)).collect();
    assert_eq!(chat_block(&routine, &runs), Some(5));
    assert_eq!(bot_click(&routine, &runs), Some(Click::Chat(5)));
    assert_eq!(chat_block(&found(7), &[]), Some(7));
}

#[test]
fn a_missed_run_a_pending_reminder_and_a_projects_entry_go_to_no_block() {
    let missed = of_bot(11, "missed", Some(2));
    assert_eq!(chat_block(&found(2), &[&missed]), None);
    assert_eq!(bot_click(&found(2), &[&missed]), Some(Click::Nothing));
    assert_eq!(chat_block(&found(4), &[]), None);
    assert_eq!(chat_block(&found(9), &[]), None);
    assert_eq!(bot_click(&found(9), &[]), None);
}

#[test]
fn the_scope_keeps_only_that_bots_entries_and_none_keeps_only_the_projects() {
    let ids = |list: Vec<Entry>| list.iter().map(|entry| entry.id).collect::<Vec<_>>();
    assert_eq!(ids(scoped(queue(), Some("prs"))), [5, 7, 2, 3, 4]);
    assert_eq!(ids(scoped(queue(), Some("ops"))), Vec::<u64>::new());
    assert_eq!(ids(scoped(queue(), None)), [9]);
}

#[test]
fn a_scoped_list_never_keeps_an_empty_scheduled_section() {
    let reminderless = [of_bot(1, "answered", None)];
    let groups = |scope: bool| -> Vec<Group> {
        sections(&reminderless, "", None, scope).iter().map(|section| section.group).collect()
    };
    assert_eq!(groups(true), [Group::Started]);
    assert_eq!(groups(false), [Group::Scheduled, Group::Started]);
}

#[test]
fn removing_a_bots_routine_or_run_says_its_answers_stay_in_the_chat() {
    assert!(remove_tip(&found(2)).contains("what it answered stays in the chat"));
    assert!(remove_tip(&found(5)).contains("its answer stays in the chat"));
    assert_eq!(remove_tip(&found(4)), "Remove");
}

fn for_bot(when: When) -> Draft {
    Draft {
        when,
        clock: "09:00".into(),
        text: "Which of my PRs are blocked?".into(),
        target: Target::Worktree { project: 3, branch: "feat/x".into() },
        agent: Some("codex".into()),
        mode: Some("plan".into()),
        flow: Some("team".into()),
        cleanup: Some(true),
        accounts: vec![AccountPick { agent: "claude".into(), folder: "work".into() }],
        bot: Some("prs".into()),
        ..draft()
    }
}

fn strings(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_string()).collect()
}

#[test]
fn a_new_routine_for_a_bot_sends_the_bot_and_nothing_of_a_project() {
    let draft = Draft { name: Some(" PR review ".into()), ..for_bot(When::Daily) };
    let sent = strings(&[
        "queue",
        "add",
        "--every",
        "day 09:00",
        "--prompt=Which of my PRs are blocked?",
        "--bot",
        "prs",
        "--name",
        "PR review",
        "--json",
    ]);
    assert_eq!(draft.args(), Ok(sent));
}

#[test]
fn a_new_reminder_for_a_bot_sends_at() {
    let draft = Draft { amount: "1".into(), unit: 'h', ..for_bot(When::In) };
    let sent =
        strings(&["queue", "add", "--at", "in 1h", "--prompt=Which of my PRs are blocked?", "--bot", "prs", "--json"]);
    assert_eq!(draft.args(), Ok(sent));
}

#[test]
fn an_edit_of_a_bots_entry_sends_only_what_changed_and_never_the_bot() {
    let untouched = Draft { edit: Some(2), text: String::new(), name: None, ..for_bot(When::Keep) };
    assert_eq!(untouched.args(), Ok(strings(&["queue", "edit", "2", "--json"])));
    let renamed = Draft { name: Some("Morning PRs".into()), ..untouched.clone() };
    assert_eq!(renamed.args(), Ok(strings(&["queue", "edit", "2", "--name", "Morning PRs", "--json"])));
    let moved = Draft { text: "Only the red ones".into(), ..Draft { when: When::Weekdays, ..untouched } };
    let sent = strings(&["queue", "edit", "2", "--every", "weekday 09:00", "--prompt=Only the red ones", "--json"]);
    assert_eq!(moved.args(), Ok(sent));
}

#[test]
fn a_bots_entry_without_a_prompt_is_refused_before_sending() {
    let empty = Draft { text: "  ".into(), ..for_bot(When::Daily) };
    assert_eq!(empty.args(), Err("write what it should do".to_string()));
}

#[test]
fn a_new_bot_form_starts_on_every_day_at_nine() {
    let (when, times) = start(None, true);
    assert_eq!((when, times.clock.as_str()), (When::Daily, "09:00"));
    assert_eq!(start(None, false).0, When::In);
    assert_eq!(start(Some(&found(4)), true).0, When::Keep);
}

#[test]
fn a_bots_reminder_is_told_as_a_reminder() {
    assert_eq!(saved_text(false, false, true), "Reminder scheduled");
    assert_eq!(saved_text(true, false, true), "Reminder saved");
    assert_eq!(saved_text(false, true, true), "Routine scheduled");
    assert_eq!(saved_text(false, false, false), "Scheduled");
}
