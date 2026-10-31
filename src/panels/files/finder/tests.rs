use gpui::{px, size};

use super::*;
use crate::model::Session;

fn hits(paths: &[&str]) -> Results {
    Results::Names(paths.iter().map(|p| NameHit { path: (*p).to_string(), lit: Vec::new() }).collect())
}

fn answered(paths: &[&str]) -> Pick {
    let mut pick = Pick::default();
    let generation = pick.ask();
    pick.answer(generation, hits(paths));
    pick
}

fn session(shown: u64) -> Session {
    serde_json::from_value(serde_json::json!({
        "projects": [
            {"id": 10, "name": "cornercase-terminal", "path": "/code/cc", "workspaces": [
                {"id": 100, "name": "master", "path": "/code/cc"},
                {"id": 101, "name": "fix-login", "path": "/code/cc-fix-login", "branch": "fix/login", "worktree": true}
            ]}
        ],
        "view": {"id": 1, "projection": 1, "shown": {"project": 10, "workspace": shown}}
    }))
    .expect("the session fixture parses")
}

#[test]
fn it_searches_the_shown_workspace_of_the_selected_project() {
    let session = session(101);
    let (project, workspace) = shown_workspace(Some(&session)).expect("a workspace is shown");
    assert_eq!(workspace.path, "/code/cc-fix-login");
    assert_eq!(place_label(project, workspace), "cornercase-terminal › fix-login");
    assert!(shown_workspace(None).is_none());
}

#[test]
fn an_answer_to_an_older_query_is_dropped() {
    let mut pick = Pick::default();
    let old = pick.ask();
    let new = pick.ask();
    assert_eq!(pick.answer(old, hits(&["old.rs"])), None);
    assert!(pick.running);
    assert_eq!(pick.answer(new, hits(&["new.rs"])), Some(false));
    assert!(!pick.running);
    assert_eq!(pick.hits[0].path, "new.rs");
}

#[test]
fn enter_before_the_answer_opens_the_first_hit_when_it_comes() {
    let mut pick = Pick::default();
    let generation = pick.ask();
    assert_eq!(pick.submit(), None);
    assert_eq!(pick.answer(generation, hits(&["a.rs", "b.rs"])), Some(true));
    assert_eq!(pick.selected, 0);
}

#[test]
fn typing_again_forgets_an_early_enter() {
    let mut pick = Pick::default();
    pick.ask();
    pick.submit();
    pick.retyped();
    let generation = pick.ask();
    assert_eq!(pick.answer(generation, hits(&["a.rs"])), Some(false));
}

#[test]
fn enter_opens_the_chosen_hit() {
    let mut pick = answered(&["a.rs", "b.rs", "c.rs"]);
    assert!(pick.step(1));
    assert_eq!(pick.submit(), Some(1));
    assert_eq!(answered(&[]).submit(), None);
}

#[test]
fn the_arrows_stay_within_the_hits() {
    let mut pick = answered(&["a.rs", "b.rs", "c.rs"]);
    assert!(!pick.step(-1));
    assert!(pick.step(10));
    assert_eq!(pick.selected, 2);
    assert!(!pick.step(1));
    assert!(pick.step(-1));
    assert_eq!(pick.selected, 1);
    assert!(!answered(&[]).step(1));
}

#[test]
fn a_new_answer_starts_from_the_top() {
    let mut pick = answered(&["a.rs", "b.rs"]);
    pick.step(1);
    let generation = pick.ask();
    pick.answer(generation, hits(&["c.rs", "d.rs"]));
    assert_eq!(pick.selected, 0);
}

#[test]
fn clearing_the_query_drops_the_hits_and_any_answer_on_its_way() {
    let mut pick = answered(&["a.rs"]);
    let generation = pick.ask();
    pick.clear();
    assert!(pick.hits.is_empty() && !pick.running);
    assert_eq!(pick.answer(generation, hits(&["b.rs"])), None);
}

#[test]
fn the_keys_move_by_one_or_by_a_page() {
    assert_eq!(step_of("down", 9), Some(1));
    assert_eq!(step_of("up", 9), Some(-1));
    assert_eq!(step_of("pagedown", 9), Some(9));
    assert_eq!(step_of("pageup", 9), Some(-9));
    assert_eq!(step_of("enter", 9), None);
}

#[test]
fn the_status_says_what_the_finder_waits_for() {
    let idle = Pick::default();
    assert_eq!(status(false, "x", false, &idle), Status::Nowhere);
    assert_eq!(status(true, "  ", true, &idle), Status::Hint);
    assert_eq!(status(true, "x", true, &idle), Status::Reading);
    let mut running = Pick::default();
    running.ask();
    assert_eq!(status(true, "x", false, &running), Status::Searching);
    assert_eq!(status(true, "x", false, &answered(&["a.rs"])), Status::Found("1 file".into()));
    let mut failed = Pick::default();
    let generation = failed.ask();
    failed.answer(generation, Results::Failed("ssh: connection refused".into()));
    assert_eq!(status(true, "x", false, &failed), Status::Failed("ssh: connection refused".into()));
}

#[test]
fn the_card_fits_the_window() {
    let wide = frame_for(size(px(1280.), px(800.)));
    assert_eq!((wide.width, wide.height), (640., 440.));
    assert!(wide.page >= 10);
    let small = frame_for(size(px(640.), px(360.)));
    assert!(small.width <= 640. - 48. && small.height <= 360. - 96.);
    assert!(small.page >= 1);
}
