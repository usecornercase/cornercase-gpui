use super::*;

fn session(text: &str) -> Session {
    serde_json::from_str(text).expect("valid JSON")
}

const STACKED: &str = r#"{"projects":[{"id":1,"workspaces":[
    {"id":2,"name":"tokens","branch":"tokens","pull_request":{"number":483,"url":"https://x/483"}},
    {"id":3,"name":"dark","branch":"dark","behind":1,"pull_request":{"number":485,"url":"https://x/485","state":"open",
        "stack":{"number":480,"base":"main","position":2,"size":3,"layers":[
            {"number":483,"url":"https://x/483"},{"number":485,"url":"https://x/485"},{"number":486,"url":"https://x/486"}]}}},
    {"id":4,"name":"detached","pull_request":null}]}]}"#;

#[test]
fn each_layer_names_the_workspace_that_holds_it_top_first() {
    let shown = Shown::take(&session(STACKED), 3).expect("a pull request");
    let held: Vec<Option<u64>> = shown.holders.iter().map(|h| h.as_ref().map(|(id, _)| *id)).collect();
    assert_eq!(held, [None, Some(3), Some(2)]);
    assert_eq!((shown.branch.as_deref(), shown.behind), (Some("dark"), 1));
}

#[test]
fn a_workspace_without_a_pull_request_shows_no_dialog() {
    assert!(Shown::take(&session(STACKED), 4).is_none());
    assert!(Shown::take(&session(STACKED), 9).is_none());
}

#[test]
fn the_shortcut_says_why_there_is_nothing_to_show() {
    let session = session(STACKED);
    let missing_of = |id| session.workspace(id).and_then(|(_, ws)| missing(ws));
    assert_eq!(missing_of(3), None);
    assert_eq!(missing_of(4).as_deref(), Some("detached is not on a branch"));
    let none = Workspace { name: "feat".into(), branch: Some("feat/x".into()), ..Default::default() };
    assert_eq!(missing(&none).as_deref(), Some("No pull request for feat/x"));
}

#[test]
fn the_body_grows_with_the_window_within_limits() {
    assert_eq!(body_height(800.), 560.);
    assert_eq!(body_height(600.), 360.);
    assert_eq!(body_height(300.), 200.);
    assert_eq!(body_height(1400.), 560.);
}
