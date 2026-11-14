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
    let (text, kind) = updated_words("vps", &UpdateAnswer::Printed(Said::Done(printed.into())));
    assert_eq!(kind, ToastKind::Info);
    assert!(text.starts_with("vps: updated cornercase 0.13.62"), "{text}");
    assert!(text.contains("the server keeps running"), "{text}");
    let (text, _) = updated_words("vps", &UpdateAnswer::Printed(Said::Done(String::new())));
    assert_eq!(text, "vps: cornercase update ended");
}

#[test]
fn a_failed_update_says_why() {
    let printed =
        "cornercase 0.13.64 is out (you have 0.13.62). Update it with:\nbrew update && brew upgrade cornercase";
    let (text, kind) = updated_words("vps", &UpdateAnswer::Printed(Said::Failed(printed.into())));
    assert_eq!(kind, ToastKind::Error);
    assert!(text.starts_with("Could not update cornercase on vps: "), "{text}");
    assert!(text.ends_with("brew upgrade cornercase"), "{text}");
}

fn updated() -> UpdateAnswer {
    UpdateAnswer::Updated { from: "0.13.53".into(), version: "0.13.74".into() }
}

fn by_hand() -> UpdateAnswer {
    UpdateAnswer::Manual(ByHand {
        version: "0.13.53".into(),
        latest: "0.13.74".into(),
        command: "brew update && brew upgrade cornercase".into(),
    })
}

#[test]
fn an_install_there_names_the_version_on_disk_and_its_server() {
    let (text, kind) = updated_words("vps", &updated());
    assert_eq!(
        text,
        "cornercase 0.13.74 is installed on vps, in place of 0.13.53; \
         its server keeps running the release it started with until it restarts there"
    );
    assert_eq!(kind, ToastKind::Done);
}

#[test]
fn an_up_to_date_machine_says_its_release() {
    let (text, kind) = updated_words("vps", &UpdateAnswer::UpToDate { version: "0.13.74".into() });
    assert_eq!(text, "cornercase on vps is already 0.13.74, the latest release");
    assert_eq!(kind, ToastKind::Info);
}

#[test]
fn a_manual_answer_there_gives_the_command_to_run_there() {
    let (text, kind) = updated_words("vps", &by_hand());
    assert_eq!(
        text,
        "cornercase 0.13.53 on vps cannot update itself: run `brew update && brew upgrade cornercase` there to install 0.13.74"
    );
    assert_eq!(kind, ToastKind::NeedsYou);
}

#[test]
fn a_refused_json_there_is_a_failure() {
    let (text, kind) = updated_words("vps", &UpdateAnswer::Refused);
    assert_eq!(
        text,
        "Could not update cornercase on vps: cornercase refused update --json, which needs cornercase 0.13.53 or newer"
    );
    assert_eq!(kind, ToastKind::Error);
}

#[test]
fn the_older_page_shows_the_same_words_and_connects_again_only_when_it_ended() {
    let words = |answer: &UpdateAnswer| updated_words("vps.invalid", answer).0;
    let (said, ended) = update_page("vps.invalid", &updated());
    assert_eq!((said, ended), (Said::Done(words(&updated())), true));
    let up = UpdateAnswer::UpToDate { version: "0.13.74".into() };
    assert_eq!(update_page("vps.invalid", &up), (Said::Done(words(&up)), true));
    assert_eq!(update_page("vps.invalid", &by_hand()), (Said::Done(words(&by_hand())), false));
    let refused = update_page("vps.invalid", &UpdateAnswer::Refused);
    assert_eq!(refused, (Said::Failed(words(&UpdateAnswer::Refused)), false));
    let failed = UpdateAnswer::Printed(Said::Failed("no".into()));
    assert_eq!(update_page("vps.invalid", &failed), (Said::Failed("no".into()), false));
    let done = UpdateAnswer::Printed(Said::Done("updated".into()));
    assert_eq!(update_page("vps.invalid", &done), (Said::Done("updated".into()), true));
}
