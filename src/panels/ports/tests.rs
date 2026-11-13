use std::collections::HashSet;

use super::data::{List, Port, list_command};
use super::rows;
use super::there::{Scope, Source, machine_notes, scope_of, source_of};
use super::view::{
    Go, PortView, stop_label, stop_many_warning, stop_title, stop_warning, there_key, to_stop, view_here, view_there,
};
use crate::model::tunnels::{Gate, Older};
use crate::model::{Aim, Session, ThereId};
use crate::store::Reach;
use crate::test_util::{fixture, vps_aim, vps_read, vps_target};

fn ports() -> Vec<Port> {
    serde_json::from_str::<List>(fixture!("ports/workspace.json")).expect("the ports parse").ports
}

#[test]
fn a_port_reads_its_url_program_and_pane() {
    let p = &ports()[0];
    assert_eq!((p.url.as_deref(), p.pane, p.workspace), (Some("http://localhost:61440"), Some(28), Some(22)));
    assert_eq!(p.command_line(), "python3 -m http.server 0 --bind 127.0.0.1");
}

fn view(port: &Port) -> PortView {
    view_here(port, None, false)
}

#[test]
fn stopping_names_the_port_and_the_pid() {
    assert_eq!(view(&ports()[0]).stop_command(), ["ports", "stop", "61440", "--pid", "52064"]);
}

#[test]
fn only_a_port_of_a_workspace_can_be_stopped() {
    let outside: Port = Port { workspace: None, ..Port::default() };
    assert!(!outside.stoppable());
    assert!(ports()[0].stoppable());
}

#[test]
fn the_place_says_how_the_port_belongs_to_the_workspace() {
    let by_pane = Port { pane: Some(4), via: Some("pane".into()), ..Port::default() };
    assert_eq!(by_pane.place(None, false), "pane 4");
    let by_agent = Port { pane: Some(4), via: Some("agent".into()), agent: Some("claude".into()), ..Port::default() };
    assert_eq!(by_agent.place(None, false), "agent claude · pane 4");
    let by_folder = Port { via: Some("cwd".into()), workspace: Some(2), ..Port::default() };
    assert_eq!(by_folder.place(None, false), "in the worktree, no tab");
    assert_eq!(Port::default().place(None, true), "no workspace");
}

#[test]
fn the_list_command_asks_for_the_workspace_or_every_port() {
    assert_eq!(list_command(false, Some(3)), ["ports", "--json", "--workspace", "3"]);
    assert_eq!(list_command(true, Some(3)), ["ports", "--json", "--all"]);
}

#[test]
fn the_stop_warning_names_an_agent_and_a_program_no_tab_started() {
    let p = Port { via: Some("agent".into()), pane: None, program: "vite".into(), ..Port::default() };
    let text = stop_warning(&view(&p), None);
    assert!(text.contains("An agent started it"));
    assert!(text.contains("No tab started it"));
}

fn port(number: u16, workspace: Option<u64>) -> Port {
    Port { number, pid: u64::from(number) + 1, program: "vite".into(), workspace, pane: Some(4), ..Port::default() }
}

fn views(ports: &[Port]) -> Vec<PortView> {
    ports.iter().map(view).collect()
}

fn numbers(ports: &[&PortView]) -> Vec<u16> {
    ports.iter().map(|p| p.number).collect()
}

#[test]
fn with_nothing_ticked_stop_all_takes_every_port_of_the_workspace() {
    let ports = [port(1, Some(2)), port(3, None), port(5, Some(2))];
    assert_eq!(numbers(&to_stop(&views(&ports), &HashSet::new(), false)), [1, 5]);
}

#[test]
fn the_ticked_ports_are_the_ones_stopped() {
    let ports = [port(1, Some(2)), port(5, Some(2))];
    let ticked = HashSet::from([view(&ports[1]).key]);
    assert_eq!(numbers(&to_stop(&views(&ports), &ticked, false)), [5]);
}

#[test]
fn every_port_stops_only_what_is_ticked() {
    let ports = [port(1, Some(2)), port(5, Some(3))];
    assert_eq!(to_stop(&views(&ports), &HashSet::new(), true), Vec::<&PortView>::new());
    let ticked = HashSet::from([view(&ports[0]).key]);
    assert_eq!(numbers(&to_stop(&views(&ports), &ticked, true)), [1]);
}

#[test]
fn a_port_of_no_workspace_is_never_stopped_even_ticked() {
    let ports = [port(1, None)];
    let ticked = HashSet::from([view(&ports[0]).key]);
    assert_eq!(to_stop(&views(&ports), &ticked, false), Vec::<&PortView>::new());
}

#[test]
fn the_button_says_how_many_it_stops() {
    assert_eq!(stop_label(3, false), "Stop all…");
    assert_eq!(stop_label(2, true), "Stop 2…");
}

#[test]
fn the_question_names_each_program_and_warns_once() {
    let (a, b) = (port(1, Some(2)), Port { via: Some("agent".into()), pane: None, ..port(5, Some(2)) });
    let text = stop_many_warning(&[&view(&a), &view(&b)], None);
    assert!(text.contains("vite on port 1 (pid 2), vite on port 5 (pid 6)"));
    assert!(text.contains("An agent started some of them."));
    assert!(text.contains("No tab started some of them"));
}

#[test]
fn the_ports_note_says_how_ports_there_open_by_way() {
    use crate::model::tunnels::{Older, Way};

    let held = rows::remote_notes("devbox", &Way::Cornercase);
    assert_eq!(held.len(), 1);
    assert!(held[0].contains("Tunnel forwards one") && held[0].contains("through cornercase"));
    let own = rows::remote_notes("devbox", &Way::Own(Older::Server(None)));
    assert_eq!(own.len(), 2);
    assert!(own[0].contains("through an ssh tunnel") && own[1].contains("0.13.50 or newer on devbox"));
    assert_eq!(rows::remote_notes("devbox", &Way::Waiting).len(), 1);
}

fn session() -> Session {
    serde_json::from_str(fixture!("machines/view.json")).expect("a view line")
}

fn machine_port(number: u16, pane: u64, workspace: u64) -> Port<ThereId> {
    Port {
        number,
        pid: 90,
        program: "Python".into(),
        via: Some("pane".into()),
        pane: Some(ThereId::heard(pane)),
        workspace: Some(ThereId::heard(workspace)),
        ..Port::default()
    }
}

#[test]
fn a_machines_ports_parse_with_its_own_ids() {
    let list: List<ThereId> = serde_json::from_str(fixture!("ports/machine_workspace.json")).expect("parses");
    let [port] = list.ports.as_slice() else { panic!("one port") };
    assert_eq!((port.number, port.pane, port.workspace), (8765, Some(ThereId::heard(3)), Some(ThereId::heard(2))));
    assert_eq!(port.url.as_deref(), Some("http://localhost:8765"));
}

#[test]
fn a_machine_ports_place_names_the_machines_tab_not_ours() {
    let mut session = session();
    session.projects.iter_mut().flat_map(|p| p.workspaces.iter_mut()).flat_map(|w| w.tabs.iter_mut()).for_each(|tab| {
        tab.name = "ours".into();
    });
    let seen = session.machine("vps").expect("vps");
    let there = view_there(&machine_port(8765, 1, 3), Some(seen), false);
    assert_eq!(there.place, "pane 1 · zsh");
    assert!(session.tab_of_pane(1).is_some_and(|tab| tab.name == "ours"));
}

#[test]
fn every_port_there_names_the_machines_workspace() {
    let session = session();
    let seen = session.machine("vps").expect("vps");
    let there = view_there(&machine_port(8765, 5, 7), Some(seen), true);
    assert_eq!(there.place, "shop › main · pane 5 · zsh");
    let none = view_there(&Port { workspace: None, pane: None, via: None, ..machine_port(1, 5, 7) }, Some(seen), true);
    assert_eq!(none.place, "no workspace");
}

#[test]
fn go_on_a_machines_row_aims_at_its_pane_there() {
    let session = session();
    let seen = session.machine("vps").expect("vps");
    let there = view_there(&machine_port(8765, 5, 7), Some(seen), false);
    let boot = seen.boot.clone();
    assert_eq!(there.go, Some(Go::There { machine: "vps".into(), boot, pane: ThereId::heard(5) }));
    let local = view_here(&Port { pane: Some(5), ..Port::default() }, None, false);
    assert_eq!(local.go, Some(Go::Here(5)));
    assert_eq!(view_there(&machine_port(1, 5, 7), None, false).go, None);
}

#[test]
fn the_same_port_and_pid_here_and_there_have_different_keys() {
    let here = Port { number: 8765, pid: 90, ..Port::default() };
    assert_ne!(view_here(&here, None, false).key, there_key(&machine_port(8765, 5, 7)));
}

#[test]
fn stopping_on_a_machine_names_it() {
    let there = view_there(&machine_port(8765, 5, 7), None, false);
    assert_eq!(stop_title(&there, Some("vps")), "Stop Python on port 8765 on vps?");
    assert_eq!(stop_title(&there, None), "Stop Python on port 8765?");
    assert!(stop_warning(&there, Some("vps")).starts_with("Python (pid 90) on vps gets SIGTERM"));
    assert!(stop_many_warning(&[&there], Some("vps")).contains("Python on port 8765 (pid 90) on vps"));
}

#[test]
fn a_picked_machine_place_wins_over_the_workspace_on_screen() {
    let read = vps_read(Reach::Up(vps_target()));
    assert!(matches!(source_of(Some(read), Some(3)), Source::There(_)));
    assert!(matches!(source_of(None, Some(3)), Source::Here(Some(3))));
}

#[test]
fn a_new_aim_all_or_destination_is_a_new_scope() {
    let read = vps_read(Reach::Up(vps_target()));
    let first = scope_of(&Source::There(Box::new(read.clone())), false);
    assert_eq!(first, scope_of(&Source::There(Box::new(read.clone())), false));
    assert_ne!(first, scope_of(&Source::There(Box::new(read.clone())), true));
    let aim = Aim { workspace: ThereId::heard(8), ..vps_aim() };
    assert_ne!(first, scope_of(&Source::There(Box::new(crate::store::MachineRead { aim, ..read.clone() })), false));
    let moved = crate::store::MachineRead { destination: "other".into(), ..read };
    assert_ne!(first, scope_of(&Source::There(Box::new(moved)), false));
    assert_ne!(first, Scope::Here { all: false, workspace: Some(3) });
}

#[test]
fn the_machines_notes_follow_the_gate() {
    let open = machine_notes(&Gate::Open, "vps");
    assert!(open[0].contains("Tunnel forwards a port") && open[0].contains("does not open a machine's ports"));
    let here = machine_notes(&Gate::Older(Older::ThisComputer(crate::cli::Version::parse("0.13.39"))), "vps");
    assert!(
        here[0].starts_with("Tunnel needs cornercase 0.13.50 or newer on this ")
            && here[0].contains("(it has 0.13.39)")
    );
    let there = machine_notes(&Gate::Older(Older::Server(crate::cli::Version::parse("0.13.45"))), "vps");
    assert_eq!(there, ["Tunnel needs cornercase 0.13.50 or newer on vps (its server runs 0.13.45)."]);
    assert_eq!(machine_notes(&Gate::Waiting, "vps"), Vec::<String>::new());
}
