use super::{add_note, cleared_too, elsewhere, header_text, where_note};
use crate::cli::{Release, Releases, Version};
use crate::panels::queue::data::{Entry, Place};
use crate::platform::Os;

fn bots_entry(id: u64, bot: &str, (state, routine): (&str, Option<u64>)) -> Entry {
    let place = Place { bot: Some(bot.into()), ..Place::default() };
    Entry { id, state: state.into(), routine, place, ..Entry::default() }
}

fn reminder(id: u64, bot: &str) -> Entry {
    bots_entry(id, bot, ("pending", None))
}

fn queue() -> Vec<Entry> {
    let routine = Entry { repeat: Some("30m".into()), ..reminder(2, "prs") };
    let project = Entry { id: 9, state: "started".into(), ..Entry::default() };
    let runs = [bots_entry(5, "prs", ("answered", Some(2))), bots_entry(7, "prs", ("failed", Some(2)))];
    let mut queue = vec![routine, reminder(4, "prs"), Entry { state: "answered".into(), ..reminder(3, "prs") }];
    queue.extend(runs);
    queue.push(project);
    queue
}

fn releases(server: &str, here: Release) -> Releases<'static> {
    Releases { server: Version::parse(server), here, machine: None }
}

#[test]
fn the_line_counts_routines_and_reminders_of_bots_that_exist_and_never_runs() {
    let bots = |ids: &[&str]| ids.iter().map(|id| (*id).to_string()).collect::<Vec<_>>();
    assert_eq!(
        elsewhere(&queue(), &bots(&["prs"])).as_deref(),
        Some("3 bot routines: in each bot's chat, under Routines")
    );
    assert_eq!(elsewhere(&queue(), &bots(&["ops"])), None);
    assert_eq!(elsewhere(&queue(), &[]), None);
    let one = [reminder(1, "ops")];
    assert_eq!(elsewhere(&one, &bots(&["ops"])).as_deref(), Some("1 bot routine: in its bot's chat, under Routines"));
}

#[test]
fn the_note_shows_only_when_this_computers_command_line_is_older() {
    let older_here = releases("0.13.109", Release::Read(Version::parse("0.13.80")));
    let words = "Scheduling for a bot from this window needs cornercase 0.13.85 or newer on this Mac (it has 0.13.80).";
    assert_eq!(add_note(&older_here, Os::Mac).as_deref(), Some(words));
    assert_eq!(add_note(&releases("0.13.80", Release::Read(Version::parse("0.13.80"))), Os::Mac), None);
    assert_eq!(add_note(&releases("0.13.109", Release::Read(Version::parse("0.13.109"))), Os::Mac), None);
    assert_eq!(add_note(&releases("0.13.109", Release::Unread), Os::Mac), None);
}

#[test]
fn clear_started_counts_the_bots_runs_it_would_take() {
    assert_eq!(cleared_too(&queue()), 3);
    let project = Entry { id: 1, state: "started".into(), ..Entry::default() };
    assert_eq!(cleared_too(&[project]), 0);
}

#[test]
fn a_bots_form_is_titled_by_what_it_makes() {
    let titles = [(false, true), (false, false), (true, true), (true, false)]
        .map(|(editing, repeats)| header_text(editing, repeats, "PRs").0);
    assert_eq!(titles, ["Schedule a routine", "Schedule a reminder", "Edit routine", "Edit reminder"]);
    assert!(header_text(false, true, "PRs").1.starts_with("PRs answers in its chat every time"));
    assert!(header_text(false, false, "PRs").1.starts_with("PRs answers in its chat at the time"));
}

#[test]
fn a_reminders_note_is_not_a_routines() {
    let routine = where_note(true, "PRs");
    let reminder = where_note(false, "PRs");
    assert!(routine.starts_with("A routine only runs") && routine.contains("conversation of PRs"), "{routine}");
    assert!(!reminder.contains("routine") && reminder.contains("conversation of PRs"), "{reminder}");
}
