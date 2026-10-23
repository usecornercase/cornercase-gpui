use std::ops::Range;

use super::found::{Found, ago_from, by_pane, parse_time, row_of_match};
use super::names::{Goto, Kind, candidates, find, key_rank, rank};
use crate::model::Session;

fn session() -> Session {
    serde_json::from_value(serde_json::json!({
        "groups": [{"id": 1, "name": "Work"}],
        "projects": [
            {"id": 10, "name": "cornercase-terminal", "workspaces": [
                {"id": 100, "name": "master", "tabs": [{"id": 1000, "name": "zsh"}, {"id": 1001, "name": "claude",
                    "panes": [{"id": 5, "status": "waiting"}]}]},
                {"id": 101, "name": "fix-login", "branch": "fix/login", "worktree": true, "tabs": [{"id": 1002, "name": "codex"}]}
            ]},
            {"id": 11, "name": "gpui", "group": 1, "workspaces": [
                {"id": 102, "name": "main", "tabs": [{"id": 1003, "name": "term"}]}
            ]}
        ]
    }))
    .expect("the session fixture parses")
}

fn names(query: &str) -> Vec<String> {
    rank(candidates(&session()), query).into_iter().map(|r| r.candidate.name).collect()
}

#[test]
fn candidates_follow_the_sidebar() {
    let c = candidates(&session());
    assert_eq!(c[0].goto, Goto::Group { id: 1, first: Some(11) });
    assert_eq!(c[0].status, None);
    let kinds: Vec<Kind> = c.iter().map(|c| c.kind).collect();
    assert_eq!(kinds.iter().filter(|k| **k == Kind::Tab).count(), 4);
    let tab = c.iter().find(|c| c.goto == Goto::Tab(1001)).expect("tab 1001");
    assert_eq!(tab.context, "cornercase-terminal › master");
    assert_eq!(tab.status.as_deref(), Some("waiting"));
    let ws = c.iter().find(|c| c.goto == Goto::Workspace(101)).expect("workspace 101");
    assert_eq!(ws.name, "⎇ fix-login");
    assert_eq!(ws.keys, ["fix-login", "fix/login"]);
    assert!(c.iter().find(|c| c.goto == Goto::Tab(1002)).expect("tab 1002").keys.contains(&"fix/login".to_string()));
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
    assert_eq!(names("c"), ["cornercase-terminal", "claude", "codex"]);
    assert_eq!(names("main")[0], "main");
    assert_eq!(names("zsh"), ["zsh"]);
    assert_eq!(names(""), Vec::<String>::new());
    let found = rank(candidates(&session()), "fix/login");
    assert_eq!(found[0].candidate.name, "⎇ fix-login");
    assert_eq!(found.len(), 2);
    assert_eq!(found[1].highlight, Vec::<Range<usize>>::new());
}

#[test]
fn highlights_land_on_the_name() {
    let found = rank(candidates(&session()), "login");
    let ws = &found[0];
    assert_eq!(ws.candidate.name, "⎇ fix-login");
    let r = ws.highlight[0].clone();
    assert_eq!(&ws.candidate.name[r], "login");
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
