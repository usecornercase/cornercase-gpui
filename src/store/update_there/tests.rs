use super::*;
use crate::model::machines::LinkState;

fn update() -> UpdateThere {
    UpdateThere {
        machine: "vps".into(),
        destination: "vps.invalid".into(),
        args: ["--remote", "vps.invalid", "update"].map(String::from).to_vec(),
    }
}

fn listed(state: LinkState, updating: bool) -> Machine {
    Machine { name: "vps".into(), state, updating, ..Machine::default() }
}

#[test]
fn an_update_is_offered_only_with_update_words() {
    assert_eq!(offer_of(None, None, false), UpdateOffer::Hidden);
    assert_eq!(offer_of(None, None, true), UpdateOffer::Hidden);
    assert_eq!(offer_of(Some(&update()), None, false), UpdateOffer::Ask(update()));
}

#[test]
fn the_machines_own_update_wins() {
    let older = listed(LinkState::Older, false);
    let updating = listed(LinkState::Older, true);
    let linked = listed(LinkState::Linked, false);
    assert_eq!(offer_of(Some(&update()), Some(&older), false), UpdateOffer::Hidden);
    assert_eq!(offer_of(Some(&update()), Some(&updating), false), UpdateOffer::Hidden);
    assert_eq!(offer_of(Some(&update()), Some(&linked), false), UpdateOffer::Ask(update()));
}

#[test]
fn a_running_update_shows_running() {
    let linked = listed(LinkState::Linked, false);
    assert_eq!(
        offer_of(Some(&update()), Some(&linked), true),
        UpdateOffer::Running("Updating cornercase on vps…".into())
    );
}

#[test]
fn one_update_per_destination() {
    let (a, b) = (update_job("vps.invalid", "vps"), update_job("vps.invalid", "there"));
    assert_eq!(a.key(), b.key());
    assert_eq!(a.key(), "update-there:vps.invalid");
    assert_eq!(update().label(), "Update cornercase on vps…");
}

#[test]
fn the_machines_question_says_what_follows() {
    let (title, message, button) = update_question("vps", AfterUpdate::Reads, Os::Mac);
    assert_eq!(title, "Update cornercase on vps?");
    for part in ["latest release", "restarts nothing", "this Mac"] {
        assert!(message.contains(part), "{message}");
    }
    assert!(message.ends_with("The window then reads vps again."), "{message}");
    assert_eq!(button, "Update");
}

#[test]
fn the_windows_page_asks_as_before() {
    let (title, message, button) = update_question("vps", AfterUpdate::Connects, Os::Mac);
    assert_eq!(title, "Update cornercase on vps?");
    assert!(message.contains("`cornercase update`") && message.contains("restarts nothing"), "{message}");
    assert!(message.ends_with("The window then connects again."), "{message}");
    assert_eq!(button, "Update");
}

#[test]
fn a_done_update_says_cornercases_words_not_a_success() {
    let printed =
        "updated cornercase 0.13.62 → 0.13.64\nthe server keeps running 0.13.62; run `cornercase kill-server`";
    let (text, kind) = updated_words("vps", &Said::Done(printed.into()));
    assert_eq!(kind, ToastKind::Info);
    assert!(text.starts_with("vps: updated cornercase 0.13.62"), "{text}");
    assert!(text.contains("the server keeps running"), "{text}");
    let (text, _) = updated_words("vps", &Said::Done(String::new()));
    assert_eq!(text, "vps: cornercase update ended");
}

#[test]
fn a_failed_update_says_why() {
    let printed =
        "cornercase 0.13.64 is out (you have 0.13.62). Update it with:\nbrew update && brew upgrade cornercase";
    let (text, kind) = updated_words("vps", &Said::Failed(printed.into()));
    assert_eq!(kind, ToastKind::Error);
    assert!(text.starts_with("Could not update cornercase on vps: "), "{text}");
    assert!(text.ends_with("brew upgrade cornercase"), "{text}");
}
