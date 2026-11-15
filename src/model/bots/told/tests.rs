use std::time::{Duration, Instant};

use super::*;

fn answer(bot: &str, id: u64) -> Answer {
    Answer { bot: bot.to_string(), id, failed: false, run: false, text: format!("Answer {id}") }
}

fn ids(answers: &[Answer]) -> Vec<u64> {
    answers.iter().map(|a| a.id).collect()
}

fn later(base: Instant, ms: u64) -> Instant {
    base + Duration::from_millis(ms)
}

fn line(kind: &str, text: &str) -> Line {
    Line { id: 7, at: 0, kind: Kind::from(kind), text: text.to_string(), held: false }
}

#[test]
fn an_answer_heard_before_its_count_rises_is_told_when_it_rises() {
    let (mut answers, base) = (Answers::default(), Instant::now());
    assert_eq!(answers.heard(answer("notes", 4), base), None);
    assert_eq!(ids(&answers.counted("notes", (0, 1), later(base, 300))), [4]);
}

#[test]
fn an_answer_heard_after_its_count_rose_is_told_when_it_comes() {
    let (mut answers, base) = (Answers::default(), Instant::now());
    assert_eq!(answers.counted("notes", (0, 1), base), []);
    assert_eq!(answers.heard(answer("notes", 4), later(base, 200)), Some(answer("notes", 4)));
}

#[test]
fn an_answer_whose_count_never_rises_is_not_told() {
    let (mut answers, base) = (Answers::default(), Instant::now());
    answers.heard(answer("notes", 4), base);
    assert_eq!(answers.counted("notes", (0, 0), later(base, 300)), []);
    assert_eq!(answers.counted("notes", (0, 1), later(base, 5_000)), []);
}

#[test]
fn a_line_heard_twice_is_told_once() {
    let (mut answers, base) = (Answers::default(), Instant::now());
    answers.heard(answer("notes", 4), base);
    assert_eq!(answers.heard(answer("notes", 4), later(base, 10)), None);
    assert_eq!(ids(&answers.counted("notes", (0, 1), later(base, 300))), [4]);
    assert_eq!(answers.heard(answer("notes", 4), later(base, 400)), None);
    assert_eq!(answers.counted("notes", (1, 2), later(base, 800)), []);
}

#[test]
fn a_line_older_than_the_last_told_is_not_told() {
    let (mut answers, base) = (Answers::default(), Instant::now());
    answers.heard(answer("notes", 9), base);
    answers.counted("notes", (0, 1), later(base, 100));
    answers.counted("notes", (1, 2), later(base, 200));
    assert_eq!(answers.heard(answer("notes", 5), later(base, 300)), None);
    assert_eq!(answers.counted("notes", (2, 3), later(base, 600)), []);
}

#[test]
fn steps_notices_and_your_own_lines_are_no_answers() {
    for kind in ["you", "step", "notice", "something-new"] {
        assert_eq!(answer_of("notes", &line(kind, "x"), false), None, "{kind}");
    }
    assert_eq!(answer_of("notes", &line("bot", "x"), false).map(|a| a.failed), Some(false));
}

#[test]
fn an_error_is_an_answer_that_failed() {
    let failed = answer_of("notes", &line("error", "Not logged in"), true).expect("an answer");
    assert_eq!((failed.failed, failed.run, failed.id), (true, true, 7));
}

#[test]
fn two_answers_of_one_bot_counted_in_one_app_line_are_both_told_oldest_first() {
    let (mut answers, base) = (Answers::default(), Instant::now());
    answers.heard(answer("notes", 6), base);
    answers.heard(answer("notes", 4), later(base, 50));
    assert_eq!(ids(&answers.counted("notes", (0, 2), later(base, 300))), [4, 6]);
}

#[test]
fn a_count_that_rises_by_one_tells_the_newest_answer_and_drops_the_older() {
    let (mut answers, base) = (Answers::default(), Instant::now());
    answers.heard(answer("notes", 4), base);
    answers.heard(answer("notes", 6), later(base, 50));
    assert_eq!(ids(&answers.counted("notes", (0, 1), later(base, 300))), [6]);
    assert_eq!(answers.counted("notes", (1, 2), later(base, 400)), []);
}

#[test]
fn a_rise_with_no_answer_within_two_seconds_tells_nothing_later() {
    let (mut answers, base) = (Answers::default(), Instant::now());
    answers.counted("notes", (0, 1), base);
    assert_eq!(answers.heard(answer("notes", 4), later(base, 2_500)), None);
}

#[test]
fn an_answer_heard_more_than_two_seconds_before_its_rise_is_not_told() {
    let (mut answers, base) = (Answers::default(), Instant::now());
    answers.heard(answer("notes", 4), base);
    assert_eq!(answers.counted("notes", (0, 1), later(base, 2_500)), []);
}

#[test]
fn a_count_back_at_zero_forgets_what_was_owed() {
    let (mut answers, base) = (Answers::default(), Instant::now());
    answers.counted("notes", (0, 1), base);
    answers.counted("notes", (1, 0), later(base, 500));
    assert_eq!(answers.heard(answer("notes", 4), later(base, 700)), None);
}

#[test]
fn the_count_of_another_bot_tells_nothing_of_this_one() {
    let (mut answers, base) = (Answers::default(), Instant::now());
    answers.heard(answer("notes", 4), base);
    assert_eq!(answers.counted("ops", (0, 1), later(base, 100)), []);
    assert_eq!(ids(&answers.counted("notes", (0, 1), later(base, 200))), [4]);
}

#[test]
fn a_bot_that_leaves_the_list_forgets_what_was_heard_for_it() {
    let (mut answers, base) = (Answers::default(), Instant::now());
    answers.heard(answer("notes", 4), base);
    answers.counted("ops", (0, 1), base);
    answers.keep_listed(&["ops".to_string()]);
    assert_eq!(answers.counted("notes", (0, 1), later(base, 100)), []);
    assert_eq!(answers.heard(answer("ops", 2), later(base, 150)), Some(answer("ops", 2)));
    answers.forget_bot("ops");
    assert_eq!(answers.heard(answer("ops", 2), later(base, 200)), None);
}

#[test]
fn a_new_server_run_forgets_everything() {
    let (mut answers, base) = (Answers::default(), Instant::now());
    answers.heard(answer("notes", 9), base);
    answers.counted("notes", (0, 1), base);
    answers.counted("ops", (0, 1), base);
    answers.forget();
    assert_eq!(answers.heard(answer("ops", 1), later(base, 100)), None);
    assert_eq!(answers.heard(answer("notes", 3), later(base, 150)), None);
    assert_eq!(ids(&answers.counted("notes", (0, 1), later(base, 300))), [3]);
}

#[test]
fn the_chat_shown_in_the_active_window_tells_nothing() {
    assert!(!tells(&answer("prs", 1), Some("prs"), true));
}

#[test]
fn the_chat_shown_while_the_window_is_behind_tells() {
    assert!(tells(&answer("prs", 1), Some("prs"), false));
}

#[test]
fn a_bot_whose_chat_does_not_show_tells_whether_the_window_is_active_or_behind() {
    for active in [true, false] {
        assert!(tells(&answer("notes", 1), Some("prs"), active));
        assert!(tells(&answer("notes", 1), None, active));
    }
}

#[test]
fn a_runs_error_is_left_to_its_queue_message() {
    let failed = Answer { failed: true, run: true, ..answer("notes", 1) };
    assert!(!tells(&failed, None, false));
    assert!(tells(&Answer { run: false, ..failed }, None, false));
}

#[test]
fn a_runs_answer_is_told() {
    assert!(tells(&Answer { run: true, ..answer("notes", 1) }, None, true));
}

#[test]
fn the_first_line_of_a_plain_answer_is_its_first_paragraph() {
    assert_eq!(first_line("One line.\nSame paragraph.\n\nSecond."), "One line. Same paragraph.");
}

#[test]
fn a_heading_is_its_text_without_its_marks() {
    assert_eq!(first_line("## Answer (fake)\n\nbody"), "Answer (fake)");
}

#[test]
fn a_list_item_is_its_text_without_its_mark() {
    assert_eq!(first_line("- First item\n- Second"), "First item");
}

#[test]
fn an_answer_that_is_only_a_code_block_is_its_first_line_of_code() {
    assert_eq!(first_line("```sh\n\n  gh pr list \n```"), "gh pr list");
}

#[test]
fn empty_first_lines_and_a_rule_are_passed_over() {
    assert_eq!(first_line("\n\n   \nAfter two empty lines"), "After two empty lines");
    assert_eq!(first_line("---\n\nAfter the rule"), "After the rule");
    assert_eq!(first_line("   \n\n"), "");
}

#[test]
fn a_link_keeps_its_text_and_drops_its_address() {
    assert_eq!(first_line("[The **pull** request](https://example.com/pr/12) is ready"), "The pull request is ready");
}

#[test]
fn a_long_first_line_is_cut_at_100_characters_with_an_ellipsis() {
    let long = "w".repeat(300);
    let cut = first_line(&long);
    assert_eq!(cut.chars().count(), 100);
    assert!(cut.ends_with('…'));
    let exact = "é".repeat(100);
    assert_eq!(first_line(&exact), exact);
}

#[test]
fn control_characters_and_runs_of_blanks_are_left_out() {
    assert_eq!(first_line("a\u{7}b   c\u{1b}[0m\t d"), "ab c[0m d");
}

#[test]
fn an_answers_toast_names_the_bot_and_its_first_line() {
    let told = Told::new(&answer("notes", 1), "notes");
    assert_eq!(told.text(), "notes: Answer 1");
}

#[test]
fn an_errors_toast_says_the_bot_could_not_answer() {
    let failed = Answer { failed: true, text: "Not logged in\nmore".into(), ..answer("notes", 1) };
    assert_eq!(Told::new(&failed, "notes").text(), "notes could not answer: Not logged in more");
}

#[test]
fn an_answer_with_no_text_says_the_bot_answered() {
    let blank = Answer { text: String::new(), ..answer("notes", 1) };
    assert_eq!(Told::new(&blank, "notes").text(), "notes answered");
    assert_eq!(Told::new(&Answer { failed: true, ..blank }, "notes").text(), "notes could not answer");
}

#[test]
fn the_headline_says_answered_or_could_not_answer_and_the_body_is_the_first_line() {
    let told = Told::new(&answer("notes", 1), "notes");
    assert_eq!((told.headline().as_str(), told.body().as_deref()), ("notes answered", Some("Answer 1")));
    let failed = Told::new(&Answer { failed: true, text: String::new(), ..answer("notes", 1) }, "notes");
    assert_eq!((failed.headline().as_str(), failed.body()), ("notes could not answer", None));
}

#[test]
fn a_gone_bot_is_named_as_gone() {
    assert_eq!(gone("notes"), "notes is gone");
}
