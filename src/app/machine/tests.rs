use std::time::{Duration, Instant};

use serde_json::json;

use super::*;
use crate::cli::QueueTap;
use crate::model::machines::LinkState;
use crate::model::with_boot;
use crate::model::{Shown, Tab, ThereProject, ThereWorkspace, ViewInfo};

fn id(number: u64) -> ThereId {
    ThereId::heard(number)
}

fn workspace(number: u64, path: &str, tabs: &[u64]) -> ThereWorkspace {
    let tabs = tabs.iter().map(|tab| Tab { id: id(*tab), ..Tab::default() }).collect();
    ThereWorkspace { id: id(number), name: "notes".into(), path: path.into(), tabs, ..ThereWorkspace::default() }
}

fn machine(name: &str, boot: &str) -> MachineSeen {
    let project = ThereProject {
        workspaces: vec![workspace(1, "/home/ana/code/shop", &[20, 21]), workspace(2, "/home/ana/code/web", &[30])],
        ..ThereProject::default()
    };
    MachineSeen {
        name: name.into(),
        boot: boot.into(),
        state: LinkState::Linked,
        projects: vec![project],
        ..MachineSeen::default()
    }
}

fn place_on(seen: &MachineSeen, ws: u64, tab: u64) -> MachinePlace {
    MachinePlace::at(seen, seen.workspace(id(ws)).expect("the workspace"), Some(id(tab)))
}

fn place(tab: u64) -> MachinePlace {
    place_on(&machine("vps", "b1"), 1, tab)
}

pub(super) fn opening() -> MachineTab {
    MachineTab::new(&place(20), None)
}

pub(super) fn tapped() -> (MachineTab, QueueTap) {
    let (queue, tap) = QueueTap::new();
    let mut tab = opening();
    tab.conn = Connection::new(Target::Local, queue);
    (tab, tap)
}

fn frame(tab: u64, pane: u64) -> Frame {
    serde_json::from_value(json!({
        "boot": "b1",
        "surface": 4,
        "revision": 1,
        "tab": tab,
        "panes": [{ "pane": pane, "active": true }],
    }))
    .expect("a frame")
}

fn live(tab_id: u64, pane: u64) -> (MachineTab, QueueTap) {
    let (mut tab, tap) = tapped();
    tab.conn.view_id = Some(9);
    tab.conn.frame_arrived(frame(tab_id, pane));
    tab.heard_frame(id(tab_id), Instant::now());
    (tab, tap)
}

fn arriving(tab_id: u64) -> (MachineTab, QueueTap) {
    let (mut tab, tap) = live(20, 5);
    tab.steered(&place(tab_id));
    (tab, tap)
}

fn held(tab: &MachineTab) -> usize {
    tab.arriving.as_ref().map_or(0, |arriving| arriving.keys.len())
}

fn lost_stream(next: Instant) -> Lost {
    Lost::Stream { next, try_number: 1, why: None }
}

#[test]
fn nothing_picked_and_nothing_shown_plans_nothing() {
    assert_eq!(plan(None, None), Plan::Nothing);
}

#[test]
fn a_first_pick_opens_a_connection() {
    assert_eq!(plan(None, Some(&place(20))), Plan::Open);
}

#[test]
fn leaving_the_place_goes_back_to_the_window_own_tab() {
    assert_eq!(plan(Some(&opening()), None), Plan::Back);
}

#[test]
fn another_tab_on_the_same_run_moves_the_view() {
    assert_eq!(plan(Some(&opening()), Some(&place(21))), Plan::Steer);
}

#[test]
fn another_workspace_on_the_same_run_moves_the_view_too() {
    let other = place_on(&machine("vps", "b1"), 2, 30);
    assert_eq!(plan(Some(&opening()), Some(&other)), Plan::Steer);
}

#[test]
fn another_machine_opens_a_connection_of_its_own() {
    let other = place_on(&machine("mini", "b1"), 1, 20);
    assert_eq!(plan(Some(&opening()), Some(&other)), Plan::Open);
}

#[test]
fn a_restart_there_opens_a_new_connection() {
    let other = place_on(&machine("vps", "b2"), 1, 20);
    assert_eq!(plan(Some(&opening()), Some(&other)), Plan::Open);
}

#[test]
fn a_link_that_falls_takes_the_tab_down() {
    for state in [TabState::Opening, TabState::Live, TabState::Down(lost_stream(Instant::now()))] {
        assert_eq!(link_step(&state, false), LinkStep::Fall, "{state:?}");
    }
}

#[test]
fn a_link_that_comes_back_opens_the_tab_again() {
    assert_eq!(link_step(&TabState::Down(Lost::Link), true), LinkStep::Reopen);
    assert_eq!(link_step(&TabState::Down(Lost::NoWay("why".into())), true), LinkStep::Reopen);
}

#[test]
fn a_stream_that_fell_with_the_link_up_waits_for_its_own_retry() {
    assert_eq!(link_step(&TabState::Down(lost_stream(Instant::now())), true), LinkStep::Stay);
}

#[test]
fn a_live_tab_on_a_live_link_stays() {
    assert_eq!(link_step(&TabState::Live, true), LinkStep::Stay);
    assert_eq!(link_step(&TabState::Opening, true), LinkStep::Stay);
}

#[test]
fn a_window_on_another_server_never_opens_the_tab() {
    assert_eq!(link_step(&TabState::Down(Lost::Elsewhere("why".into())), true), LinkStep::Stay);
    assert_eq!(link_step(&TabState::Down(Lost::Elsewhere("why".into())), false), LinkStep::Stay);
}

#[test]
fn with_no_machine_shown_a_key_goes_to_the_windows_own_pane() {
    assert_eq!(keys_go(None, Some(5)), KeysGo::Pane(5));
}

#[test]
fn with_no_machine_and_no_frame_a_key_goes_nowhere() {
    assert_eq!(keys_go(None, None), KeysGo::Nowhere);
}

#[test]
fn a_key_goes_to_the_pane_of_a_live_machines_tab() {
    let (tab, _tap) = live(20, 5);
    assert!(tab.live());
    assert_eq!(keys_go(Some(&tab), Some(5)), KeysGo::Pane(5));
}

#[test]
fn a_key_is_refused_before_the_first_frame() {
    assert_eq!(keys_go(Some(&opening()), None), KeysGo::Refuse);
}

#[test]
fn a_key_is_refused_while_the_tab_is_down() {
    let (mut tab, _tap) = live(20, 5);
    tab.fall(Lost::Link);
    assert!(!tab.live());
    assert_eq!(keys_go(Some(&tab), None), KeysGo::Refuse);
}

#[test]
fn a_key_waits_while_another_tab_arrives() {
    let (tab, _tap) = arriving(21);
    assert!(!tab.live());
    assert_eq!(keys_go(Some(&tab), None), KeysGo::Hold);
}

#[test]
fn asking_for_another_tab_drops_the_keys_held_for_the_one_before() {
    let (mut tab, tap) = arriving(21);
    tab.hold(json!({ "text": "a" }), Instant::now());
    assert!(tab.steered(&place(20)));
    assert_eq!(held(&tab), 0);
    assert_eq!(tap.inputs(), Vec::new());
}

#[test]
fn a_fall_keeps_the_last_frame_dimmed() {
    let (mut tab, _tap) = live(20, 5);
    tab.fall(Lost::Link);
    assert!(tab.conn.has_frame);
    assert!(tab.dimmed());
}

#[test]
fn a_live_tab_is_not_dimmed() {
    assert!(!live(20, 5).0.dimmed());
}

#[test]
fn nothing_sent_after_a_fall_reaches_the_machine() {
    let (mut tab, tap) = live(20, 5);
    tab.fall(lost_stream(Instant::now()));
    tab.conn.queue.input(5, json!({ "text": "a" }));
    tab.conn.act(vec!["focus".into()]);
    assert_eq!(tap.inputs(), Vec::new());
    assert_eq!(tap.commands(), Vec::<Vec<String>>::new());
}

#[test]
fn a_fall_drops_the_keys_held_and_says_so() {
    let (mut tab, tap) = arriving(21);
    assert!(tab.hold(json!({ "text": "a" }), Instant::now()));
    assert!(tab.fall(Lost::Link));
    assert!(tab.arriving.is_none());
    assert_eq!(tap.inputs(), Vec::new());
}

#[test]
fn a_fall_with_no_keys_held_has_nothing_to_say() {
    let (mut tab, _tap) = live(20, 5);
    assert!(!tab.fall(Lost::Link));
}

#[test]
fn each_fall_waits_longer_until_a_frame_comes() {
    let (mut tab, _tap) = live(20, 5);
    assert_eq!(tab.next_wait().0, Duration::from_secs(1));
    assert_eq!(tab.next_wait().0, Duration::from_secs(2));
    assert_eq!(tab.next_wait().0, Duration::from_secs(4));
    tab.heard_frame(id(20), Instant::now());
    assert_eq!(tab.next_wait().0, Duration::from_secs(1));
}

#[test]
fn each_fall_has_a_number_of_its_own_for_its_retry() {
    let (mut tab, _tap) = live(20, 5);
    let (_, first) = tab.next_wait();
    tab.heard_frame(id(20), Instant::now());
    let (_, second) = tab.next_wait();
    assert_ne!(first, second);
}

#[test]
fn a_frame_makes_the_tab_live() {
    let mut tab = opening();
    tab.heard_frame(id(20), Instant::now());
    assert_eq!(tab.state, TabState::Live);
}

#[test]
fn a_tab_the_view_was_asked_for_needs_no_steering() {
    let (tab, _tap) = live(20, 5);
    assert!(!tab.needs_steering(&place(20)));
}

#[test]
fn another_tab_needs_steering_once_the_view_is_known() {
    let (tab, _tap) = live(20, 5);
    assert!(tab.needs_steering(&place(21)));
    assert!(!opening().needs_steering(&place(21)));
}

#[test]
fn a_workspace_with_no_tab_is_steered_to_without_waiting_for_a_frame() {
    let (mut tab, _tap) = live(20, 5);
    let seen = machine("vps", "b1");
    let bare = workspace(3, "/home/ana/code/bare", &[]);
    tab.steered(&MachinePlace::at(&seen, &bare, None));
    assert!(tab.arriving.is_none());
}

#[test]
fn keys_held_go_to_the_active_pane_of_the_tab_asked_for() {
    let (mut tab, tap) = arriving(21);
    tab.hold(json!({ "text": "a" }), Instant::now());
    tab.hold(json!({ "text": "b" }), Instant::now());
    tab.conn.frame_arrived(frame(21, 7));
    assert!(!tab.heard_frame(id(21), Instant::now()));
    assert!(tab.live());
    assert_eq!(tap.inputs(), [(7, json!({ "text": "a" })), (7, json!({ "text": "b" }))]);
}

#[test]
fn a_frame_of_the_tab_left_keeps_the_keys_waiting() {
    let (mut tab, tap) = arriving(21);
    tab.hold(json!({ "text": "a" }), Instant::now());
    tab.conn.frame_arrived(frame(20, 5));
    tab.heard_frame(id(20), Instant::now());
    assert!(!tab.live());
    assert_eq!(tap.inputs(), Vec::new());
    assert_eq!(held(&tab), 1);
}

#[test]
fn keys_that_waited_too_long_are_dropped_and_said() {
    let (mut tab, tap) = arriving(21);
    let start = Instant::now();
    tab.hold(json!({ "text": "a" }), start);
    tab.conn.frame_arrived(frame(21, 7));
    assert!(tab.heard_frame(id(21), start + HELD_FOR + Duration::from_secs(1)));
    assert_eq!(tap.inputs(), Vec::new());
}

#[test]
fn a_key_that_comes_too_late_drops_those_before_it() {
    let (mut tab, _tap) = arriving(21);
    let start = Instant::now();
    tab.hold(json!({ "text": "a" }), start);
    assert!(!tab.hold(json!({ "text": "b" }), start + HELD_FOR + Duration::from_secs(1)));
    assert_eq!(held(&tab), 0);
}

#[test]
fn only_so_many_keys_wait() {
    let (mut tab, _tap) = arriving(21);
    let now = Instant::now();
    for _ in 0..MOST_HELD {
        assert!(tab.hold(json!({ "text": "a" }), now));
    }
    assert!(!tab.hold(json!({ "text": "a" }), now));
    assert_eq!(held(&tab), MOST_HELD);
}

#[test]
fn opening_a_view_gives_the_machine_this_windows_colours() {
    let (mut tab, tap) = tapped();
    let (tx, _rx) = futures::channel::mpsc::unbounded();
    tab.view_opened(&tx);
    assert!(tab.themed);
    assert_eq!(tap.commands(), [["theme", "-"]]);
}

#[test]
fn a_window_that_hid_while_the_view_opened_says_so_once_it_can() {
    let (mut tab, tap) = tapped();
    tab.conn.view_id = Some(9);
    tab.told_visible = false;
    let (tx, _rx) = futures::channel::mpsc::unbounded();
    tab.view_opened(&tx);
    assert_eq!(tap.commands(), [vec!["theme", "-"], vec!["view-set", "--view", "9", "--hidden"]]);
}

#[test]
fn a_pane_of_the_frame_shown_copies_here() {
    let (tab, _tap) = live(20, 5);
    assert!(copies_here(&tab.conn.grid, Some(5)));
}

#[test]
fn a_pane_the_frame_does_not_show_copies_nothing_here() {
    let (tab, _tap) = live(20, 5);
    assert!(!copies_here(&tab.conn.grid, Some(99)));
    assert!(!copies_here(&tab.conn.grid, None));
}

#[test]
fn a_programs_copy_and_bell_are_what_is_heard_of_a_machines_events() {
    let copied = host_heard(Event::Clipboard { text: "x".into(), pane: Some(5) });
    assert!(matches!(copied, Some(Heard::Copied(text, Some(5))) if text == "x"));
    assert!(matches!(host_heard(Event::Bell { count: 1, tab: Some(6), pane: None }), Some(Heard::Bell(Some(6)))));
    assert!(host_heard(Event::Other).is_none());
}

#[test]
fn the_first_try_says_it_connects() {
    assert_eq!(opening().standing(Instant::now()), Standing::Connecting);
}

#[test]
fn a_try_after_a_fall_says_it_reconnects() {
    let (mut tab, _tap) = live(20, 5);
    tab.state = TabState::Opening;
    assert_eq!(tab.standing(Instant::now()), Standing::Reconnecting);
}

#[test]
fn a_live_tab_with_a_frame_shows_no_card() {
    assert_eq!(live(20, 5).0.standing(Instant::now()), Standing::Shown);
}

#[test]
fn a_link_that_fell_leaves_its_words_to_the_list() {
    let (mut tab, _tap) = live(20, 5);
    tab.fall(Lost::Link);
    assert_eq!(tab.standing(Instant::now()), Standing::LinkDown);
}

#[test]
fn a_stream_that_fell_counts_down_to_its_retry() {
    let now = Instant::now();
    let (mut tab, _tap) = live(20, 5);
    tab.fall(lost_stream(now + Duration::from_millis(3500)));
    assert_eq!(tab.standing(now), Standing::RetryIn(4, None));
}

#[test]
fn a_stream_that_fell_keeps_the_last_thing_ssh_said() {
    assert_eq!(reason_of("a banner\nssh: refused\n\n").as_deref(), Some("ssh: refused"));
    assert_eq!(reason_of("  \n"), None);
}

#[test]
fn a_view_on_no_tab_says_so() {
    let (mut tab, _tap) = live(20, 5);
    let view = ViewInfo { id: 9, projection: 9, shown: Shown::default(), ..ViewInfo::default() };
    tab.session = Some(Session { view: Some(view), ..Session::default() });
    assert_eq!(tab.standing(Instant::now()), Standing::NoTab);
}

#[test]
fn a_refused_key_says_the_link_is_down() {
    let (mut tab, _tap) = live(20, 5);
    tab.fall(Lost::Link);
    let mut seen = machine("vps", "b1");
    seen.state = LinkState::Unreachable;
    assert_eq!(refusal(&tab, Some(&seen)), "vps unreachable");
}

#[test]
fn a_refused_key_says_the_machine_cannot_be_reached_when_only_the_stream_fell() {
    let (mut tab, _tap) = live(20, 5);
    tab.fall(lost_stream(Instant::now()));
    assert_eq!(refusal(&tab, None), "vps unreachable");
}

#[test]
fn a_refused_key_says_the_tab_is_still_opening() {
    assert_eq!(refusal(&opening(), None), "The tab on vps is still opening");
}

#[test]
fn a_refused_key_says_a_window_elsewhere_cannot_draw_it() {
    let (mut tab, _tap) = live(20, 5);
    tab.fall(Lost::Elsewhere("why".into()));
    assert_eq!(refusal(&tab, None), format!("The tab on vps needs this window on this {}", desktop::here()));
}

#[test]
fn a_line_or_frame_of_another_run_is_not_this_tabs() {
    let tab = opening();
    assert!(tab.same_run("b1"));
    assert!(!tab.same_run("b2"));
}

fn run_of(tab: &MachineTab) -> MachineRun {
    tab.run()
}

#[test]
fn closing_a_pane_there_names_the_run_it_was_seen_on() {
    assert_eq!(with_boot(Closable::Pane(5).args(), "b1"), ["close", "--pane", "5", "--boot", "b1"]);
}

#[test]
fn closing_a_tab_there_names_the_run_it_was_seen_on() {
    assert_eq!(with_boot(Closable::Tab(20).args(), "b1"), ["close", "--tab", "20", "--boot", "b1"]);
}

#[test]
fn a_close_goes_on_the_machine_s_own_queue() {
    let (tab, tap) = live(20, 5);
    tab.send_close(Closable::Pane(5), |_| {});
    assert_eq!(tap.commands(), [["close", "--pane", "5", "--boot", "b1"]]);
}

#[test]
fn a_tab_that_fell_sends_no_close() {
    let (mut tab, tap) = live(20, 5);
    tab.fall(Lost::Link);
    tab.send_close(Closable::Pane(5), |_| {});
    assert_eq!(tap.commands(), Vec::<Vec<String>>::new());
}

#[test]
fn a_live_tab_of_the_same_run_may_be_closed() {
    let (tab, _tap) = live(20, 5);
    assert_eq!(still_on(Some(&tab), &run_of(&tab), None), Ok(()));
    let (tab, _tap) = arriving(21);
    assert_eq!(still_on(Some(&tab), &run_of(&tab), None), Ok(()));
}

#[test]
fn a_tab_of_another_run_refuses_as_a_restart() {
    let (tab, _tap) = live(20, 5);
    let run = MachineRun { boot: "b0".into(), ..run_of(&tab) };
    assert_eq!(still_on(Some(&tab), &run, None), Err("vps restarted".into()));
}

#[test]
fn a_tab_still_opening_refuses() {
    let tab = opening();
    assert_eq!(still_on(Some(&tab), &run_of(&tab), None), Err("The tab on vps is still opening".into()));
}

#[test]
fn a_tab_that_fell_refuses_with_the_link_s_words() {
    let (mut tab, _tap) = live(20, 5);
    tab.fall(Lost::Link);
    assert_eq!(still_on(Some(&tab), &run_of(&tab), None), Err("vps unreachable".into()));
}

#[test]
fn no_machine_tab_or_another_machine_refuses_as_no_longer_shown() {
    let (tab, _tap) = live(20, 5);
    let run = run_of(&tab);
    let gone = Err("The tab on vps is no longer shown".to_string());
    assert_eq!(still_on(None, &run, None), gone);
    let mut other = MachineTab::new(&place_on(&machine("mini", "b1"), 1, 20), None);
    other.state = TabState::Live;
    assert_eq!(still_on(Some(&other), &run, None), gone);
}

#[test]
fn the_close_job_never_matches_a_local_pane() {
    let key = Closable::Pane(9).key(&MachineRun { machine: "vps".into(), boot: "b1".into() });
    assert_ne!(key, "close-pane:9");
    assert!(key.contains("vps") && key.contains("b1"));
}

fn with_agents(mut tab: MachineTab) -> MachineTab {
    tab.session = Some(
        serde_json::from_value(json!({
            "boot": "b1",
            "projects": [{"id": 1, "workspaces": [{"id": 2, "tabs": [
                {"id": 20, "name": "shop", "panes": [{"id": 5, "agent": "claude"}, {"id": 6}]}
            ]}]}]
        }))
        .expect("a session"),
    );
    tab
}

#[test]
fn closing_an_agent_there_names_the_run_it_was_seen_on() {
    assert_eq!(with_boot(Closable::Agent(5).args(), "b1"), ["close", "--agent", "--pane", "5", "--boot", "b1"]);
}

#[test]
fn an_agents_close_job_is_its_own() {
    let run = MachineRun { machine: "vps".into(), boot: "b1".into() };
    let key = Closable::Agent(5).key(&run);
    assert_ne!(key, Closable::Pane(5).key(&run));
    assert_ne!(key, "close-agent:5");
    assert!(key.contains("agent"));
}

#[test]
fn an_agent_close_goes_on_the_machine_s_own_queue() {
    let (tab, tap) = live(20, 5);
    tab.send_close(Closable::Agent(5), |_| {});
    assert_eq!(tap.commands(), [["close", "--agent", "--pane", "5", "--boot", "b1"]]);
}

#[test]
fn the_agent_check_there_names_the_run() {
    assert_eq!(
        with_boot(agent_check_args(5), "b1"),
        ["close", "--agent", "--pane", "5", "--check", "--json", "--boot", "b1"]
    );
}

#[test]
fn the_agent_check_goes_on_the_machine_s_own_queue() {
    let (tab, tap) = live(20, 5);
    tab.send_agent_check(5, |_| {});
    assert_eq!(tap.commands(), [["close", "--agent", "--pane", "5", "--check", "--json", "--boot", "b1"]]);
}

#[test]
fn a_tab_that_fell_sends_no_agent_check() {
    let (mut tab, tap) = live(20, 5);
    tab.fall(Lost::Link);
    tab.send_agent_check(5, |_| {});
    assert_eq!(tap.commands(), Vec::<Vec<String>>::new());
}

#[test]
fn a_live_tab_publishes_its_run_target_and_agent_panes() {
    let (tab, _tap) = live(20, 5);
    let there = with_agents(tab).there(None);
    assert_eq!((there.machine.as_str(), there.boot.as_str()), ("vps", "b1"));
    assert_eq!((there.target, there.down, there.agents), (Target::Local, None, vec![5]));
}

#[test]
fn a_tab_that_fell_or_still_opens_publishes_why_it_is_down() {
    let (mut tab, _tap) = live(20, 5);
    tab.fall(Lost::Link);
    assert_eq!(tab.there(None).down.as_deref(), Some("vps unreachable"));
    assert_eq!(opening().there(None).down.as_deref(), Some("The tab on vps is still opening"));
}
