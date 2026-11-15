use std::time::Instant;

use super::*;
use crate::cli::Version;

fn bot(id: &str) -> Bot {
    Bot { id: id.into(), name: id.into(), ..Bot::default() }
}

fn lacking(has: Option<u64>) -> Lacks<'static> {
    Lacks::ThisComputer(has.map(|has| Version::new(0, 13, has)))
}

#[test]
fn a_new_bot_shows_at_once_when_the_list_already_holds_it() {
    let made = Made::new("prs", Some("a"), Instant::now());
    assert_eq!(made.arrival(&[bot("prs")], Some("a"), Instant::now()), Arrival::Listed);
}

#[test]
fn a_new_bot_waits_for_the_line_that_lists_it() {
    let now = Instant::now();
    let made = Made::new("prs", Some("a"), now);
    assert_eq!(made.arrival(&[bot("ops")], Some("a"), now), Arrival::NotYet);
    assert_eq!(made.arrival(&[bot("ops"), bot("prs")], Some("a"), now), Arrival::Listed);
}

#[test]
fn a_new_bot_the_list_never_names_is_dropped_after_thirty_seconds() {
    let now = Instant::now();
    let made = Made::new("prs", Some("a"), now);
    assert_eq!(made.arrival(&[], Some("a"), now + AWAITED_FOR / 2), Arrival::NotYet);
    assert_eq!(made.arrival(&[], Some("a"), now + AWAITED_FOR), Arrival::Gone);
}

#[test]
fn a_new_server_run_drops_the_wait_for_a_new_bot() {
    let now = Instant::now();
    let made = Made::new("prs", Some("a"), now);
    assert_eq!(made.arrival(&[bot("prs")], Some("b"), now), Arrival::Gone);
    assert_eq!(Made::new("prs", None, now).arrival(&[], Some("b"), now), Arrival::NotYet);
}

#[test]
fn making_a_bot_is_offered_when_both_halves_have_bots() {
    assert_eq!(making(None, Os::Mac), Making::Offered);
}

#[test]
fn an_older_cornercase_here_says_so_where_new_bot_would_be() {
    assert_eq!(
        making(Some(lacking(Some(73))), Os::Mac),
        Making::Here("Making a bot needs cornercase 0.13.77 or newer on this Mac (it has 0.13.73).".into())
    );
    assert_eq!(
        making(Some(lacking(None)), Os::Linux),
        Making::Here("Making a bot needs cornercase 0.13.77 or newer on this computer.".into())
    );
}

#[test]
fn an_older_server_offers_nothing_and_says_why_on_the_key() {
    let with = Some(Lacks::Server(Some(Version::new(0, 13, 73))));
    assert_eq!(
        making(with, Os::Mac),
        Making::Server("Making a bot needs cornercase 0.13.77 or newer: the server runs 0.13.73.".into())
    );
    assert_eq!(
        making(Some(Lacks::Server(None)), Os::Mac),
        Making::Server("Making a bot needs cornercase 0.13.77 or newer.".into())
    );
}

#[test]
fn a_removal_asked_here_is_not_told() {
    let mut manage = Managing::default();
    manage.asked_to_remove("prs");
    assert!(!manage.told("prs", true));
}

#[test]
fn a_removal_from_elsewhere_is_told_when_its_chat_shows() {
    let manage = Managing::default();
    assert!(manage.told("prs", true));
    assert!(!manage.told("prs", false));
}

#[test]
fn a_failed_removal_forgets_it_was_asked() {
    let mut manage = Managing::default();
    manage.asked_to_remove("prs");
    manage.removal_failed("prs");
    assert!(manage.told("prs", true));
}

#[test]
fn a_removal_asked_here_is_forgotten_once_the_list_drops_the_bot() {
    let mut manage = Managing::default();
    manage.asked_to_remove("prs");
    manage.keep_listed(&["prs".to_string()]);
    assert!(manage.asked("prs"));
    manage.keep_listed(&[]);
    assert!(!manage.asked("prs"));
}

#[test]
fn a_dialog_that_names_a_removed_bot_closes() {
    let mut manage = Managing::default();
    manage.name_dialog("prs", 7);
    assert!(manage.closes_dialog("prs", Some(7)));
}

#[test]
fn another_dialog_stays_when_a_named_bot_is_removed() {
    let mut manage = Managing::default();
    manage.name_dialog("prs", 7);
    assert!(!manage.closes_dialog("prs", Some(8)));
    assert!(!manage.closes_dialog("prs", None));
}

#[test]
fn a_dialog_that_names_another_bot_stays() {
    let mut manage = Managing::default();
    manage.name_dialog("prs", 7);
    assert!(!manage.closes_dialog("ops", Some(7)));
}

#[test]
fn a_refusal_belongs_to_its_dialog_and_goes_on_the_next_change() {
    let mut manage = Managing::default();
    manage.refuse(7, "there is a bot called prs already".into());
    assert_eq!(manage.refusal(7), Some("there is a bot called prs already"));
    assert_eq!(manage.refusal(8), None);
    assert!(manage.forget_refusal(7));
    assert!(!manage.forget_refusal(7));
}

#[test]
fn a_move_of_the_person_drops_the_wait_for_a_new_bot() {
    let mut manage = Managing { made: Some(Made::new("prs", None, Instant::now())), ..Managing::default() };
    manage.person_moved();
    assert!(manage.made.is_none());
}

#[test]
fn a_form_closed_before_the_answer_awaits_no_chat() {
    let now = Instant::now();
    let creating = Creating::new(7, "prs", None);
    assert!(creating.made(false, "prs", now).is_none());
    assert_eq!(creating.made(true, "prs", now).map(|made| made.id), Some("prs".to_string()));
}

#[test]
fn a_reply_from_another_server_run_drops_the_wait_for_the_new_bot() {
    let now = Instant::now();
    let made = Creating::new(7, "prs", Some("run-a")).made(true, "prs", now).expect("shown");
    assert_eq!(made.arrival(&[bot("prs")], Some("run-b"), now), Arrival::Gone);
    assert_eq!(made.arrival(&[bot("prs")], Some("run-a"), now), Arrival::Listed);
}

#[test]
fn a_dialog_naming_a_bot_the_list_dropped_closes() {
    let mut manage = Managing::default();
    manage.name_dialog("prs", 7);
    assert!(manage.named_gone(&["ops".to_string()], Some(7)));
    assert!(!manage.named_gone(&["prs".to_string()], Some(7)));
    assert!(!manage.named_gone(&[], Some(8)));
}
