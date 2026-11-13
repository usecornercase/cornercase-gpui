use super::*;
use crate::model::tunnels::Older;
use crate::store::OlderNote;
use crate::test_util::vps_target;

fn up() -> Reach {
    Reach::Up(vps_target())
}

fn older_reach() -> Reach {
    Reach::Older(OlderNote { line: "l".into(), tip: "t".into() })
}

#[test]
fn a_machines_note_says_open_shows_it_in_the_browser_and_follows_the_gate() {
    let open = machine_notes(&Gate::Open, "vps", &up());
    assert!(
        open[0].starts_with("These ports are on vps: Tunnel forwards one to this ")
            && open[0].contains("Open shows it in the Browser panel")
    );
    let here = machine_notes(&Gate::Older(Older::ThisComputer(crate::cli::Version::parse("0.13.39"))), "vps", &up());
    assert!(
        here[0].starts_with("Tunnel needs cornercase 0.13.50 or newer on this ")
            && here[0].contains("(it has 0.13.39)")
    );
    let there = machine_notes(&Gate::Older(Older::Server(crate::cli::Version::parse("0.13.45"))), "vps", &up());
    assert_eq!(there, ["Tunnel needs cornercase 0.13.50 or newer on vps (its server runs 0.13.45)."]);
    assert_eq!(machine_notes(&Gate::Waiting, "vps", &up()), Vec::<String>::new());
}

#[test]
fn a_refused_machine_offers_no_tunnel_note() {
    assert_eq!(machine_notes(&Gate::Open, "vps", &Reach::Refused("r".into())), Vec::<String>::new());
    let older = Gate::Older(Older::Server(None));
    assert_eq!(machine_notes(&older, "vps", &Reach::Refused("r".into())), Vec::<String>::new());
}

#[test]
fn a_machine_refused_as_older_offers_no_cornercase_note_but_the_gate_still_speaks() {
    assert_eq!(machine_notes(&Gate::Open, "vps", &older_reach()), Vec::<String>::new());
    assert_eq!(machine_notes(&Gate::Waiting, "vps", &older_reach()), Vec::<String>::new());
    let older = Gate::Older(Older::Server(crate::cli::Version::parse("0.13.45")));
    assert_eq!(
        machine_notes(&older, "vps", &older_reach()),
        ["Tunnel needs cornercase 0.13.50 or newer on vps (its server runs 0.13.45)."]
    );
}
