use super::*;
use crate::cli::Remote;
use crate::model::{ThereId, ThereProject, ThereWorkspace};
use crate::platform::Os;
use crate::test_util::Change;

fn workspace(id: u64, name: &str) -> ThereWorkspace {
    ThereWorkspace {
        id: ThereId::heard(id),
        name: name.into(),
        path: format!("/home/ana/code/{name}"),
        ..ThereWorkspace::default()
    }
}

fn seen(state: LinkState) -> MachineSeen {
    let projects = vec![ThereProject {
        name: "shop".into(),
        workspaces: vec![workspace(7, "main"), workspace(9, "notes")],
        ..ThereProject::default()
    }];
    MachineSeen { name: "vps".into(), state, boot: "b1".into(), projects, ..MachineSeen::default() }
}

fn listed() -> Vec<Machine> {
    vec![Machine { name: "vps".into(), destination: "ana@vps.example.com".into(), ..Machine::default() }]
}

fn read_in(
    place: &MachinePlace,
    seen: &MachineSeen,
    window: &Target,
    list: &[Machine],
    home: Option<&str>,
) -> MachineRead {
    let destination = list.iter().find(|m| m.name == seen.name).map(|m| m.destination.clone()).unwrap_or_default();
    read_of(place, seen, (reach(window, Listing { machines: list, known: true }, seen), destination), home)
}

fn read(state: LinkState, window: &Target) -> MachineRead {
    read_seen(&seen(state), window)
}

fn read_seen(seen: &MachineSeen, window: &Target) -> MachineRead {
    let ws = seen.workspace(ThereId::heard(7)).expect("a workspace");
    read_in(&MachinePlace::at(seen, ws, None), seen, window, &listed(), Some("/home/ana"))
}

const TRIES_AT: &str = "1970-01-01T00:16:44.139Z";
const NOW: i64 = 1_000;

fn other_server() -> Target {
    Target::Remote(Remote::new("devbox", "").expect("a destination"))
}

#[test]
fn a_linked_machine_is_read_at_its_own_destination() {
    let read = read(LinkState::Linked, &Target::Local);
    let target = read.reach.target().expect("reached");
    assert_eq!(target.flags(), ["--remote", "ana@vps.example.com"]);
}

#[test]
fn a_machine_down_has_nothing_to_ask_with_and_comes_back_by_itself() {
    let read = read(LinkState::Unreachable, &Target::Local);
    assert_eq!(read.reach, Reach::Down("vps unreachable".into()));
    assert!(read.reach.target().is_none());
}

#[test]
fn a_machine_connecting_counts_as_coming_back() {
    assert!(matches!(read(LinkState::Connecting, &Target::Local).reach, Reach::Down(_)));
}

#[test]
fn an_older_machine_waits_for_someone_rather_than_coming_back() {
    let read = read(LinkState::Older, &Target::Local);
    assert_eq!(read.reach, Reach::Refused("vps runs an older cornercase".into()));
}

#[test]
fn a_window_on_another_server_never_reads_a_machine_even_when_it_is_linked() {
    let read = read(LinkState::Linked, &other_server());
    assert!(read.reach.target().is_none());
    assert!(matches!(read.reach, Reach::Refused(_)));
}

#[test]
fn a_window_on_another_server_does_not_promise_to_reconnect_to_a_machine_that_is_down() {
    assert!(matches!(read(LinkState::Unreachable, &other_server()).reach, Reach::Refused(_)));
}

#[test]
fn a_read_names_the_place_and_the_folder_as_that_machine_writes_it() {
    let read = read(LinkState::Linked, &Target::Local);
    assert_eq!(read.place, "shop › main");
    assert_eq!(read.path, "/home/ana/code/main");
    assert_eq!(read.folder, "~/code/main");
    assert_eq!(read.aim.workspace_arg(), "7");
}

#[test]
fn without_the_machines_home_the_folder_shows_whole() {
    let seen = seen(LinkState::Linked);
    let ws = seen.workspace(ThereId::heard(7)).expect("a workspace");
    let read = read_in(&MachinePlace::at(&seen, ws, None), &seen, &Target::Local, &listed(), None);
    assert_eq!(read.folder, "/home/ana/code/main");
}

fn place_then(later: &MachineSeen) -> MachineRead {
    let picked = seen(LinkState::Linked);
    let ws = picked.workspace(ThereId::heard(7)).expect("a workspace");
    read_in(&MachinePlace::at(&picked, ws, None), later, &Target::Local, &listed(), None)
}

#[test]
fn a_place_the_snapshot_no_longer_lists_keeps_its_own_name() {
    let emptied = MachineSeen { projects: Vec::new(), ..seen(LinkState::Connecting) };
    assert_eq!(place_then(&emptied).place, "main");
}

#[test]
fn the_numbers_of_another_run_do_not_name_a_place_kept_while_the_link_is_down() {
    let projects = vec![ThereProject {
        name: "other".into(),
        workspaces: vec![workspace(7, "another")],
        ..ThereProject::default()
    }];
    let restarted = MachineSeen { boot: "b2".into(), projects, ..seen(LinkState::Unreachable) };
    assert_eq!(place_then(&restarted).place, "main");
}

#[test]
fn a_read_names_the_destination_it_goes_through_also_while_the_machine_is_down() {
    assert_eq!(read(LinkState::Linked, &Target::Local).destination, "ana@vps.example.com");
    assert_eq!(read(LinkState::Unreachable, &Target::Local).destination, "ana@vps.example.com");
}

#[test]
fn a_machine_gone_from_the_list_has_no_destination() {
    let seen = seen(LinkState::Linked);
    let ws = seen.workspace(ThereId::heard(7)).expect("a workspace");
    let read = read_in(&MachinePlace::at(&seen, ws, None), &seen, &Target::Local, &[], None);
    assert_eq!(read.destination, "");
}

#[test]
fn a_read_of_a_link_that_fell_says_when_it_tries_next() {
    let fell = MachineSeen { retry_at: Some(TRIES_AT.into()), ..seen(LinkState::Unreachable) };
    assert_eq!(read_seen(&fell, &Target::Local).next_try(NOW), Some(NextTry::In(4)));
}

#[test]
fn a_read_of_a_linked_machine_promises_no_try() {
    assert_eq!(read(LinkState::Linked, &Target::Local).next_try(NOW), None);
}

fn tab(down: Option<&str>) -> TabThere {
    TabThere {
        machine: "vps".into(),
        boot: "b1".into(),
        target: Target::Local,
        down: down.map(str::to_string),
        agents: vec![5],
    }
}

fn reach_of(tab: Option<&TabThere>, machine: &str, boot: &str, pane: u64) -> AgentReach {
    reach_agent(tab, machine, boot, pane)
}

#[test]
fn an_agent_on_a_live_tab_can_be_read() {
    assert_eq!(reach_of(Some(&tab(None)), "vps", "b1", 5), AgentReach::Ready(Target::Local));
}

#[test]
fn a_pane_that_runs_no_agent_or_is_gone_reads_as_gone() {
    let gone = AgentReach::Gone("that agent is no longer on vps".into());
    assert_eq!(reach_of(Some(&tab(None)), "vps", "b1", 6), gone);
    assert_eq!(reach_of(Some(&tab(None)), "vps", "b1", 99), gone);
}

#[test]
fn a_restart_or_another_tab_reads_as_gone() {
    let live = tab(None);
    assert_eq!(reach_of(Some(&live), "vps", "b0", 5), AgentReach::Gone("vps restarted".into()));
    let none = AgentReach::Gone("The tab on vps is no longer shown".into());
    assert_eq!(reach_of(None, "vps", "b1", 5), none);
    assert_eq!(reach_of(Some(&live), "mini", "b1", 5), AgentReach::Gone("The tab on mini is no longer shown".into()));
}

#[test]
fn a_tab_that_is_down_reads_as_down_even_when_the_agent_is_listed() {
    let down = tab(Some("vps unreachable"));
    assert_eq!(reach_of(Some(&down), "vps", "b1", 5), AgentReach::Down("vps unreachable".into()));
}

fn unread() -> Listing<'static> {
    Listing { machines: &[], known: false }
}

#[test]
fn a_linked_machine_waits_while_the_machines_list_is_not_read() {
    let got = reach(&Target::Local, unread(), &seen(LinkState::Linked));
    assert_eq!(got, Reach::Waiting("waiting for the machines list".into()));
    assert!(got.target().is_none());
}

#[test]
fn a_machine_left_out_of_a_read_list_is_refused_as_before() {
    let list = Listing { machines: &[], known: true };
    let got = reach(&Target::Local, list, &seen(LinkState::Linked));
    assert_eq!(got, Reach::Refused("vps is no longer listed".into()));
}

#[test]
fn a_machine_down_or_an_older_link_reads_as_before_while_the_list_is_not_read() {
    let down = reach(&Target::Local, unread(), &seen(LinkState::Unreachable));
    assert_eq!(down, Reach::Down("vps unreachable".into()));
    let older = reach(&Target::Local, unread(), &seen(LinkState::Older));
    assert_eq!(older, Reach::Refused("vps runs an older cornercase".into()));
}

#[test]
fn a_window_on_another_server_is_refused_also_while_the_list_is_not_read() {
    let got = reach(&other_server(), unread(), &seen(LinkState::Linked));
    assert!(matches!(got, Reach::Refused(_)), "{got:?}");
}

fn refusal_of(there: Option<&str>, update: &[&str]) -> Older {
    Older {
        destination: "vps.invalid".into(),
        there: there.map(str::to_string),
        here: "0.13.57".into(),
        update: update.iter().map(|word| (*word).to_string()).collect(),
    }
}

#[test]
fn an_older_note_names_both_releases_and_the_update() {
    let older = refusal_of(Some("0.13.39"), &["cornercase", "--remote", "vps.invalid", "update"]);
    let mac = older_note("vps", &older, Os::Mac);
    assert_eq!(mac.line, "vps runs cornercase 0.13.39 and this Mac runs 0.13.57: update it");
    assert_eq!(mac.tip, "Update it from this Mac with cornercase --remote vps.invalid update");
    let linux = older_note("vps", &older, Os::Linux);
    assert_eq!(linux.line, "vps runs cornercase 0.13.39 and this computer runs 0.13.57: update it");
    assert_eq!(
        mac.tunnels,
        "vps runs cornercase 0.13.39 and this Mac runs 0.13.57: nothing new is tunnelled to vps until it is updated"
    );
    assert!(linux.tunnels.contains("this computer runs 0.13.57"), "{}", linux.tunnels);
    let update = UpdateThere {
        machine: "vps".into(),
        destination: "vps.invalid".into(),
        args: ["--remote", "vps.invalid", "update"].map(String::from).to_vec(),
    };
    assert_eq!(mac.update.as_deref(), Some(&update));
}

#[test]
fn a_refusal_without_update_words_offers_no_update() {
    for words in [&[][..], &["sh", "update"][..]] {
        assert_eq!(older_note("vps", &refusal_of(Some("0.13.39"), words), Os::Mac).update, None);
    }
}

#[test]
fn a_cornercase_too_old_to_say_its_release_is_named_so() {
    let note = older_note("vps", &refusal_of(None, &[]), Os::Mac);
    assert_eq!(
        note.line,
        "vps runs a cornercase too old to say which release it is, and this Mac runs 0.13.57: update it"
    );
    assert_eq!(note.tip, "Update cornercase on vps from this Mac");
}

fn facts_of(seen: &MachineSeen) -> Facts {
    Facts::of(seen, Some(&listed()[0]), Version::parse("0.13.57"))
}

fn kept() -> KeptOlder {
    KeptOlder { facts: facts_of(&seen(LinkState::Linked)), older: refusal_of(Some("0.13.39"), &[]) }
}

fn up() -> Reach {
    reach(&Target::Local, Listing { machines: &listed(), known: true }, &seen(LinkState::Linked))
}

#[test]
fn a_kept_refusal_turns_a_linked_machine_older() {
    let facts = facts_of(&seen(LinkState::Linked));
    let got = kept_reach(up(), Some(&kept()), &facts, "vps");
    assert_eq!(got, Reach::Older(older_note("vps", &kept().older, Os::CURRENT)));
    assert!(got.target().is_none());
}

#[test]
fn a_kept_refusal_stops_holding_once_its_facts_change() {
    let changes: [Change<Facts>; 6] = [
        ("boot", |f| f.boot = "b2".into()),
        ("state", |f| f.state = LinkState::Connecting),
        ("version", |f| f.version = Some("0.13.57".into())),
        ("server version", |f| f.server_version = Some("0.13.57".into())),
        ("destination", |f| f.destination = Some("other".into())),
        ("this Mac's release", |f| f.here = Version::parse("0.13.58")),
    ];
    for (name, change) in changes {
        let mut facts = facts_of(&seen(LinkState::Linked));
        change(&mut facts);
        assert_eq!(kept_reach(up(), Some(&kept()), &facts, "vps"), up(), "{name}");
    }
}

#[test]
fn a_kept_refusal_never_hides_a_machine_that_is_down_refused_or_waiting() {
    let facts = facts_of(&seen(LinkState::Linked));
    for other in [Reach::Down("d".into()), Reach::Refused("r".into()), Reach::Waiting("w".into())] {
        assert_eq!(kept_reach(other.clone(), Some(&kept()), &facts, "vps"), other);
    }
}

#[test]
fn a_probe_is_sent_once_and_not_again_within_15_s_unless_the_facts_changed() {
    let facts = facts_of(&seen(LinkState::Linked));
    let other = Facts { boot: "b2".into(), ..facts.clone() };
    let at = Instant::now();
    let answered = Check::Answered { facts: facts.clone(), at };
    assert!(may_check(&up(), None, &facts, at));
    assert!(!may_check(&up(), Some(&Check::InFlight), &facts, at));
    assert!(!may_check(&up(), Some(&answered), &facts, at + Duration::from_secs(14)));
    assert!(may_check(&up(), Some(&answered), &facts, at + Duration::from_secs(15)));
    assert!(may_check(&up(), Some(&answered), &other, at));
}

#[test]
fn nothing_is_probed_while_a_machine_is_older_down_refused_or_waiting() {
    let facts = facts_of(&seen(LinkState::Linked));
    let older = Reach::Older(older_note("vps", &refusal_of(None, &[]), Os::Mac));
    for reach in [older, Reach::Down("d".into()), Reach::Refused("r".into()), Reach::Waiting("w".into())] {
        assert!(!may_check(&reach, None, &facts, Instant::now()), "{reach:?}");
    }
}

#[test]
fn the_home_is_read_from_what_status_says_there() {
    assert_eq!(home_in(r#"{"home": "/home/ana"}"#).as_deref(), Some("/home/ana"));
    assert_eq!(home_in(r#"{"boot": "b"}"#), None);
    assert_eq!(home_in("not json"), None);
}

fn answer(homes: &mut Homes, machine: &str, boot: &str, home: &str) {
    assert!(homes.ask(machine, boot));
    homes.answered(machine, boot, Some(home.into()));
}

#[test]
fn a_machine_keeps_one_home_however_often_its_server_restarts() {
    let mut homes = Homes::default();
    for boot in ["b1", "b2", "b3"] {
        answer(&mut homes, "vps", boot, "/home/ana");
    }
    assert_eq!(homes.home("vps", "b3"), Some("/home/ana"));
    assert_eq!((homes.home("vps", "b1"), homes.home("vps", "b2")), (None, None));
    assert_eq!(homes.known.len(), 1);
    assert!(!homes.ask("vps", "b3"));
}

#[test]
fn a_run_is_asked_once_until_it_answers() {
    let mut homes = Homes::default();
    assert!(homes.ask("vps", "b1"));
    assert!(!homes.ask("vps", "b1"));
    homes.answered("vps", "b1", None);
    assert!(homes.ask("vps", "b1"));
}

#[test]
fn an_answer_for_a_run_already_left_is_dropped() {
    let mut homes = Homes::default();
    assert!(homes.ask("vps", "b1"));
    assert!(homes.ask("vps", "b2"));
    homes.answered("vps", "b1", Some("/home/ana".into()));
    assert_eq!((homes.home("vps", "b1"), homes.home("vps", "b2")), (None, None));
    assert!(!homes.ask("vps", "b2"));
}

#[test]
fn each_machine_keeps_its_own_home() {
    let mut homes = Homes::default();
    answer(&mut homes, "vps", "b1", "/home/ana");
    answer(&mut homes, "lab", "c1", "/home/bo");
    assert_eq!((homes.home("vps", "b1"), homes.home("lab", "c1")), (Some("/home/ana"), Some("/home/bo")));
}

#[test]
fn a_home_is_never_read_for_another_machine_with_the_same_boot() {
    let mut homes = Homes::default();
    answer(&mut homes, "vps", "b1", "/home/ana");
    assert_eq!(homes.home("lab", "b1"), None);
    assert_eq!(homes.home("vps", "b1"), Some("/home/ana"));
}

#[test]
fn a_refusal_is_kept_only_for_the_facts_the_request_started_under() {
    let asked = facts_of(&seen(LinkState::Linked));
    let restarted = Facts { boot: "b2".into(), ..asked.clone() };
    let updated = Facts { here: Version::parse("0.13.58"), ..asked.clone() };
    assert_eq!(still_current(Some(asked.clone()), Some(&asked)), Some(asked.clone()));
    assert_eq!(still_current(Some(asked.clone()), Some(&restarted)), None);
    assert_eq!(still_current(Some(asked.clone()), Some(&updated)), None);
    assert_eq!(still_current(Some(asked), None), None);
    assert_eq!(still_current(None, Some(&facts_of(&seen(LinkState::Linked)))), None);
}

#[test]
fn a_probe_refused_as_older_after_the_facts_moved_ends_its_wait_and_keeps_nothing() {
    let asked = facts_of(&seen(LinkState::Linked));
    let now = Facts { boot: "b2".into(), ..asked.clone() };
    let mut state = ThereState::default();
    state.checks.insert("vps".into(), Check::InFlight);
    state.probe_answered("vps", (asked.clone(), Some(refusal_of(Some("0.13.39"), &[]))), Some(&now));
    assert!(state.older.is_empty());
    assert!(may_check(&up(), state.checks.get("vps"), &now, Instant::now()));
    assert!(!may_check(&up(), Some(&Check::InFlight), &now, Instant::now()));
}

#[test]
fn a_probe_refused_as_older_under_the_current_facts_keeps_the_refusal() {
    let facts = facts_of(&seen(LinkState::Linked));
    let mut state = ThereState::default();
    state.probe_answered("vps", (facts.clone(), Some(refusal_of(Some("0.13.39"), &[]))), Some(&facts));
    assert_eq!(state.older.get("vps").map(|kept| &kept.facts), Some(&facts));
    state.probe_answered("vps", (facts.clone(), None), Some(&facts));
    assert!(state.older.is_empty());
}

#[test]
fn a_recheck_of_a_kept_refusal_keeps_it_and_asks_for_a_probe() {
    let facts = facts_of(&seen(LinkState::Linked));
    let mut state = ThereState::default();
    state.probe_answered("vps", (facts.clone(), Some(refusal_of(Some("0.13.39"), &[]))), Some(&facts));
    assert!(state.recheck("vps"));
    assert!(state.older.contains_key("vps"));
    assert!(!state.checks.contains_key("vps"));
}

#[test]
fn a_recheck_without_a_refusal_only_forgets_the_last_check() {
    let facts = facts_of(&seen(LinkState::Linked));
    let mut state = ThereState::default();
    state.checks.insert("vps".into(), Check::Answered { facts, at: Instant::now() });
    assert!(!state.recheck("vps"));
    assert!(!state.checks.contains_key("vps"));
}

#[test]
fn a_recheck_while_a_probe_is_in_flight_sends_nothing_more() {
    let facts = facts_of(&seen(LinkState::Linked));
    let mut state = ThereState::default();
    state.probe_answered("vps", (facts.clone(), Some(refusal_of(Some("0.13.39"), &[]))), Some(&facts));
    state.checks.insert("vps".into(), Check::InFlight);
    assert!(!state.recheck("vps"));
    assert_eq!(state.checks.get("vps"), Some(&Check::InFlight));
}

#[test]
fn a_probe_says_whether_the_refusal_changed() {
    let facts = facts_of(&seen(LinkState::Linked));
    let mut state = ThereState::default();
    let refused = || Some(refusal_of(Some("0.13.39"), &[]));
    assert!(!state.probe_answered("vps", (facts.clone(), None), Some(&facts)));
    assert!(state.probe_answered("vps", (facts.clone(), refused()), Some(&facts)));
    assert!(!state.probe_answered("vps", (facts.clone(), refused()), Some(&facts)));
    assert!(state.probe_answered("vps", (facts.clone(), None), Some(&facts)));
}
