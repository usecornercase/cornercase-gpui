use super::*;
use crate::model::tunnels::Older;

#[test]
fn a_machines_note_says_open_shows_it_in_the_browser_and_follows_the_gate() {
    let open = machine_notes(&Gate::Open, "vps", false);
    assert!(
        open[0].starts_with("These ports are on vps: Tunnel forwards one to this ")
            && open[0].contains("Open shows it in the Browser panel")
    );
    let here = machine_notes(&Gate::Older(Older::ThisComputer(crate::cli::Version::parse("0.13.39"))), "vps", false);
    assert!(
        here[0].starts_with("Tunnel needs cornercase 0.13.50 or newer on this ")
            && here[0].contains("(it has 0.13.39)")
    );
    let there = machine_notes(&Gate::Older(Older::Server(crate::cli::Version::parse("0.13.45"))), "vps", false);
    assert_eq!(there, ["Tunnel needs cornercase 0.13.50 or newer on vps (its server runs 0.13.45)."]);
    assert_eq!(machine_notes(&Gate::Waiting, "vps", false), Vec::<String>::new());
}

#[test]
fn a_refused_machine_offers_no_tunnel_note() {
    assert_eq!(machine_notes(&Gate::Open, "vps", true), Vec::<String>::new());
    let older = Gate::Older(Older::Server(None));
    assert_eq!(machine_notes(&older, "vps", true), Vec::<String>::new());
}
