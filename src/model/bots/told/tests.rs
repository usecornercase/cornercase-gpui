use std::time::{Duration, Instant};

use super::*;

fn answer(bot: &str, id: u64) -> Answer {
    Answer { bot: bot.to_string(), id, failed: false, run: None, text: format!("Answer {id}") }
}

fn ids(answers: &[Answer]) -> Vec<u64> {
    answers.iter().map(|a| a.id).collect()
}

fn later(base: Instant, ms: u64) -> Instant {
    base + Duration::from_millis(ms)
}

fn line(kind: &str, text: &str) -> Line {
    Line {
        id: 7,
        at: 0,
        kind: Kind::from(kind),
        text: text.to_string(),
        held: false,
        ask: None,
        routine: None,
        agent: None,
        body: None,
    }
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
    for kind in ["you", "step", "notice", "something-new", "started", "finished", "waiting"] {
        assert_eq!(answer_of("notes", &line(kind, "x")), None, "{kind}");
    }
    assert_eq!(answer_of("notes", &line("bot", "x")).map(|a| a.failed), Some(false));
}

#[test]
fn an_error_is_an_answer_that_failed() {
    let run = Line { routine: Some(5), ..line("error", "Not logged in") };
    let failed = answer_of("notes", &run).expect("an answer");
    assert_eq!((failed.failed, failed.run, failed.id), (true, Some(5), 7));
}

#[test]
fn two_answers_of_one_bot_counted_in_one_app_line_are_both_told_oldest_first() {
    let (mut answers, base) = (Answers::default(), Instant::now());
    answers.heard(answer("notes", 6), base);
    answers.heard(answer("notes", 4), later(base, 50));
    assert_eq!(ids(&answers.counted("notes", (0, 2), later(base, 300))), [4, 6]);
}

fn six_then_four_told() -> (Answers, Instant) {
    let (mut answers, base) = (Answers::default(), Instant::now());
    answers.heard(answer("notes", 4), base);
    answers.heard(answer("notes", 6), later(base, 50));
    assert_eq!(ids(&answers.counted("notes", (0, 1), later(base, 300))), [6]);
    assert_eq!(ids(&answers.counted("notes", (1, 2), later(base, 800))), [4]);
    (answers, base)
}

#[test]
fn a_count_that_rises_by_one_tells_the_newest_answer_and_keeps_the_older_for_the_next_rise() {
    let (mut answers, base) = six_then_four_told();
    assert_eq!(answers.counted("notes", (2, 3), later(base, 900)), []);
}

#[test]
fn the_told_mark_only_moves_up_so_a_newer_answer_heard_again_is_not_told() {
    let (mut answers, base) = six_then_four_told();
    assert_eq!(answers.heard(answer("notes", 6), later(base, 900)), None);
    assert_eq!(answers.counted("notes", (2, 3), later(base, 1000)), []);
    assert_eq!(answers.heard(answer("notes", 6), later(base, 1100)), None);
}

#[test]
fn an_answer_kept_across_a_rise_still_expires_after_two_seconds() {
    let (mut answers, base) = (Answers::default(), Instant::now());
    answers.heard(answer("notes", 4), base);
    answers.heard(answer("notes", 6), later(base, 50));
    answers.counted("notes", (0, 1), later(base, 300));
    assert_eq!(answers.counted("notes", (1, 2), later(base, 2_500)), []);
}

#[test]
fn an_answer_that_was_read_is_not_told_for_the_next_count() {
    let (mut answers, base) = (Answers::default(), Instant::now());
    answers.heard(answer("notes", 4), base);
    answers.heard(answer("ops", 2), later(base, 20));
    answers.counted("notes", (1, 0), later(base, 100));
    assert_eq!(answers.counted("notes", (0, 1), later(base, 400)), []);
    assert_eq!(answers.heard(answer("notes", 6), later(base, 500)), Some(answer("notes", 6)));
    assert_eq!(ids(&answers.counted("ops", (0, 1), later(base, 600))), [2]);
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
fn a_count_that_falls_to_none_means_the_answers_were_read() {
    assert!(read_since(2, 0));
    assert!(read_since(1, 0));
    assert!(!read_since(0, 0));
    assert!(!read_since(2, 1));
    assert!(!read_since(0, 1));
}

#[test]
fn a_line_heard_twice_before_a_rise_of_two_is_one_answer() {
    let (mut answers, base) = (Answers::default(), Instant::now());
    answers.heard(answer("notes", 4), base);
    answers.heard(answer("notes", 4), later(base, 10));
    assert_eq!(ids(&answers.counted("notes", (0, 2), later(base, 300))), [4]);
}

#[test]
fn a_bot_made_again_with_a_reused_id_tells_its_first_answer_after_it_left_the_list() {
    let (mut answers, base) = (Answers::default(), Instant::now());
    answers.heard(answer("notes", 9), base);
    answers.counted("notes", (0, 1), later(base, 100));
    answers.keep_listed(&["ops".to_string()]);
    answers.heard(answer("notes", 1), later(base, 200));
    assert_eq!(ids(&answers.counted("notes", (0, 1), later(base, 300))), [1]);
}

#[test]
fn a_bot_made_again_with_a_reused_id_tells_its_first_answer_after_it_was_removed() {
    let (mut answers, base) = (Answers::default(), Instant::now());
    answers.heard(answer("notes", 9), base);
    answers.counted("notes", (0, 1), later(base, 100));
    answers.forget_bot("notes");
    answers.heard(answer("notes", 1), later(base, 200));
    assert_eq!(ids(&answers.counted("notes", (0, 1), later(base, 300))), [1]);
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
    assert!(!tells(&answer("prs", 1), (Some("prs"), true), true));
}

#[test]
fn the_chat_shown_while_the_window_is_behind_tells() {
    assert!(tells(&answer("prs", 1), (Some("prs"), false), true));
}

#[test]
fn a_bot_whose_chat_does_not_show_tells_whether_the_window_is_active_or_behind() {
    for active in [true, false] {
        assert!(tells(&answer("notes", 1), (Some("prs"), active), true));
        assert!(tells(&answer("notes", 1), (None, active), true));
    }
}

#[test]
fn a_runs_error_is_left_to_its_queue_message() {
    let failed = Answer { failed: true, run: Some(5), ..answer("notes", 1) };
    assert!(!tells(&failed, (None, false), true));
    assert!(tells(&Answer { run: None, ..failed }, (None, false), true));
}

#[test]
fn nothing_is_told_below_the_gate() {
    assert!(!tells(&answer("notes", 1), (None, false), false));
}

#[test]
fn a_runs_answer_is_told() {
    assert!(tells(&Answer { run: Some(5), ..answer("notes", 1) }, (None, true), true));
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
    let told = Told::new(&answer("notes", 1), BotKey::here("notes"), "notes", None);
    assert_eq!(told.text(), "notes: Answer 1");
}

#[test]
fn an_errors_toast_says_the_bot_could_not_answer() {
    let failed = Answer { failed: true, text: "Not logged in\nmore".into(), ..answer("notes", 1) };
    assert_eq!(
        Told::new(&failed, BotKey::here("notes"), "notes", None).text(),
        "notes could not answer: Not logged in"
    );
}

#[test]
fn an_error_is_read_as_the_chat_draws_it_not_as_markdown() {
    let failed = Answer { failed: true, text: "\n__init__.py *failed* [x](y)\nnext".into(), ..answer("notes", 1) };
    assert_eq!(Told::new(&failed, BotKey::here("notes"), "notes", None).first, "__init__.py *failed* [x](y)");
    let long = Answer { text: "e".repeat(300), ..failed };
    let first = Told::new(&long, BotKey::here("notes"), "notes", None).first;
    assert_eq!((first.chars().count(), first.ends_with('…')), (100, true));
}

#[test]
fn an_answer_with_no_text_says_the_bot_answered() {
    let blank = Answer { text: String::new(), ..answer("notes", 1) };
    assert_eq!(Told::new(&blank, BotKey::here("notes"), "notes", None).text(), "notes answered");
    assert_eq!(
        Told::new(&Answer { failed: true, ..blank }, BotKey::here("notes"), "notes", None).text(),
        "notes could not answer"
    );
}

#[test]
fn the_headline_says_answered_or_could_not_answer_and_the_body_is_the_first_line() {
    let told = Told::new(&answer("notes", 1), BotKey::here("notes"), "notes", None);
    assert_eq!((told.headline().as_str(), told.body().as_deref()), ("notes answered", Some("Answer 1")));
    let failed = Told::new(
        &Answer { failed: true, text: String::new(), ..answer("notes", 1) },
        BotKey::here("notes"),
        "notes",
        None,
    );
    assert_eq!((failed.headline().as_str(), failed.body()), ("notes could not answer", None));
}

#[test]
fn a_gone_bot_is_named_as_gone() {
    assert_eq!(gone("notes"), "notes is gone");
}

#[test]
fn an_ask_is_told_in_cornercases_words() {
    let names = vec!["campus-user".to_string(), "campus-password".to_string()];
    let told = Told::asking(BotKey::here("campus"), "Campus", &Ask { id: 3, names, ..Ask::default() });
    assert_eq!(told.text(), "Campus asks for a secret: campus-user, campus-password");
    assert_eq!(told.headline(), "Campus asks for a secret");
    assert_eq!(told.body().as_deref(), Some("campus-user, campus-password"));
    assert_eq!((told.ask, told.failed), (Some(3), false));
}

#[test]
fn a_runs_answer_names_its_routine_in_the_toast_and_the_headline() {
    let run = Answer { run: Some(5), ..answer("prs", 1) };
    let told = Told::new(&run, BotKey::here("prs"), "PRs", Some("Review PRs"));
    assert_eq!(told.text(), "PRs · Review PRs: Answer 1");
    assert_eq!(told.headline(), "PRs · Review PRs answered");
    assert_eq!(told.run, Some(5));
}

#[test]
fn an_answer_without_a_label_reads_as_before() {
    let told = Told::new(&Answer { run: Some(5), ..answer("prs", 1) }, BotKey::here("prs"), "PRs", None);
    assert_eq!((told.text().as_str(), told.headline().as_str()), ("PRs: Answer 1", "PRs answered"));
    let blank = Told::new(&answer("prs", 1), BotKey::here("prs"), "PRs", Some(""));
    assert_eq!(blank.text(), "PRs: Answer 1");
}

#[test]
fn a_runs_failure_is_still_not_told_whatever_its_label() {
    let failed = Answer { failed: true, run: Some(5), ..answer("prs", 1) };
    assert!(!tells(&failed, (None, false), true));
    assert_eq!(
        Told::new(&failed, BotKey::here("prs"), "PRs", Some("Review PRs")).headline(),
        "PRs · Review PRs could not answer"
    );
}

#[test]
fn the_outline_text_is_one_line_per_block_without_marks() {
    let text = outline_text("# Done\n\n**Fixed** the `flaky` test.\n\n- one\n- two\n\n---\n\n```\nlet x = 1;\n```");
    assert_eq!(text, "Done\nFixed the flaky test.\none\ntwo\nlet x = 1;");
}
