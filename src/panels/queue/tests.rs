use serde_json::json;

use super::data::{
    Body, Click, Entry, Group, Place, State, agent_choices, entries, folder_name, live_pane, mode_choices, notice,
    place_label, sections, sentence, status_label,
};
use super::draft::{Draft, Target, When};
use super::time::{Civil, days, local, relative, when_label};
use crate::model::{PaneInfo, Project, Session, Tab, Workspace};

#[test]
fn reads_entries_leniently() {
    let app = json!({ "boot": "1-2", "queue": [
        { "id": 4, "at": 1_800_000_000, "zone": "Europe/Madrid", "label": "#12 Fix login", "agent": "claude",
          "command": "claude", "program": "claude", "mode": "plan", "prompt": "Fix it\nplease", "submit": true,
          "place": { "project": "/src/shop", "workspace": "/src/shop", "workspace_name": null,
                     "workspace_id": 3, "server": "1-2", "branch": "fix-login" },
          "state": "started", "pane": 9, "server": "1-2" },
        { "id": 5, "at": 1_800_000_100, "label": "later", "state": "failed", "reason": "agent missing",
          "accounts": { "claude": "" }, "new_field": 1 },
        "not an entry",
    ]});
    let list = entries(Some(&app));
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].place.branch.as_deref(), Some("fix-login"));
    assert_eq!(list[0].pane, Some(9));
    assert_eq!(list[0].server.as_deref(), Some("1-2"));
    assert_eq!(list[1].reason.as_deref(), Some("agent missing"));
    assert!(list[1].runnable() && !list[0].runnable());
    assert_eq!(entries(None), vec![]);
}

fn entry(id: u64, state: &str) -> Entry {
    Entry { id, state: state.into(), label: format!("entry {id}"), ..Entry::default() }
}

fn ids(list: &[Entry], filter: &str) -> Vec<(Group, Vec<u64>)> {
    sections(list, filter, None)
        .into_iter()
        .map(|(group, shown)| (group, shown.iter().map(|e| e.id).collect()))
        .collect()
}

fn session_with_pane(pane: u64) -> Session {
    let tab = Tab { panes: vec![PaneInfo { id: pane, ..PaneInfo::default() }], ..Tab::default() };
    let workspace = Workspace { tabs: vec![tab], ..Workspace::default() };
    Session { projects: vec![Project { workspaces: vec![workspace], ..Project::default() }], ..Session::default() }
}

#[test]
fn every_state_falls_in_its_section() {
    let states = ["pending", "starting", "started", "failed", "missed", "cancelled"];
    let groups: Vec<Group> = states.iter().map(|s| State::of(&entry(1, s)).group()).collect();
    assert_eq!(
        groups,
        [Group::Scheduled, Group::Scheduled, Group::Started, Group::Unstarted, Group::Unstarted, Group::Other]
    );
}

#[test]
fn what_did_not_start_comes_first_and_each_section_keeps_the_servers_order() {
    let list = [entry(1, "started"), entry(2, "missed"), entry(3, "pending"), entry(4, "failed"), entry(5, "pending")];
    assert_eq!(
        ids(&list, ""),
        [(Group::Unstarted, vec![2, 4]), (Group::Scheduled, vec![3, 5]), (Group::Started, vec![1])]
    );
}

#[test]
fn the_scheduled_section_stays_to_say_that_nothing_is_scheduled() {
    assert_eq!(ids(&[entry(1, "started")], ""), [(Group::Scheduled, vec![]), (Group::Started, vec![1])]);
}

#[test]
fn a_filter_leaves_out_the_sections_it_empties() {
    let list = [Entry { label: "release notes".into(), ..entry(1, "started") }, entry(2, "pending")];
    assert_eq!(ids(&list, "release"), [(Group::Started, vec![1])]);
}

#[test]
fn a_started_entry_names_its_pane_only_while_that_pane_lives() {
    let started = Entry { pane: Some(9), server: Some("boot-1".into()), ..entry(1, "started") };
    let session = session_with_pane(9);
    assert_eq!(live_pane(&started, Some("boot-1"), Some(&session)), Some(9));
    assert_eq!(live_pane(&started, Some("boot-2"), Some(&session)), None);
    assert_eq!(live_pane(&started, Some("boot-1"), Some(&session_with_pane(4))), None);
    assert_eq!(live_pane(&Entry { state: "failed".into(), ..started }, Some("boot-1"), Some(&session)), None);
}

#[test]
fn the_status_names_the_state_and_the_pane() {
    let labels: Vec<String> = ["pending", "starting", "failed", "missed", "cancelled"]
        .iter()
        .map(|s| status_label(&entry(1, s), None))
        .collect();
    assert_eq!(labels, ["Pending", "Starting…", "Failed", "Missed", "cancelled"]);
    assert_eq!(status_label(&entry(1, "started"), Some(9)), "Started · pane 9");
    assert_eq!(status_label(&entry(1, "started"), None), "Started · pane closed");
}

#[test]
fn a_failed_entry_says_why_and_a_missed_one_what_happened() {
    let failed = Entry { reason: Some(" agent missing".into()), ..entry(1, "failed") };
    assert_eq!(notice(&failed).as_deref(), Some("Agent missing"));
    assert_eq!(notice(&entry(2, "failed")), None);
    assert!(notice(&entry(3, "missed")).is_some_and(|n| n.contains("Start it now")));
    assert_eq!(notice(&entry(4, "pending")), None);
}

#[test]
fn an_error_reads_as_a_sentence() {
    assert_eq!(sentence("write the prompt it starts with"), "Write the prompt it starts with");
    assert_eq!(sentence("éxito"), "Éxito");
    assert_eq!(sentence(""), "");
}

#[test]
fn a_click_edits_what_can_run_and_shows_a_live_pane() {
    assert_eq!(Click::of(&entry(1, "missed"), None, false), Click::Edit);
    assert_eq!(Click::of(&entry(1, "pending"), None, true), Click::Nothing);
    assert_eq!(Click::of(&entry(1, "started"), Some(9), false), Click::Open(9));
    assert_eq!(Click::of(&entry(1, "started"), None, false), Click::Nothing);
    assert_eq!(Click::of(&entry(1, "starting"), None, false), Click::Nothing);
}

#[test]
fn the_empty_page_shows_once_the_queue_is_known_to_be_empty() {
    assert_eq!(Body::of(false, 0, 0), Body::Loading);
    assert_eq!(Body::of(true, 0, 0), Body::Empty);
    assert_eq!(Body::of(true, 3, 0), Body::NoMatch);
    assert_eq!(Body::of(true, 3, 1), Body::List);
}

#[test]
fn names_places_like_the_tui() {
    assert_eq!(folder_name("/src/shop/"), "shop");
    let mut place = Place { project: "/src/shop".into(), ..Default::default() };
    assert_eq!(place_label(&place, None), "shop");
    place.workspace = Some("/src/shop".into());
    assert_eq!(place_label(&place, None), "shop");
    place.workspace = Some("/src/wt/feat-x".into());
    assert_eq!(place_label(&place, None), "shop › feat-x");
    place.workspace_name = Some("review".into());
    assert_eq!(place_label(&place, None), "shop › review");
    place.branch = Some("feat/x".into());
    assert_eq!(place_label(&place, None), "shop › feat/x");
}

fn civil(year: i32, month: u32, day: u32, weekday: u32) -> Civil {
    Civil { year, month, day, hour: 9, minute: 5, weekday }
}

#[test]
fn says_when_from_today() {
    let today = civil(2026, 10, 31, 6);
    assert_eq!(when_label(civil(2026, 10, 31, 6), today), "today 09:05");
    assert_eq!(when_label(civil(2026, 11, 1, 0), today), "tomorrow 09:05");
    assert_eq!(when_label(civil(2026, 10, 30, 5), today), "yesterday 09:05");
    assert_eq!(when_label(civil(2026, 11, 3, 2), today), "Tue 09:05");
    assert_eq!(when_label(civil(2027, 1, 2, 6), today), "2027-01-02 09:05");
    assert_eq!(days(1970, 1, 1), 0);
    assert_eq!(days(2000, 3, 1), 11_017);
}

#[test]
fn says_how_far() {
    assert_eq!(relative(1000, 1000), "now");
    assert_eq!(relative(1000 + 300, 1000), "in 5m");
    assert_eq!(relative(1000 + 7500, 1000), "in 2h 5m");
    assert_eq!(relative(1000 + 7200, 1000), "in 2h");
    assert_eq!(relative(1000, 1000 + 90_000), "1d 1h ago");
}

#[test]
fn local_time_round_trips_the_date() {
    let at = local(1_800_000_000).expect("localtime");
    assert!((2027..=2027).contains(&at.year));
    assert!(at.hour < 24 && at.minute < 60 && at.weekday < 7);
}

fn draft() -> Draft {
    Draft {
        edit: None,
        issue: false,
        text: "review the login fix".into(),
        source: None,
        when: When::In,
        clock: String::new(),
        date: String::new(),
        amount: "2".into(),
        unit: 'h',
        target: Target::Workspace(7),
        agent: None,
        mode: None,
        name: Some(String::new()),
    }
}

#[test]
fn builds_queue_add() {
    let d = draft();
    assert_eq!(
        d.args().map(|a| a.join(" ")),
        Ok("queue add --at in 2h --prompt review the login fix --workspace 7 --json".to_string())
    );
    let d = Draft {
        issue: true,
        text: "ENG-42".into(),
        source: Some("linear".into()),
        when: When::On,
        date: "2026-10-24".into(),
        clock: "9:00".into(),
        target: Target::Worktree { project: 3, branch: "feat/x".into() },
        agent: Some("codex".into()),
        mode: Some("plan".into()),
        name: Some("night run".into()),
        ..draft()
    };
    assert_eq!(
        d.args().map(|a| a.join(" ")),
        Ok("queue add --at 2026-10-24 9:00 --issue ENG-42 --source linear --project 3 --worktree feat/x \
         --agent codex --mode plan --name night run --json"
            .to_string())
    );
    assert_eq!(Draft { when: When::Tomorrow, clock: "9:30".into(), ..draft() }.at(), Ok(Some("tomorrow 9:30".into())));
    assert_eq!(Draft { when: When::ClaudeReset, ..draft() }.at(), Ok(Some("claude reset".into())));
}

#[test]
fn refuses_an_incomplete_form() {
    assert!(Draft { text: " ".into(), ..draft() }.args().is_err());
    assert!(Draft { amount: "0".into(), ..draft() }.args().is_err());
    assert!(Draft { when: When::At, ..draft() }.args().is_err());
    assert!(Draft { target: Target::Worktree { project: 1, branch: " ".into() }, ..draft() }.args().is_err());
}

#[test]
fn an_edit_sends_only_what_changed() {
    let d = Draft { edit: Some(4), text: String::new(), when: When::Keep, target: Target::Keep, name: None, ..draft() };
    assert_eq!(d.args().map(|a| a.join(" ")), Ok("queue edit 4 --json".to_string()));
    let d = Draft { edit: Some(4), when: When::At, clock: "14:30".into(), target: Target::Keep, ..draft() };
    assert_eq!(
        d.args().map(|a| a.join(" ")),
        Ok("queue edit 4 --at 14:30 --prompt review the login fix --json".to_string())
    );
}

#[test]
fn reads_agents_and_modes_from_the_settings() {
    let config = json!({ "settings": [
        { "key": "agent", "value": "claude", "choices": [{ "value": "auto" }, { "value": "claude" }] },
        { "key": "agents.claude.mode", "value": "plan", "choices": [{ "value": "default" }, { "value": "plan" }] },
    ]});
    assert_eq!(agent_choices(&config), (vec!["auto".to_string(), "claude".to_string()], "claude".to_string()));
    assert_eq!(mode_choices(&config, "claude"), (vec!["default".into(), "plan".into()], Some("plan".into())));
    assert_eq!(mode_choices(&config, "codex"), (vec![], None));
    assert_eq!(agent_choices(&json!({})), (vec![], "auto".to_string()));
}
