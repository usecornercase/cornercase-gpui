use super::{
    Drive, close_label, confirm_words, drive, exposure, offers_tell, open_label, open_tooltip, opening_words,
    own_exposure, own_reverse_stays, says,
};
use crate::model::tunnels::{Gate, Older, Toward};
use crate::store::ReverseState;

fn machine() -> Toward {
    Toward::Machine("vps".to_string())
}

fn older() -> Gate {
    Gate::Older(Older::ThisComputer(crate::cli::Version::parse("0.13.60")))
}

#[test]
fn agents_on_this_computer_get_chromes_own_port() {
    assert_eq!(drive(None, Gate::Open, None, None), Drive::Here);
}

#[test]
fn a_machines_agents_are_offered_the_reverse_hold_once_both_sides_have_it() {
    assert_eq!(drive(Some(&machine()), Gate::Open, None, None), Drive::Closed);
}

#[test]
fn a_machines_agents_on_an_older_side_are_told_what_to_update() {
    let why = "needs cornercase 0.13.61 or newer on vps".to_string();
    let found = drive(Some(&machine()), older(), None, Some(why.clone()));
    assert_eq!(found, Drive::Needs(why));
    assert_ne!(drive(Some(&machine()), older(), None, None), Drive::Own);
}

#[test]
fn a_remote_windows_agents_use_its_own_ssh_below_0_13_61() {
    assert_eq!(drive(Some(&Toward::Server), older(), None, Some("x".to_string())), Drive::Own);
    assert_eq!(drive(Some(&Toward::Server), Gate::Open, None, None), Drive::Closed);
}

#[test]
fn an_opening_or_open_hold_wins_over_the_gate() {
    let opening = drive(Some(&machine()), older(), Some(ReverseState::Opening), Some("x".to_string()));
    assert_eq!(opening, Drive::Opening);
    let open = drive(Some(&machine()), Gate::Open, Some(ReverseState::Open { there: 41234 }), None);
    assert_eq!(open, Drive::Open(41234));
}

#[test]
fn the_confirmation_says_who_can_reach_the_port() {
    let (title, message, button) = confirm_words("vps");
    assert_eq!(title, "Open this Chrome to vps?");
    assert!(message.contains("every other user and program on vps"), "{message}");
    assert!(message.contains("GatewayPorts yes") && message.contains("the logins its profile holds"), "{message}");
    assert_eq!(button, "Open to vps");
}

#[test]
fn the_menu_says_what_each_state_offers() {
    assert_eq!(says(&Drive::Closed, "vps"), "Agents on vps can drive this Chrome once you open it to vps.");
    assert!(says(&Drive::Needs(String::new()), "vps").starts_with("Agents on vps cannot drive this Chrome"));
    assert_eq!(says(&Drive::Open(1), "vps"), "Agents on vps can drive this Chrome, and you see their tabs here.");
    assert_eq!(exposure("vps"), "Open to vps: every user and program there can drive it.");
    assert!(own_exposure("devbox").contains("every user and program on devbox"));
    assert_eq!(open_label("vps"), "Open this Chrome to vps…");
    assert_eq!(opening_words("vps"), "Opening this Chrome to vps…");
    assert_eq!(close_label("vps"), "Close it to vps");
    assert_eq!(open_tooltip("vps"), "Asks first: every user and program on vps could then drive it");
}

#[test]
fn the_windows_own_reverse_ssh_stops_once_the_hold_can_carry_it() {
    assert!(!own_reverse_stays(Gate::Open));
    assert!(own_reverse_stays(older()));
    assert!(own_reverse_stays(Gate::Waiting));
}

#[test]
fn tell_is_offered_only_where_the_menu_has_an_address() {
    for offered in [Drive::Here, Drive::Own, Drive::Open(41234)] {
        assert!(offers_tell(&offered), "{offered:?}");
    }
    for hidden in [Drive::Closed, Drive::Opening, Drive::Needs("needs cornercase 0.13.61".to_string())] {
        assert!(!offers_tell(&hidden), "{hidden:?}");
    }
}
