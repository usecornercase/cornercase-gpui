use super::*;
use crate::model::PaneInfo;
use crate::store::args;
use crate::test_util::fixture;

const SEEN: &str = "2026-11-06T01:23:55.000Z";
const TRY_IN_4_S: &str = "2026-11-06T01:25:59.139Z";

fn session() -> Session {
    serde_json::from_str(fixture!("machines/view.json")).expect("a view line")
}

fn vps(session: &Session) -> &MachineSeen {
    session.machine("vps").expect("vps sent its workspaces")
}

fn named<'a>(seen: &'a MachineSeen, name: &str) -> &'a ThereWorkspace {
    seen.workspaces().find(|ws| ws.name == name).expect("a workspace of that name")
}

fn agent_tab(status: &str) -> Tab<ThereId> {
    let pane =
        PaneInfo { id: ThereId(15), agent: Some("claude".into()), status: Some(status.into()), ..PaneInfo::default() };
    Tab { id: ThereId(16), panes: vec![pane], ..Tab::default() }
}

fn down(seen: Option<&str>) -> MachineSeen {
    MachineSeen {
        name: "vps".into(),
        state: LinkState::Unreachable,
        seen: seen.map(str::to_string),
        ..MachineSeen::default()
    }
}

fn now() -> i64 {
    crate::panels::flows::model::epoch(SEEN).expect("a time") + 120
}

#[test]
fn the_view_line_carries_each_machine_with_its_state_and_boot() {
    let session = session();
    let seen = vps(&session);
    assert_eq!((seen.icon, seen.state), (MachineIcon::Server, LinkState::Linked));
    assert_eq!(seen.boot, "200-1793933863758856668");
    assert!(seen.seen.is_some());
    let names: Vec<&str> = seen.projects.iter().map(|there| there.name.as_str()).collect();
    assert_eq!(names, ["work", "shop", "solo"]);
}

#[test]
fn a_line_of_a_server_with_no_machines_has_none() {
    let session: Session = serde_json::from_str(r#"{"projects": []}"#).expect("a view line");
    assert_eq!(session.machines.len(), 0);
    assert_eq!(session.alone().count(), 0);
}

#[test]
fn a_machines_tab_reads_its_agent_as_a_tab_of_ours_does() {
    let session = session();
    let notes = named(vps(&session), "notes");
    let tab = notes.tabs.iter().find(|tab| tab.agent_pane().is_some()).expect("a tab with an agent");
    assert_eq!(tab.loudest(), Some("working"));
    assert_eq!(tab.id, ThereId(16));
    assert_eq!(notes.custom_name.as_deref(), Some("notes"));
}

#[test]
fn a_worktree_there_says_so() {
    let session = session();
    let seen = vps(&session);
    assert!(named(seen, "feat/rows").worktree);
    assert!(!named(seen, "notes").worktree);
}

#[test]
fn a_workspace_lists_under_the_project_here_it_pairs_with_and_only_there() {
    let session = session();
    let shop = session.projects.iter().find(|p| p.name == "shop").expect("shop here");
    let under: Vec<&str> = session.paired_with(shop.id).map(|(_, ws)| ws.name.as_str()).collect();
    assert_eq!(under, ["main", "notes", "feat/rows"]);
    let work = session.projects.iter().find(|p| p.name == "work").expect("work here");
    assert_eq!(session.paired_with(work.id).count(), 0);
}

#[test]
fn a_project_with_no_pair_stands_alone() {
    let session = session();
    let alone: Vec<&str> = session.alone().map(|(_, there)| there.name.as_str()).collect();
    assert_eq!(alone, ["work", "solo"]);
}

#[test]
fn a_pair_that_names_no_project_here_stands_alone_too() {
    let mut session = session();
    session.projects.retain(|p| p.name != "shop");
    let alone: Vec<&str> = session.alone().map(|(_, there)| there.name.as_str()).collect();
    assert_eq!(alone, ["work", "shop", "solo"]);
}

#[test]
fn the_same_numbers_on_two_servers_stay_two_workspaces() {
    let session = session();
    let (project, ours) = session.workspace(7).expect("workspace 7 here");
    assert_eq!((project.path.as_str(), ours.path.as_str()), ("/home/me/code/shop", "/home/me/code/shop"));
    let theirs = vps(&session).workspace(ThereId(7)).expect("workspace 7 there");
    assert_eq!(theirs.path, "/home/ana/code/shop");
}

#[test]
fn a_projects_count_counts_a_machines_workspaces() {
    let session = session();
    let shop = session.projects.iter().find(|p| p.name == "shop").expect("shop here");
    assert_eq!(session.workspaces_under(shop), 4);
}

#[test]
fn a_projects_mark_is_the_loudest_of_ours_and_theirs() {
    let mut session = session();
    let shop = session.projects.iter().find(|p| p.name == "shop").expect("shop here").clone();
    assert_eq!(session.attention_under(&shop), None);
    let notes = session.machines[0].projects[1].workspaces.iter_mut().find(|ws| ws.name == "notes").expect("notes");
    notes.tabs = vec![agent_tab("waiting")];
    assert_eq!(session.attention_under(&shop), Some("waiting"));
}

#[test]
fn an_agent_working_there_does_not_mark_the_project() {
    let session = session();
    let shop = session.projects.iter().find(|p| p.name == "shop").expect("shop here");
    assert_eq!(named(vps(&session), "notes").loudest(), Some("working"));
    assert_eq!(session.attention_under(shop), None);
}

#[test]
fn an_agent_on_a_linked_machine_keeps_the_orbits_turning() {
    let session = session();
    assert!(session.anyone_working());
}

#[test]
fn an_agent_on_a_machine_that_is_down_lets_them_rest() {
    let mut session = session();
    session.machines[0].state = LinkState::Unreachable;
    assert!(!session.anyone_working());
}

#[test]
fn a_rename_names_the_id_and_takes_any_text_as_the_name() {
    let aim = Aim { machine: "vps".into(), boot: "b".into(), workspace: ThereId(7) };
    assert_eq!(aim.rename_args("-x y"), ["rename", "--workspace", "7", "--boot", "b", "--", "-x y"]);
}

#[test]
fn a_close_names_the_id() {
    let aim = Aim { machine: "vps".into(), boot: "b".into(), workspace: ThereId(7) };
    assert_eq!(aim.close_args(), ["close", "--workspace", "7", "--boot", "b"]);
}

#[test]
fn every_act_on_a_machine_says_which_run_of_its_server_the_id_is_from() {
    let aim = Aim { machine: "vps".into(), boot: "4242-1".into(), workspace: ThereId(7) };
    let acts = [aim.rename_args("x"), aim.close_args()];
    for act in acts {
        let at = act.iter().position(|word| word == "--boot").expect("--boot");
        assert_eq!(act[at + 1], "4242-1");
    }
}

#[test]
fn a_view_line_says_when_a_fallen_link_tries_next() {
    let seen: MachineSeen = serde_json::from_str(fixture!("machines/seen_fallen.json")).expect("a machine");
    assert_eq!(seen.next_try(now()), Some(NextTry::In(4)));
}

#[test]
fn a_tab_on_a_fallen_link_counts_down_before_saying_when_it_was_seen() {
    let seen = MachineSeen { retry_at: Some(TRY_IN_4_S.into()), ..down(Some(SEEN)) };
    let note = seen.tab_note(&agent_tab("waiting"), now());
    assert_eq!(note.as_deref(), Some("waiting · reconnecting in 4 s · seen 2 min ago"));
}

#[test]
fn a_tab_on_a_link_whose_try_is_under_way_says_so() {
    let seen = MachineSeen { retry_at: Some(TRY_IN_4_S.into()), ..down(Some(SEEN)) };
    let note = seen.tab_note(&agent_tab("waiting"), now() + 4);
    assert_eq!(note.as_deref(), Some("waiting · reconnecting… · seen 2 min ago"));
}

#[test]
fn a_tab_note_counts_while_it_shows_a_time_that_moves() {
    let tab = agent_tab("waiting");
    assert!(down(Some(SEEN)).tab_note_counts(&tab, now()));
    let never_seen = MachineSeen { retry_at: Some(TRY_IN_4_S.into()), ..down(None) };
    assert!(never_seen.tab_note_counts(&tab, now()));
    assert!(!never_seen.tab_note_counts(&tab, now() + 4));
}

#[test]
fn nothing_counts_on_a_linked_machine_or_a_tab_with_no_note() {
    let linked = MachineSeen { state: LinkState::Linked, ..down(Some(SEEN)) };
    assert!(!linked.tab_note_counts(&agent_tab("waiting"), now()));
    assert!(!down(Some(SEEN)).tab_note_counts(&Tab::default(), now()));
}

#[test]
fn a_key_of_a_machines_id_is_not_a_number_a_command_would_take() {
    assert_eq!(ThereId(7).key(), "there-7");
}

#[test]
fn a_tab_with_an_agent_on_a_machine_that_is_down_says_its_status_and_when_it_was_seen() {
    assert_eq!(down(Some(SEEN)).tab_note(&agent_tab("waiting"), now()).as_deref(), Some("waiting · seen 2 min ago"));
}

#[test]
fn a_machine_never_seen_says_only_the_status() {
    assert_eq!(down(None).tab_note(&agent_tab("working"), now()).as_deref(), Some("working"));
}

#[test]
fn a_tab_without_an_agent_says_nothing_of_it() {
    assert_eq!(down(Some(SEEN)).tab_note(&Tab::default(), now()), None);
}

#[test]
fn a_linked_machines_tab_says_nothing_of_when_it_was_seen() {
    let linked = MachineSeen { state: LinkState::Linked, ..down(Some(SEEN)) };
    assert_eq!(linked.tab_note(&agent_tab("waiting"), now()), None);
}

#[test]
fn each_state_says_why_nothing_can_be_sent() {
    let note = |state| MachineSeen { state, ..down(None) }.link_note();
    assert_eq!(note(LinkState::Unreachable), "vps unreachable");
    assert_eq!(note(LinkState::Older), "vps runs an older cornercase");
    assert_eq!(note(LinkState::Connecting), "vps is connecting");
}

#[test]
fn an_aim_finds_its_workspace_while_the_machine_runs_on() {
    let session = session();
    let seen = vps(&session);
    let aim = seen.aim_at(named(seen, "notes"));
    let (_, ws) = sighted(&session.machines, &aim).expect("sighted");
    assert_eq!(ws.name, "notes");
}

#[test]
fn an_aim_at_a_machine_that_restarted_is_refused() {
    let session = session();
    let seen = vps(&session);
    let aim = Aim { boot: "before".into(), ..seen.aim_at(named(seen, "notes")) };
    assert_eq!(sighted(&session.machines, &aim).err().as_deref(), Some("vps restarted"));
}

#[test]
fn an_aim_at_a_workspace_that_left_is_refused() {
    let session = session();
    let aim = Aim { machine: "vps".into(), boot: vps(&session).boot.clone(), workspace: ThereId(99) };
    assert_eq!(sighted(&session.machines, &aim).err().as_deref(), Some("that workspace is no longer on vps"));
}

#[test]
fn an_aim_at_a_machine_no_longer_listed_is_refused() {
    let aim = Aim { machine: "gone".into(), boot: "b".into(), workspace: ThereId(1) };
    assert_eq!(sighted(&[], &aim).err().as_deref(), Some("gone is no longer listed"));
}

#[test]
fn a_place_keeps_its_tab_while_the_machine_runs_on() {
    let session = session();
    let seen = vps(&session);
    let place = MachinePlace::at(seen, named(seen, "notes"), Some(ThereId(16)));
    assert_eq!(place.followed(&session.machines), Some(place));
}

#[test]
fn a_place_on_a_workspace_picks_its_active_tab() {
    let session = session();
    let seen = vps(&session);
    let place = MachinePlace::at(seen, named(seen, "notes"), None);
    assert_eq!(place.tab, Some(ThereId(10)));
}

#[test]
fn a_place_whose_tab_closed_falls_back_to_the_active_one() {
    let session = session();
    let seen = vps(&session);
    let mut place = MachinePlace::at(seen, named(seen, "notes"), None);
    place.tab = Some(ThereId(99));
    assert_eq!(place.followed(&session.machines).and_then(|place| place.tab), Some(ThereId(10)));
}

#[test]
fn a_place_whose_workspace_closed_is_dropped() {
    let session = session();
    let seen = vps(&session);
    let mut place = MachinePlace::at(seen, named(seen, "notes"), None);
    place.workspace = ThereId(99);
    assert_eq!(place.followed(&session.machines), None);
}

#[test]
fn a_place_on_a_machine_no_longer_listed_is_dropped() {
    let session = session();
    let seen = vps(&session);
    let place = MachinePlace::at(seen, named(seen, "notes"), None);
    assert_eq!(place.followed(&[]), None);
}

#[test]
fn after_a_restart_a_place_finds_its_workspace_by_path_and_name() {
    let session = session();
    let seen = vps(&session);
    let place = MachinePlace { boot: "before".into(), ..MachinePlace::at(seen, named(seen, "notes"), None) };
    let followed = place.followed(&session.machines).expect("found by its path");
    assert_eq!((followed.workspace, followed.boot.as_str()), (ThereId(11), seen.boot.as_str()));
}

#[test]
fn after_a_restart_a_place_whose_folder_is_gone_is_dropped() {
    let session = session();
    let seen = vps(&session);
    let mut place = MachinePlace { boot: "before".into(), ..MachinePlace::at(seen, named(seen, "notes"), None) };
    place.path = "/gone".into();
    assert_eq!(place.followed(&session.machines), None);
}

#[test]
fn a_notices_panes_find_the_tab_that_holds_them() {
    let session = session();
    let boot = vps(&session).boot.as_str();
    let place = session.place_holding_panes(("vps", boot), &[ThereId(99), ThereId(15)]).expect("pane 15 is there");
    assert_eq!((place.workspace, place.tab), (ThereId(11), Some(ThereId(16))));
}

#[test]
fn a_notices_panes_that_left_find_nothing() {
    let session = session();
    let boot = vps(&session).boot.as_str();
    assert_eq!(session.place_holding_panes(("vps", boot), &[ThereId(99)]), None);
    assert_eq!(session.place_holding_panes(("other", boot), &[ThereId(15)]), None);
}

#[test]
fn a_notice_from_before_the_machine_restarted_finds_nothing() {
    assert_eq!(session().place_holding_panes(("vps", "before"), &[ThereId(15)]), None);
}

#[test]
fn two_tabs_of_a_workspace_are_the_same_workspace() {
    let session = session();
    let seen = vps(&session);
    let notes = named(seen, "notes");
    let one = MachinePlace::at(seen, notes, Some(ThereId(10)));
    assert!(one.same_workspace(&MachinePlace::at(seen, notes, Some(ThereId(16)))));
    assert!(!one.same_workspace(&MachinePlace::at(seen, named(seen, "main"), None)));
}

#[test]
fn a_place_under_a_pair_names_the_project_here() {
    let session = session();
    let seen = vps(&session);
    let place = MachinePlace::at(seen, named(seen, "notes"), None);
    assert_eq!(session.pair_of(&place), Some(8));
    assert!(session.alone_of(&place).is_none());
}

#[test]
fn a_place_in_a_project_alone_names_no_project_here() {
    let session = session();
    let seen = vps(&session);
    let place = MachinePlace::at(seen, &seen.projects[2].workspaces[0], None);
    assert_eq!(session.pair_of(&place), None);
    assert_eq!(session.alone_of(&place).map(|(_, there)| there.name.as_str()), Some("solo"));
}

#[test]
fn a_place_opens_a_view_on_its_tab_at_the_size_wanted() {
    let session = session();
    let seen = vps(&session);
    let place = MachinePlace::at(seen, named(seen, "notes"), Some(ThereId(16)));
    assert_eq!(place.open_args((80, 24), false), ["view", "--cols", "80", "--rows", "24", "--tab", "16"]);
}

#[test]
fn a_place_on_a_workspace_with_no_tab_opens_a_view_on_the_workspace() {
    let session = session();
    let seen = vps(&session);
    let place = MachinePlace { tab: None, ..MachinePlace::at(seen, named(seen, "notes"), None) };
    assert_eq!(place.open_args((80, 24), false), ["view", "--cols", "80", "--rows", "24", "--workspace", "11"]);
}

#[test]
fn a_view_opened_behind_other_windows_starts_hidden() {
    let session = session();
    let seen = vps(&session);
    let place = MachinePlace::at(seen, named(seen, "notes"), Some(ThereId(16)));
    assert_eq!(place.open_args((80, 24), true).last().map(String::as_str), Some("--hidden"));
}

#[test]
fn a_place_moves_a_view_to_its_tab() {
    let session = session();
    let seen = vps(&session);
    let place = MachinePlace::at(seen, named(seen, "notes"), Some(ThereId(16)));
    assert_eq!(place.focus_args(40), ["focus", "--tab", "16", "--view", "40"]);
}

#[test]
fn a_place_is_kept_while_its_machine_is_down_even_when_its_workspace_cannot_be_seen() {
    let session = session();
    let seen = vps(&session);
    let place = MachinePlace::at(seen, named(seen, "notes"), None);
    let down = MachineSeen { name: seen.name.clone(), state: LinkState::Connecting, ..MachineSeen::default() };
    assert_eq!(place.followed(&[down]), Some(place));
}

#[test]
fn two_places_on_one_tab_show_the_same() {
    let session = session();
    let seen = vps(&session);
    let place = MachinePlace::at(seen, named(seen, "notes"), Some(ThereId(16)));
    let active = MachinePlace::at(seen, named(seen, "notes"), None);
    assert!(place.shows_the_same(&place.clone()));
    assert!(!place.shows_the_same(&active));
}

#[test]
fn a_place_says_when_it_is_gone() {
    let session = session();
    let seen = vps(&session);
    let place = MachinePlace::at(seen, named(seen, "notes"), None);
    assert_eq!(place.gone_note(), "notes is no longer on vps");
}

#[test]
fn a_boot_goes_at_the_end_of_a_command() {
    let args = with_boot(args(&["mark", "--pane", "9"]), "b1");
    assert_eq!(args, ["mark", "--pane", "9", "--boot", "b1"]);
}

#[test]
fn a_boot_goes_before_the_words_after_two_dashes() {
    let args = with_boot(args(&["account", "restart", "--json", "--pane", "9", "--", "claude", "work"]), "b1");
    assert_eq!(args, ["account", "restart", "--json", "--pane", "9", "--boot", "b1", "--", "claude", "work"]);
}

#[test]
fn a_machine_workspaces_ports_go_by_its_own_id() {
    let session = session();
    let seen = vps(&session);
    let aim = seen.aim_at(named(seen, "main"));
    let id = aim.workspace_arg();
    assert_eq!(aim.ports_args(false), ["ports", "--json", "--workspace", id.as_str()]);
    assert_eq!(aim.ports_args(true), ["ports", "--json", "--all"]);
}

#[test]
fn a_panes_tab_is_found_among_the_machines_workspaces() {
    let session = session();
    let seen = vps(&session);
    let tab = seen.tab_of_pane(ThereId::heard(15)).expect("the agent's tab");
    assert_eq!(tab.name, "fake-agent");
    assert!(seen.tab_of_pane(ThereId::heard(999)).is_none());
    assert_eq!(ThereId::heard(15).pane_words(), "pane 15");
}

fn owner_of(seen: &MachineSeen, name: &str) -> WorkspaceThere {
    WorkspaceThere::at(seen, named(seen, name), "vps.invalid")
}

fn restarted(session: &Session) -> Vec<MachineSeen> {
    let mut seen = vps(session).clone();
    seen.boot = "another-boot".into();
    for project in &mut seen.projects {
        for (at, ws) in project.workspaces.iter_mut().enumerate() {
            ws.id = ThereId(900 + at as u64);
        }
    }
    vec![seen]
}

#[test]
fn a_machine_owner_follows_its_id_within_a_run() {
    let session = session();
    let owner = owner_of(vps(&session), "main");
    let followed = owner.followed(&session.machines).expect("the workspace is still there");
    assert_eq!(followed.aim(), owner.aim());
}

#[test]
fn a_machine_owner_follows_its_path_and_name_after_a_restart() {
    let session = session();
    let owner = owner_of(vps(&session), "main");
    let machines = restarted(&session);
    let followed = owner.followed(&machines).expect("found by its path and name");
    assert_ne!(followed.aim(), owner.aim());
    assert_eq!(followed.aim().boot, "another-boot");
    assert_eq!(followed.destination(), "vps.invalid");
}

#[test]
fn a_machine_owner_stays_while_its_machine_is_down() {
    let session = session();
    let owner = owner_of(vps(&session), "main");
    let mut gone = down(Some(SEEN));
    gone.boot = vps(&session).boot.clone();
    assert_eq!(owner.followed(&[gone]).as_ref(), Some(&owner));
}

#[test]
fn a_machine_owner_is_gone_when_its_workspace_closed_there() {
    let session = session();
    let owner = owner_of(vps(&session), "main");
    let mut seen = vps(&session).clone();
    seen.projects.iter_mut().for_each(|p| p.workspaces.retain(|ws| ws.name != "main"));
    assert_eq!(owner.followed(&[seen]), None);
    assert_eq!(owner.followed(&[]), None);
}

#[test]
fn owners_of_one_workspace_are_equal_whatever_their_path_or_destination() {
    let session = session();
    let seen = vps(&session);
    let a = owner_of(seen, "main");
    let b = WorkspaceThere::at(seen, named(seen, "main"), "elsewhere.invalid");
    assert_eq!(a, b);
    let hash = |owner: &WorkspaceThere| {
        use std::hash::{Hash as _, Hasher as _};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        owner.hash(&mut hasher);
        hasher.finish()
    };
    assert_eq!(hash(&a), hash(&b));
}
