use super::{
    ReverseState, address_there, approved_destination, closed_toast, copied_words, ended_toast, ending_toast,
    failed_toast, opened_toast, reverse_reason, stopped_toast,
};
use crate::model::tunnels::Toward;
use crate::store::ToastKind;
use crate::test_util::fixture;

#[test]
fn the_reverse_holds_toasts_name_the_machine_and_the_address() {
    assert_eq!(opened_toast("vps", 41234), "Opened this Chrome to vps: its agents reach it at http://127.0.0.1:41234");
    assert_eq!(failed_toast("vps", "boom"), "Could not open this Chrome to vps: boom");
    assert_eq!(ended_toast("vps", None), "This Chrome is no longer open to vps");
    assert_eq!(ended_toast("vps", Some("ssh stopped")), "This Chrome is no longer open to vps: ssh stopped");
    assert_eq!(closed_toast("vps"), "Closed this Chrome to vps");
    assert_eq!(stopped_toast("vps"), "This Chrome is no longer open to vps: it stopped");
}

#[test]
fn a_reverse_holds_reason_hides_the_destination_and_drops_its_lead() {
    let machine = Toward::Machine("vps".to_string());
    let failed = reverse_reason(fixture!("tunnels/reverse_failed.txt"), &machine, "vps", "vps.invalid");
    assert_eq!(failed, "remote port forwarding failed for listen port 8111");
    let ended = reverse_reason(fixture!("tunnels/reverse_ended.txt"), &machine, "vps", "vps.invalid");
    assert_eq!(ended, "ssh stopped");
    let hidden = reverse_reason(
        "Error: could not hold a reverse tunnel to port 1: connect to vps.invalid refused",
        &machine,
        "vps",
        "vps.invalid",
    );
    assert_eq!(hidden, "connect to vps refused");
}

#[test]
fn the_address_on_the_machine_is_one_text_for_the_toast_the_menu_and_the_copy() {
    assert_eq!(address_there(41234), "http://127.0.0.1:41234");
    assert!(opened_toast("vps", 41234).ends_with(&address_there(41234)));
    assert_eq!(copied_words("vps"), "Copied the DevTools address on vps");
}

#[test]
fn a_hold_that_ends_before_it_is_up_says_why_or_its_status() {
    let say = |code, why| ending_toast(ReverseState::Opening, code, why, "vps");
    let (text, kind) = say(Some(0), "");
    assert_eq!(text, "Could not open this Chrome to vps: cornercase tunnel hold ended with status 0");
    assert_eq!(kind, ToastKind::Error);
    assert_eq!(say(None, "").0, "Could not open this Chrome to vps: cornercase tunnel hold was stopped");
    assert_eq!(say(Some(1), "ssh stopped").0, "Could not open this Chrome to vps: ssh stopped");
}

#[test]
fn a_hold_that_ends_once_it_is_up_is_told_by_its_exit() {
    let open = ReverseState::Open { there: 41234 };
    let (text, kind) = ending_toast(open, Some(0), "", "vps");
    assert_eq!((text.as_str(), kind), ("This Chrome is no longer open to vps", ToastKind::Info));
    let (text, kind) = ending_toast(open, Some(1), "ssh stopped", "vps");
    assert_eq!((text.as_str(), kind), ("This Chrome is no longer open to vps: ssh stopped", ToastKind::Error));
    let (text, _) = ending_toast(open, Some(1), "", "vps");
    assert_eq!(text, "This Chrome is no longer open to vps: cornercase tunnel hold ended with status 1");
}

#[test]
fn a_machine_whose_address_changed_while_the_question_was_open_is_not_opened_to() {
    assert_eq!(approved_destination("vps.invalid", "vps.invalid"), Ok(()));
    let refused = approved_destination("vps.invalid", "other.invalid").expect_err("a different address");
    assert_eq!(refused, "The machine's address changed since you were asked");
}
