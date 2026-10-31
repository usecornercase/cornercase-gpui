use std::ops::Range;

use super::candidates::{candidates, tilde};
use super::found::{Found, Hit, ago_from, by_pane, parse_time, row_of_match};
use super::names::{Candidate, Goto, Kind, Ranked, context_top, find, key_rank, listing_status, rank, shown_row};
use crate::model::{Session, Shown};

fn session() -> Session {
    serde_json::from_value(serde_json::json!({
        "home": "/Users/ana",
        "groups": [{"id": 1, "name": "Work"}],
        "projects": [
            {"id": 11, "name": "gpui", "path": "/Users/ana/src/gpui-app", "group": 1, "workspaces": [
                {"id": 102, "name": "main", "tabs": [{"id": 1003, "name": "term", "panes": [{"id": 7, "program": "zsh"}]}]}
            ]},
            {"id": 10, "name": "cornercase-terminal", "path": "/Users/ana/src/cornercase-terminal", "workspaces": [
                {"id": 100, "name": "master", "tabs": [
                    {"id": 1000, "name": "zsh", "panes": [{"id": 4, "program": "zsh"}]},
                    {"id": 1001, "name": "claude", "panes": [
                        {"id": 5, "agent": "claude", "title": "✳ Fix the login redirect", "model": "Opus 5.5",
                         "status": "waiting"},
                        {"id": 6, "program": "zsh"}
                    ]}
                ]},
                {"id": 101, "name": "fix-login", "branch": "fix/login", "worktree": true,
                 "pull_request": {"number": 485, "title": "Dark mode", "state": "open"},
                 "tabs": [
                    {"id": 1002, "name": "review", "panes": [
                        {"id": 8, "agent": "codex", "role": "reviewer", "status": "working"}
                    ]},
                    {"id": 1004, "name": "server", "panes": [{"id": 9, "program": "node"}]}
                 ]}
            ]}
        ]
    }))
    .expect("the session fixture parses")
}

fn found(query: &str) -> Vec<Ranked> {
    rank(candidates(&session()), query)
}

fn names(query: &str) -> Vec<String> {
    found(query).into_iter().map(|r| r.candidate.name).collect()
}

fn candidate(goto: Goto) -> Candidate {
    candidates(&session()).into_iter().find(|c| c.goto == goto).expect("a candidate going there")
}

fn marked<'a>(text: &'a str, ranges: &[Range<usize>]) -> Vec<&'a str> {
    ranges.iter().map(|r| &text[r.clone()]).collect()
}

const FIX_LOGIN: Goto = Goto::Agent { pane: 5, tab: 1001 };
const REVIEW: Goto = Goto::Agent { pane: 8, tab: 1002 };

#[test]
fn the_empty_query_lists_the_session_in_the_sidebars_order() {
    let listed: Vec<(Kind, Goto)> = found("  ").iter().map(|r| (r.candidate.kind, r.candidate.goto)).collect();
    assert_eq!(
        listed,
        [
            (Kind::Project, Goto::Project(10)),
            (Kind::Workspace, Goto::Workspace(100)),
            (Kind::Tab, Goto::Tab(1000)),
            (Kind::Agent, FIX_LOGIN),
            (Kind::Workspace, Goto::Workspace(101)),
            (Kind::Agent, REVIEW),
            (Kind::Tab, Goto::Tab(1004)),
            (Kind::Group, Goto::Group { id: 1, first: Some(11) }),
            (Kind::Project, Goto::Project(11)),
            (Kind::Workspace, Goto::Workspace(102)),
            (Kind::Tab, Goto::Tab(1003)),
        ]
    );
}

#[test]
fn the_empty_query_starts_on_what_the_window_shows() {
    let listed = found("");
    let shown = |pane, tab, workspace| Shown { project: Some(10), workspace, tab, pane };
    assert_eq!(shown_row(&listed, shown(Some(5), Some(1001), Some(100))), Some(3));
    assert_eq!(shown_row(&listed, shown(Some(6), Some(1001), Some(100))), Some(3));
    assert_eq!(shown_row(&listed, shown(Some(4), Some(1000), Some(100))), Some(2));
    assert_eq!(shown_row(&listed, shown(None, None, Some(101))), Some(4));
    assert_eq!(shown_row(&listed, shown(None, None, None)), Some(0));
    assert_eq!(shown_row(&listed, Shown::default()), None);
}

#[test]
fn the_empty_query_opens_on_the_shown_rows_project() {
    let listed = found("");
    assert_eq!(context_top(&listed, 3), 0);
    assert_eq!(context_top(&listed, 10), 8);
    assert_eq!(context_top(&listed, 8), 8);
    let mut session = session();
    let tab = session.projects[1].workspaces[0].tabs[0].clone();
    session.projects[1].workspaces[0].tabs = vec![tab; 12];
    assert_eq!(context_top(&rank(candidates(&session), ""), 12), 7);
}

#[test]
fn an_agent_is_named_by_its_conversation_and_found_by_it() {
    let hit = &found("login redirect")[0];
    assert_eq!(hit.candidate.goto, FIX_LOGIN);
    assert_eq!(hit.candidate.name, "Fix the login redirect");
    assert_eq!(hit.candidate.detail, "claude · Opus 5.5");
    assert_eq!(hit.candidate.place, "cornercase-terminal › master");
    assert_eq!(hit.candidate.status.as_deref(), Some("waiting"));
    assert_eq!(marked(&hit.candidate.name, &hit.marks.name), ["login redirect"]);
}

#[test]
fn an_agent_without_a_conversation_takes_its_tabs_name() {
    let review = candidate(REVIEW);
    assert_eq!((review.name.as_str(), review.detail.as_str()), ("review", "codex · reviewer"));
    assert_eq!(review.place, "cornercase-terminal › fix-login");
}

#[test]
fn agents_are_found_by_their_kind_role_and_model() {
    assert_eq!(found("codex")[0].candidate.goto, REVIEW);
    assert_eq!(found("reviewer")[0].candidate.goto, REVIEW);
    let opus = &found("opus")[0];
    assert_eq!(opus.candidate.goto, FIX_LOGIN);
    assert_eq!(marked(&opus.candidate.detail, &opus.marks.detail), ["Opus"]);
}

#[test]
fn a_tab_holding_an_agent_is_listed_as_its_agent() {
    let gotos: Vec<Goto> = candidates(&session()).into_iter().map(|c| c.goto).collect();
    assert!(!gotos.contains(&Goto::Tab(1001)) && !gotos.contains(&Goto::Tab(1002)));
    assert!(gotos.contains(&FIX_LOGIN) && gotos.contains(&REVIEW));
    assert_eq!(found("claude")[0].candidate.goto, FIX_LOGIN);
}

#[test]
fn an_agent_in_a_renamed_tab_names_the_tab_in_its_place() {
    let mut session = session();
    session.projects[1].workspaces[0].tabs[1].name = "pairing".into();
    let agent = candidates(&session).into_iter().find(|c| c.goto == FIX_LOGIN).expect("the agent");
    assert_eq!(agent.place, "cornercase-terminal › master › pairing");
    let hit = rank(candidates(&session), "pairing").remove(0);
    assert_eq!(marked(&hit.candidate.place, &hit.marks.place), ["pairing"]);
}

#[test]
fn a_shell_tab_is_found_by_its_program() {
    let hit = &found("node")[0];
    assert_eq!(hit.candidate.goto, Goto::Tab(1004));
    assert_eq!(marked(&hit.candidate.detail, &hit.marks.detail), ["node"]);
    assert_eq!(candidate(Goto::Tab(1000)).detail, "");
}

#[test]
fn a_workspace_is_found_by_its_pull_request() {
    let ws = candidate(Goto::Workspace(101));
    assert_eq!((ws.name.as_str(), ws.detail.as_str()), ("⎇ fix-login", "fix/login · #485 Dark mode"));
    assert_eq!(found("#485")[0].candidate.goto, Goto::Workspace(101));
    assert_eq!(found("dark mode")[0].candidate.goto, Goto::Workspace(101));
}

#[test]
fn a_project_is_found_by_its_folder_shown_from_home() {
    let hit = &found("gpui-app")[0];
    assert_eq!(hit.candidate.goto, Goto::Project(11));
    assert_eq!((hit.candidate.detail.as_str(), hit.candidate.place.as_str()), ("~/src/gpui-app", "Work"));
    assert_eq!(marked(&hit.candidate.detail, &hit.marks.detail), ["gpui-app"]);
}

#[test]
fn paths_under_home_start_with_a_tilde() {
    assert_eq!(tilde("/Users/ana/src/x", Some("/Users/ana")), "~/src/x");
    assert_eq!(tilde("/Users/ana", Some("/Users/ana/")), "~");
    assert_eq!(tilde("/Users/anabel/x", Some("/Users/ana")), "/Users/anabel/x");
    assert_eq!(tilde("/srv/x", None), "/srv/x");
}

#[test]
fn a_group_shows_its_loudest_project_that_needs_you() {
    let mut session = session();
    assert_eq!(candidates(&session).into_iter().find(|c| c.kind == Kind::Group).and_then(|c| c.status), None);
    session.projects[0].workspaces[0].tabs[0].panes[0].status = Some("failed".into());
    let group = candidates(&session).into_iter().find(|c| c.kind == Kind::Group).expect("the group");
    assert_eq!((group.detail.as_str(), group.status.as_deref()), ("1 project", Some("failed")));
}

#[test]
fn letters_in_order_count_only_in_the_name() {
    assert_eq!(names("os5"), Vec::<String>::new());
    assert_eq!(names("fxlg"), ["⎇ fix-login", "Fix the login redirect"]);
}

#[test]
fn a_match_outside_the_name_is_marked_where_it_shows() {
    let hits = found("master");
    assert_eq!(hits[0].candidate.goto, Goto::Workspace(100));
    let tab = hits.iter().find(|r| r.candidate.goto == Goto::Tab(1000)).expect("the tab in master");
    assert_eq!(marked(&tab.candidate.place, &tab.marks.place), ["master"]);
    assert_eq!(tab.marks.name, Vec::<Range<usize>>::new());
}

#[test]
fn ranks_exact_then_prefix_then_word_then_anywhere_then_fuzzy() {
    assert_eq!(key_rank("zsh", "ZSH").map(|r| r.0), Some(0));
    assert_eq!(key_rank("master", "mas").map(|r| r.0), Some(1));
    assert_eq!(key_rank("fix/login", "log"), Some((2, vec![Range { start: 4, end: 7 }])));
    assert_eq!(key_rank("cornercase", "case"), Some((3, vec![Range { start: 6, end: 10 }])));
    assert_eq!(key_rank("cornercase-terminal", "cct").map(|r| r.0), Some(4));
    assert_eq!(key_rank("gpui", "xyz"), None);
    assert_eq!(key_rank("gpui", ""), None);
    assert_eq!(key_rank("ab", "abc"), None);
}

#[test]
fn ties_go_by_kind_then_order() {
    assert_eq!(names("c"), ["cornercase-terminal", "Fix the login redirect", "review"]);
    assert_eq!(names("main")[0], "main");
    assert_eq!(names("zsh")[0], "zsh");
    let hits = found("fix/login");
    let kinds: Vec<Kind> = hits.iter().map(|r| r.candidate.kind).collect();
    assert_eq!(kinds, [Kind::Workspace, Kind::Agent, Kind::Tab]);
    assert_eq!(hits[1].marks.name, Vec::<Range<usize>>::new());
}

#[test]
fn highlights_land_on_the_name() {
    let hit = &found("login")[0];
    assert_eq!(hit.candidate.name, "⎇ fix-login");
    assert_eq!(marked(&hit.candidate.name, &hit.marks.name), ["login"]);
}

#[test]
fn choosing_an_agent_focuses_its_pane() {
    assert_eq!(FIX_LOGIN.focus(), Some(("pane", 5)));
    assert_eq!(Goto::Tab(1000).focus(), Some(("tab", 1000)));
    assert_eq!(Goto::Group { id: 1, first: Some(11) }.focus(), Some(("project", 11)));
    assert_eq!(Goto::Group { id: 2, first: None }.focus(), None);
}

#[test]
fn the_status_says_what_the_list_holds() {
    assert!(listing_status("", 11, 11).starts_with("Everything open, in the sidebar's order"));
    assert_eq!(listing_status("", 200, 640), "The first 200 of 640 in the sidebar's order · type to find the rest");
    assert_eq!(listing_status(" ", 0, 0), "No project is open: open one with ⌘O");
    assert_eq!(listing_status("zz", 0, 0), "Nothing matches “zz”");
    assert_eq!(listing_status("a", 1, 1), "1 result");
    assert_eq!(listing_status("a", 200, 300), "The first 200 of 300 results");
}

#[test]
fn finds_ignoring_case_on_byte_ranges() {
    assert_eq!(find("Migration FAILED here", "failed"), Some(10..16));
    assert_eq!(find("…ça marche", "MARCHE"), Some(7..13));
    assert_eq!(find("abc", "zz"), None);
    assert_eq!(find("abc", "  "), None);
}

#[test]
fn reads_and_groups_search_output() {
    let text = r#"{"capped": true, "matches": [
        {"line": null, "pane": 1, "place": "p › w › claude", "project": 4, "source": "reply", "tab": 2,
         "text": "a", "who": "claude", "workspace": 3, "written": "2026-10-23T17:48:54.840Z"},
        {"line": 40, "pane": 7, "place": "p › w › zsh", "source": "screen", "text": "b", "who": "zsh", "written": null},
        {"line": 12, "pane": 1, "place": "p › w › claude", "source": "screen", "text": "c", "who": "claude"}
    ], "future": 1}"#;
    let found: Found = serde_json::from_str(text).expect("search output parses");
    assert!(found.capped);
    let groups = by_pane(&found.matches);
    assert_eq!(groups.iter().map(|g| g.pane).collect::<Vec<_>>(), [1, 7]);
    assert_eq!(groups[0].hits.iter().map(|h| h.text.as_str()).collect::<Vec<_>>(), ["a", "c"]);
    assert_eq!(groups[1].who, "zsh");
    assert_eq!(groups[1].hits[0].line, Some(40));
    assert_eq!(row_of_match(&groups, 0), 1);
    assert_eq!(row_of_match(&groups, 1), 2);
    assert_eq!(row_of_match(&groups, 2), 4);
}

#[test]
fn reads_times_and_says_how_long_ago() {
    assert_eq!(parse_time("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(parse_time("2026-10-23T17:48:54.840Z"), Some(1_792_777_734));
    assert_eq!(parse_time("2026-10-23T19:48:54+02:00"), Some(1_792_777_734));
    assert_eq!(parse_time("yesterday"), None);
    assert_eq!(ago_from(100, 130), "just now");
    assert_eq!(ago_from(0, 300), "5m ago");
    assert_eq!(ago_from(0, 7200), "2h ago");
    assert_eq!(ago_from(0, 3 * 86_400), "3d ago");
}

#[test]
fn a_screen_match_is_revealed_at_its_line() {
    let hit = Hit { pane: 7, source: "screen".into(), line: Some(340), ..Default::default() };
    assert_eq!(
        hit.reveal_args("migration failed"),
        ["focus", "--pane", "7", "--reveal=migration failed", "--line=340"]
    );
}

#[test]
fn an_agent_match_is_revealed_where_the_pane_last_shows_it() {
    let hit =
        Hit { pane: 5, source: "reply".into(), written: Some("2026-10-23T17:48:54Z".into()), ..Default::default() };
    assert_eq!(hit.reveal_args("-v"), ["focus", "--pane", "5", "--reveal=-v"]);
}
