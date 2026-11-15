use super::*;

#[test]
fn a_message_of_only_blanks_is_not_sent() {
    for text in ["", " ", "  \n\t \n"] {
        assert_eq!(on_return(text, false), None, "{text:?}");
        assert!(!sendable(text));
    }
}

#[test]
fn return_sends_the_text_as_it_was_typed() {
    assert_eq!(on_return("  hello\n", false).as_deref(), Some("  hello\n"));
}

#[test]
fn two_returns_send_a_text_once() {
    assert_eq!(on_return("hi", true), None);
    assert_eq!(on_return("", false), None);
}

#[test]
fn a_message_goes_to_a_bot_on_standard_input() {
    assert_eq!(say_args("prs"), ["bot", "say", "prs", "--json", "-"]);
}

#[test]
fn a_refused_message_goes_back_whole() {
    let mut drafts = Drafts::default();
    drafts.put_back("prs", "a\nb ");
    assert_eq!(drafts.text("prs"), "a\nb ");
}

#[test]
fn a_refused_message_goes_back_before_what_was_typed_since() {
    assert_eq!(restored("first", "second"), "first\nsecond");
    let mut drafts = Drafts::default();
    drafts.keep("prs", "second".into());
    drafts.put_back("prs", "first");
    assert_eq!(drafts.text("prs"), "first\nsecond");
}

#[test]
fn a_refused_message_to_a_listed_bot_goes_back_to_its_draft() {
    assert_eq!(refused_goes(Said::Typed, true), Goes::Draft);
}

#[test]
fn a_refused_message_to_a_bot_that_is_gone_is_offered_in_the_toast() {
    assert_eq!(refused_goes(Said::Typed, false), Goes::Toast);
}

#[test]
fn a_refused_retry_leaves_the_draft_alone() {
    assert_eq!(refused_goes(Said::Again, true), Goes::Nowhere);
    assert_eq!(refused_goes(Said::Again, false), Goes::Nowhere);
}

#[test]
fn a_draft_is_kept_for_each_bot() {
    let mut drafts = Drafts::default();
    drafts.keep("prs", "one".into());
    drafts.keep("ops", "two".into());
    assert_eq!((drafts.text("prs"), drafts.text("ops"), drafts.text("none")), ("one", "two", ""));
}

#[test]
fn a_bots_draft_is_there_when_its_chat_shows_again() {
    let mut drafts = Drafts::default();
    drafts.keep("prs", "one".into());
    drafts.keep("ops", "other".into());
    assert_eq!(drafts.text("prs"), "one");
}

#[test]
fn a_removed_bots_draft_is_dropped_by_keeping_only_the_listed() {
    let mut drafts = Drafts::default();
    drafts.keep("prs", "one".into());
    drafts.keep("ops", "two".into());
    drafts.keep_only(&["ops".to_string()]);
    assert_eq!((drafts.text("prs"), drafts.text("ops")), ("", "two"));
    assert_eq!(drafts.take("ops").as_deref(), Some("two"));
    assert_eq!(drafts.take("ops"), None);
}

#[test]
fn typing_into_a_draft_does_not_ask_the_field_to_read_it_again() {
    let mut drafts = Drafts::default();
    let before = drafts.revision();
    drafts.keep("prs", "typed".into());
    drafts.keep("prs", String::new());
    assert_eq!(drafts.revision(), before);
}

#[test]
fn a_draft_put_back_asks_the_field_to_read_it_again() {
    let mut drafts = Drafts::default();
    let before = drafts.revision();
    drafts.put_back("prs", "back");
    assert!(drafts.revision() > before);
}

#[test]
fn an_example_fills_an_empty_draft() {
    assert_eq!(with_example("", "Which?"), "Which?");
    assert_eq!(with_example("  \n", "Which?"), "Which?");
    let mut drafts = Drafts::default();
    let before = drafts.revision();
    drafts.add_example("prs", "Which?");
    assert_eq!(drafts.text("prs"), "Which?");
    assert!(drafts.revision() > before);
}

#[test]
fn an_example_goes_after_a_draft_on_a_line_of_its_own() {
    assert_eq!(with_example("my words", "Which?"), "my words\nWhich?");
}

#[test]
fn each_job_of_a_bot_has_a_key_of_its_own() {
    let keys = [say_key("prs"), stop_key("prs"), reset_key("prs"), say_key("ops")];
    for (i, key) in keys.iter().enumerate() {
        assert!(keys.iter().skip(i + 1).all(|other| other != key), "{key}");
    }
}

#[test]
fn a_chat_that_shows_another_bot_fills_the_field_from_its_draft() {
    assert_eq!(refill(Some("prs"), Some("ops"), true, 3, 3), Refill::Fill);
    assert_eq!(refill(None, Some("prs"), true, 0, 0), Refill::Fill);
}

#[test]
fn a_chat_that_shows_the_same_bot_again_keeps_the_field_and_its_caret() {
    assert_eq!(refill(Some("prs"), Some("prs"), true, 3, 3), Refill::Keep);
    assert_eq!(refill(Some("prs"), None, true, 3, 3), Refill::Keep);
}

#[test]
fn a_draft_the_window_put_in_asks_the_field_to_fill_again() {
    assert_eq!(refill(Some("prs"), Some("prs"), true, 3, 4), Refill::Fill);
}

#[test]
fn a_removed_bots_text_is_forgotten_so_a_bot_made_again_with_its_id_starts_empty() {
    assert_eq!(refill(Some("prs"), None, false, 3, 3), Refill::Forget);
}
