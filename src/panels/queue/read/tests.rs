use super::{Ask, Counter, Keep, Reads, failed, job_key, job_words, refreshing_keys, show_args};

#[test]
fn opening_asks_once_without_the_conversation() {
    let (_, ask) = Reads::new(0, Some(5));
    assert!(!ask.talk);
}

#[test]
fn the_same_result_at_asks_nothing_and_a_new_one_asks_again() {
    let (mut reads, _) = Reads::new(0, Some(5));
    assert_eq!(reads.moved(Some(5)), None);
    assert!(reads.moved(Some(9)).is_some());
    assert_eq!(reads.moved(Some(9)), None);
    assert!(reads.moved(None).is_some());
}

#[test]
fn once_the_conversation_was_wanted_every_later_ask_carries_it() {
    let (mut reads, first) = Reads::new(0, None);
    assert!(!first.talk);
    assert!(reads.wants_talk().is_some_and(|ask| ask.talk));
    assert!(reads.moved(Some(1)).is_some_and(|ask| ask.talk));
    assert!(reads.again().talk);
}

#[test]
fn choosing_the_conversation_again_asks_nothing_more() {
    let (mut reads, _) = Reads::new(0, None);
    assert!(reads.wants_talk().is_some());
    assert_eq!(reads.wants_talk(), None);
}

#[test]
fn an_older_answer_never_replaces_a_newer_one() {
    let (mut reads, first) = Reads::new(0, None);
    let second = reads.moved(Some(1)).expect("a new ask");
    assert_eq!(reads.answered(second), Keep { facts: true, talk: false });
    assert_eq!(reads.answered(first), Keep { facts: false, talk: false });
}

#[test]
fn a_plain_answer_keeps_the_conversation_shown() {
    let (mut reads, _) = Reads::new(0, None);
    let talk = reads.wants_talk().expect("the conversation is wanted");
    let plain = Ask { seq: talk.seq + 1, talk: false };
    assert_eq!(reads.answered(talk), Keep { facts: true, talk: true });
    assert_eq!(reads.answered(plain), Keep { facts: true, talk: false });
}

#[test]
fn an_answer_from_an_earlier_opening_is_not_kept_and_does_not_block_the_new_one() {
    let (_, old) = Reads::new(0, None);
    let (mut reads, fresh) = Reads::new(old.seq, None);
    assert!(fresh.seq > old.seq);
    assert_eq!(reads.answered(old), Keep { facts: false, talk: false });
    assert_eq!(reads.answered(fresh), Keep { facts: true, talk: false });
}

#[test]
fn the_command_the_job_and_the_failure_read_as_words() {
    assert_eq!(show_args(7, false), ["queue", "show", "7", "--json"]);
    assert_eq!(show_args(7, true), ["queue", "show", "7", "--json", "--conversation"]);
    assert_eq!(job_key(7, false), "queue-show:7");
    assert_eq!(job_key(7, true), "queue-show:7:talk");
    assert_eq!(job_words("fix", false), "Reading what “fix” did…");
    assert_eq!(job_words("fix", true), "Reading the conversation of “fix”…");
    assert_eq!(failed(false, "no"), "Could not read what it did: no");
    assert_eq!(failed(true, "no"), "Could not read the conversation: no");
}

#[test]
fn the_counter_hands_the_next_opening_a_base_above_every_ask_seen() {
    let mut counter = Counter::default();
    let (_, first) = Reads::new(counter.base(), None);
    counter.seen(first);
    counter.seen(Ask { seq: first.seq + 4, talk: true });
    counter.seen(first);
    assert_eq!(counter.base(), first.seq + 4);
    let (_, next) = Reads::new(counter.base(), None);
    assert!(next.seq > first.seq + 4);
}

#[test]
fn the_busy_mark_counts_the_conversation_job_only_over_a_conversation_shown() {
    assert_eq!(refreshing_keys(7, false), ["queue-show:7"]);
    assert_eq!(refreshing_keys(7, true), ["queue-show:7", "queue-show:7:talk"]);
}
