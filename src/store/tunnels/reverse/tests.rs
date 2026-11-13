use super::{
    ReverseState, closed_toast, ended_toast, failed_toast, opened_toast, reverse_reason, reverse_stays, stopped_toast,
};
use crate::model::tunnels::{Gate, Older, Toward};
use crate::store::MachineLink;
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
fn a_reverse_hold_ends_when_its_machines_link_is_lost_and_waits_with_it() {
    let up = |destination: &str| MachineLink::Up { boot: "b".to_string(), destination: destination.to_string() };
    assert!(!reverse_stays(&MachineLink::Lost, "vps.invalid"));
    assert!(reverse_stays(&MachineLink::Waiting(Some("vps.invalid".to_string())), "vps.invalid"));
    assert!(reverse_stays(&up("vps.invalid"), "vps.invalid"));
    assert!(!reverse_stays(&up("other.invalid"), "vps.invalid"));
    let older = Gate::Older(Older::Server(crate::cli::Version::parse("0.13.60")));
    assert!(!reverse_stays(&up("vps.invalid").gated(older), "vps.invalid"));
    assert_eq!(ReverseState::Opening, ReverseState::Opening);
}
