use super::*;
use crate::cli::{Queue, QueueTap};
use crate::model::bots::say_args;

fn put(queue: &Queue, call: Call) {
    drop(reply_of(queue, call.args, call.stdin));
}

fn said(text: &str) -> Vec<(Vec<String>, Option<Vec<u8>>)> {
    let (queue, tap) = QueueTap::new();
    put(&queue, Call::say("prs", text));
    tap.with_stdin()
}

fn whole(text: &str) {
    assert_eq!(said(text), [(say_args("prs"), Some(text.as_bytes().to_vec()))], "{text:?}");
}

#[test]
fn a_message_that_is_a_dash_goes_whole_on_standard_input() {
    whole("-");
}

#[test]
fn a_message_that_starts_with_a_dash_goes_whole_on_standard_input() {
    whole("-x starts with a dash");
}

#[test]
fn a_message_with_line_breaks_goes_whole_on_standard_input() {
    whole("two\nlines\n");
}

#[test]
fn a_message_with_letters_outside_ascii_goes_whole_on_standard_input() {
    whole("año ñ 😀");
}

#[test]
fn a_message_never_appears_among_the_arguments() {
    let sent = said("secret text");
    assert!(sent[0].0.iter().all(|word| !word.contains("secret")));
}

#[test]
fn stop_and_new_conversation_go_on_the_queue_with_nothing_on_standard_input() {
    let (queue, tap) = QueueTap::new();
    put(&queue, Call::of(&["bot", "stop", "prs"]));
    put(&queue, Call::of(&["bot", "reset", "prs"]));
    let sent = tap.with_stdin();
    assert_eq!(sent.len(), 2);
    assert!(sent.iter().all(|(_, stdin)| stdin.is_none()));
    assert_eq!(sent[1].0, ["bot", "reset", "prs"]);
}

#[test]
fn two_messages_sent_one_after_the_other_arrive_in_that_order() {
    let (queue, tap) = QueueTap::new();
    for text in ["first", "second"] {
        put(&queue, Call::say("prs", text));
    }
    let texts: Vec<_> = tap.with_stdin().into_iter().map(|(_, stdin)| stdin.expect("text")).collect();
    assert_eq!(texts, [b"first".to_vec(), b"second".to_vec()]);
}

#[test]
fn a_bot_that_is_gone_offers_its_message_in_the_toast() {
    let action = copy_action("my words").expect("an action");
    assert_eq!(action.0, COPY_MESSAGE);
    assert!(matches!(action.1, ToastGo::Copy(clip) if clip.text == "my words"));
}

#[test]
fn a_blank_draft_offers_nothing_in_the_toast() {
    assert!(copy_action("  \n").is_none());
}
