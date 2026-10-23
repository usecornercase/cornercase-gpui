use super::*;

fn parse<T: serde::de::DeserializeOwned>(text: &str) -> T {
    serde_json::from_str(text).expect("valid JSON")
}

fn cell(pane: &PaneGrid, x: u16, y: u16) -> (&str, &Run) {
    pane.cell(x, y).expect("a cell there")
}

#[test]
fn reads_workspace_details_from_the_session() {
    let text = r#"{"projects":[{"id":1,"name":"shop","custom_name":"shop","workspaces":[{"id":2,"name":"feat",
        "behind":3,"setup":"running","changes":{"added":4,"removed":1,"files":2,"mode":"uncommitted"},
        "pull_request":{"number":485,"title":"Dark mode","url":"https://x/485","state":"open","checks":"failing",
        "failed":["test"],"conflicts":false,"base":"main","provider":"github","stack":null},
        "tabs":[{"id":3,"panes":[{"id":4,"agent":"claude","subagents":2,"role":"coder","parent":null}]}]}]}]}"#;
    let session: Session = parse(text);
    let ws = &session.projects[0].workspaces[0];
    let pr = ws.pull_request.as_ref().expect("a pull request");
    assert_eq!((pr.number, pr.checks.as_deref(), pr.failed.len()), (485, Some("failing"), 1));
    assert_eq!(ws.setup.as_deref(), Some("running"));
    assert_eq!(ws.changes.as_ref().map(|c| c.added), Some(4));
    assert_eq!(ws.tabs[0].panes[0].subagents, 2);
    assert_eq!(session.projects[0].custom_name.as_deref(), Some("shop"));
}

#[test]
fn agents_form_a_tree_and_folds_hide_children() {
    let pane =
        |id: u64, parent: Option<u64>| PaneInfo { id, agent: Some("claude".into()), parent, ..Default::default() };
    let shell = PaneInfo { id: 9, ..Default::default() };
    let parent_elsewhere = Tab { id: 4, panes: vec![pane(13, Some(99))], ..Default::default() };
    let ws = Workspace {
        tabs: vec![
            Tab { id: 1, panes: vec![pane(10, None), shell], ..Default::default() },
            Tab { id: 2, panes: vec![pane(11, Some(10))], ..Default::default() },
            Tab { id: 3, panes: vec![pane(12, Some(11))], ..Default::default() },
            parent_elsewhere,
        ],
        ..Default::default()
    };
    let rows: Vec<(u64, usize, usize)> = ws.agent_rows().iter().map(|r| (r.pane.id, r.depth, r.children)).collect();
    assert_eq!(rows, [(10, 0, 1), (11, 1, 1), (12, 2, 0), (13, 0, 0)]);

    let mut folded = ws.clone();
    folded.tabs[1].panes[0].collapsed = true;
    let rows: Vec<u64> = folded.agent_rows().iter().map(|r| r.pane.id).collect();
    assert_eq!(rows, [10, 11, 13]);
}

fn fixture_grid() -> Grid {
    let frame: Frame = parse(crate::test_util::fixture!("frame.json"));
    let mut grid = Grid::default();
    assert!(matches!(grid.apply(frame), Applied::Ok));
    grid
}

#[test]
fn reads_the_fixture_frame() {
    let grid = fixture_grid();
    assert_eq!(grid.panes.len(), 2);
    let pane = &grid.panes[0];
    assert_eq!(cell(pane, 1, 1).0, "k");
    assert!(cell(pane, 1, 1).1.style.bold);
    assert_eq!(cell(pane, 3, 1).0, "中");
    assert_eq!(cell(pane, 2, 2).1.link.as_deref(), None);
    assert_eq!(cell(pane, 3, 2).1.link.as_deref(), Some("https://example.com/docs/a-long-page"));
    assert_eq!(pane.cursor.map(|c| c.shape), Some(CursorShape::Bar));
    assert_eq!(pane.colours.palette.get("1"), Some(&[255, 0, 0]));
}

#[test]
fn applies_a_patch_with_a_copy() {
    let full = r#"{"boot":"b","revision":1,"surface":3,"tab":1,"cols":4,"rows":3,"styles":[{}],
        "panes":[{"pane":7,"active":true,"rect":{"x":0,"y":0,"cols":4,"rows":3},"size":{"cols":4,"rows":3},
        "cursor":null,"lines":[[{"style":0,"cells":["a"]}],[{"style":0,"cells":["b"]}],[{"style":0,"cells":["c"]}]]}]}"#;
    let patch = r#"{"boot":"b","revision":2,"base":1,"surface":3,"tab":1,"cols":4,"rows":3,
        "styles":[{"bold":true}],"panes":[{"pane":7,"cursor":{"x":1,"y":2},
        "copy":{"from":1,"to":0,"count":2},"lines":{"2":[{"style":0,"cells":["d"]}]}}]}"#;
    let mut grid = Grid::default();
    grid.apply(parse(full));
    assert!(matches!(grid.apply(parse(patch)), Applied::Ok));
    let pane = &grid.panes[0];
    let text: Vec<&str> = (0..3).map(|y| cell(pane, 0, y).0).collect();
    assert_eq!(text, ["b", "c", "d"]);
    assert!(cell(pane, 0, 2).1.style.bold);
    assert_eq!(pane.cursor.map(|c| c.x), Some(1));
    assert_eq!(grid.revision, 2);
    let stale = patch.replace("\"revision\":2,\"base\":1", "\"revision\":9,\"base\":5");
    assert!(matches!(grid.apply(parse(&stale)), Applied::NeedRedraw));
}
