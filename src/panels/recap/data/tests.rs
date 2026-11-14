use std::rc::Rc;

use super::{Answer, Choice, Page, Slot, writing};
use crate::test_util::fixture;

fn answer(text: &str) -> Answer {
    serde_json::from_str(text).expect("an answer")
}

fn filled() -> Answer {
    answer(fixture!("recap/now.json"))
}

fn read(answer: Answer) -> Slot {
    let mut slot = Slot::default();
    slot.answered(Ok(answer));
    slot
}

fn page_of(slot: &Slot) -> &Rc<Page> {
    slot.page.as_ref().expect("a page")
}

#[test]
fn a_range_asks_by_its_own_name() {
    assert_eq!(Choice::Now.command(false), ["recap", "now", "--json"]);
    assert_eq!(Choice::Yesterday.command(false), ["recap", "yesterday", "--json"]);
    assert_eq!(Choice::Week.command(false), ["recap", "week", "--json"]);
}

#[test]
fn asking_again_adds_fresh() {
    assert_eq!(Choice::Week.command(true), ["recap", "week", "--json", "--fresh"]);
}

#[test]
fn each_range_runs_as_a_job_of_its_own() {
    assert_ne!(Choice::Now.busy().key(), Choice::Week.busy().key());
    assert_eq!(Choice::Yesterday.busy().key(), Choice::Yesterday.key());
}

#[test]
fn the_ranges_are_shown_as_the_tui_names_them() {
    let titles: Vec<&str> = Choice::ALL.into_iter().map(Choice::title).collect();
    assert_eq!(titles, ["Now", "Yesterday", "7 days"]);
}

#[test]
fn every_range_has_a_slot_of_its_own() {
    let at: Vec<usize> = Choice::ALL.into_iter().map(Choice::index).collect();
    assert_eq!(at, [0, 1, 2]);
}

#[test]
fn a_written_answer_gives_a_page_with_its_label() {
    let slot = read(answer(fixture!("recap/yesterday.json")));
    assert_eq!(page_of(&slot).answer.label.as_deref(), Some("yesterday · wed 4 nov"));
    assert!((slot.error.is_none() && slot.empty.is_none()) && slot.progress.is_none());
}

#[test]
fn the_byline_says_who_wrote_it_what_it_read_and_when() {
    let mut written = filled();
    written.written_at = Some("2026-11-06T10:00:00.000Z".to_string());
    let page = Page::new(written);
    let now = crate::panels::flows::model::epoch("2026-11-06T10:05:00.000Z").expect("a time");
    assert_eq!(page.byline_parts(now), ["Claude Code", "claude-sonnet-4-5", "read 3 conversations", "written 5m ago"]);
}

#[test]
fn a_recap_written_a_moment_ago_says_so() {
    let time = "2026-11-06T10:00:00.000Z";
    let page = Page::new(Answer { written_at: Some(time.to_string()), conversations: 1, ..Answer::default() });
    let now = crate::panels::flows::model::epoch(time).expect("a time");
    assert_eq!(page.byline_parts(now), ["read 1 conversation", "written just now"]);
}

#[test]
fn an_agent_without_a_title_goes_by_its_name_and_a_time_that_does_not_read_is_left_out() {
    let written = Answer {
        agent: Some("codex".to_string()),
        written_at: Some("yesterday".to_string()),
        conversations: 2,
        ..Answer::default()
    };
    assert_eq!(Page::new(written).byline_parts(0), ["codex", "read 2 conversations"]);
}

#[test]
fn nothing_to_sum_up_says_why_and_holds_no_page() {
    let slot = read(answer(fixture!("recap/empty.json")));
    assert_eq!(slot.empty.as_deref(), Some("Nothing to sum up: no agent conversation in the last 24 hours."));
    assert!(slot.page.is_none());
}

#[test]
fn nothing_to_sum_up_with_no_text_drops_the_page_it_held() {
    let mut slot = read(filled());
    slot.answered(Ok(answer(fixture!("recap/empty.json"))));
    assert!(slot.page.is_none());
}

#[test]
fn nothing_new_to_sum_up_keeps_the_last_text_and_says_why() {
    let mut stale = filled();
    stale.empty = Some("No agent conversation in the last 7 days.".to_string());
    let slot = read(stale);
    assert!(slot.page.is_some());
    assert_eq!(slot.empty.as_deref(), Some("No agent conversation in the last 7 days."));
}

#[test]
fn a_failed_run_keeps_the_last_good_text_with_its_error() {
    let slot = read(answer(fixture!("recap/failed.json")));
    assert_eq!(slot.error.as_deref(), Some("Not logged in · Please run /login"));
    assert!(page_of(&slot).answer.text.as_deref().is_some_and(|text| text.contains("On you")));
}

#[test]
fn a_failure_without_text_keeps_what_the_panel_already_read() {
    let mut slot = read(filled());
    slot.answered(Ok(Answer { error: Some("claude did not answer in time".to_string()), ..Answer::default() }));
    assert!(slot.page.is_some());
    assert_eq!(slot.error.as_deref(), Some("claude did not answer in time"));
}

#[test]
fn a_refused_command_keeps_the_text_and_shows_why() {
    let mut slot = read(filled());
    slot.answered(Err("the running cornercase server has no recap".to_string()));
    assert!(slot.page.is_some());
    assert_eq!(slot.error.as_deref(), Some("the running cornercase server has no recap"));
}

#[test]
fn a_good_answer_clears_the_last_failure() {
    let mut slot = Slot::default();
    slot.answered(Err("no recap".to_string()));
    slot.answered(Ok(filled()));
    assert!(slot.error.is_none());
}

#[test]
fn the_same_answer_keeps_its_page_and_so_its_scroll() {
    let mut slot = read(filled());
    let before = page_of(&slot).clone();
    slot.answered(Ok(filled()));
    assert!(Rc::ptr_eq(&before, page_of(&slot)));
}

#[test]
fn a_failure_on_the_same_text_keeps_its_page_too() {
    let mut slot = read(filled());
    let before = page_of(&slot).clone();
    let mut failed = filled();
    failed.error = Some("Not logged in".to_string());
    slot.answered(Ok(failed));
    assert!(Rc::ptr_eq(&before, page_of(&slot)));
}

#[test]
fn a_recap_written_again_gives_a_new_page() {
    let mut slot = read(filled());
    let before = page_of(&slot).clone();
    let mut again = filled();
    again.written_at = Some("2026-11-07T10:00:00.000Z".to_string());
    slot.answered(Ok(again));
    assert!(!Rc::ptr_eq(&before, page_of(&slot)));
}

#[test]
fn an_answer_that_still_runs_is_asked_for_again_and_gives_its_progress() {
    let mut running = filled();
    running.loading = true;
    running.progress = Some(4);
    let mut slot = Slot::default();
    assert!(slot.answered(Ok(running)));
    assert_eq!(slot.progress, Some(4));
}

#[test]
fn a_finished_answer_is_not_asked_for_again_and_forgets_the_progress() {
    let mut slot = Slot { progress: Some(4), ..Slot::default() };
    assert!(!slot.answered(Ok(filled())));
    assert_eq!(slot.progress, None);
}

#[test]
fn a_refused_command_is_not_asked_for_again() {
    assert!(!Slot::default().answered(Err("no recap".to_string())));
}

#[test]
fn writing_counts_the_steps_the_answer_gives() {
    assert_eq!(writing(None), "An agent is writing the recap…");
    assert_eq!(writing(Some(0)), "An agent is writing the recap…");
    assert_eq!(writing(Some(1)), "An agent is writing the recap… 1 step");
    assert_eq!(writing(Some(7)), "An agent is writing the recap… 7 steps");
}

#[test]
fn a_place_is_named_project_workspace_tab() {
    let places = filled().places;
    assert_eq!(places.get("c3").expect("a conversation").name(), "shop › main › claude");
    assert!(places.get("p1").expect("a pull request").is_pull());
    assert!(!places.get("c3").expect("a conversation").is_pull());
}
