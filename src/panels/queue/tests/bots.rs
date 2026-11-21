use serde_json::Value;

use super::super::data::Click;
use super::super::data::{
    Entry, Group, Place, ROUTINES_NOTE, State, bot_click, chat_block, entries, offers_detail, place_label, remove_tip,
    routine_kinds, routines_note, scoped, sections, status_label,
};
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
fn a_bots_entry_is_never_editable_and_offers_no_detail() {
    for id in [2, 4, 5, 7] {
        let entry = found(id);
        assert!(!entry.editable() && !offers_detail(&entry), "{id}");
    }
    assert!(found(4).runnable() && found(9).editable());
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
fn the_scope_keeps_only_that_bots_entries_and_all_of_them_with_none() {
    let ids = |list: Vec<Entry>| list.iter().map(|entry| entry.id).collect::<Vec<_>>();
    assert_eq!(ids(scoped(queue(), Some("prs"))), [5, 7, 2, 3, 4]);
    assert_eq!(ids(scoped(queue(), Some("ops"))), Vec::<u64>::new());
    assert_eq!(ids(scoped(queue(), None)), [5, 7, 2, 3, 4, 9]);
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

fn routine_of_project(id: u64) -> Entry {
    Entry { id, state: "pending".into(), repeat: Some("2h".into()), label: format!("entry {id}"), ..Entry::default() }
}

fn note_for(list: &[Entry]) -> &'static str {
    let sections = sections(list, "", None, false);
    let routines = sections.iter().find(|section| section.group == Group::Routines).expect("routines");
    routines_note(routine_kinds(&routines.rows))
}

#[test]
fn the_queues_routines_note_says_what_its_routines_are() {
    let bot_routine = found(2);
    assert_eq!(note_for(&[routine_of_project(1)]), ROUTINES_NOTE);
    let only_bots = note_for(std::slice::from_ref(&bot_routine));
    assert!(only_bots.contains("a bot's routine") && !only_bots.contains("worktree"), "{only_bots}");
    let mixed = note_for(&[routine_of_project(1), bot_routine]);
    assert!(mixed.contains("a project's routine") && mixed.contains("a bot's routine"), "{mixed}");
}
