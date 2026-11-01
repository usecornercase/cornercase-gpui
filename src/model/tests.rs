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
fn a_pane_whose_folder_is_gone_keeps_the_session_readable() {
    let text = r#"{"projects":[{"id":1,"name":"shop","workspaces":[{"id":2,"name":"feat",
        "tabs":[{"id":3,"panes":[{"id":4,"path":null,"program":null}]}]}]}]}"#;
    let session: Session = parse(text);
    assert_eq!(session.projects[0].workspaces[0].tabs[0].panes[0].path, None);
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

#[test]
fn a_project_shows_only_a_status_that_needs_you() {
    let tab = |status: &str| Tab { status: Some(status.into()), ..Default::default() };
    let project =
        |tabs: Vec<Tab>| Project { workspaces: vec![Workspace { tabs, ..Default::default() }], ..Default::default() };
    assert_eq!(project(vec![tab("working")]).attention(), None);
    assert_eq!(project(vec![tab("working"), tab("done")]).attention(), Some("done"));
}

#[test]
fn sections_put_loose_projects_first_and_keep_empty_groups() {
    let session = Session {
        groups: vec![Group { id: 10, ..Default::default() }, Group { id: 11, ..Default::default() }],
        projects: vec![
            Project { id: 1, group: Some(10), ..Default::default() },
            Project { id: 2, ..Default::default() },
            Project { id: 3, group: Some(99), ..Default::default() },
        ],
        ..Default::default()
    };
    assert_eq!(session.sections(), vec![(None, vec![2, 3]), (Some(10), vec![1]), (Some(11), vec![])]);
}

#[test]
fn agent_titles_lose_their_spinner() {
    let pane = |t: &str| PaneInfo { title: Some(t.into()), ..Default::default() };
    assert_eq!(pane("✳ Fix the login").agent_title().as_deref(), Some("Fix the login"));
    assert_eq!(pane("⠋ ").agent_title(), None);
}

#[test]
fn the_session_says_whose_home_folder_it_runs_in() {
    let session: Session = parse(r#"{"home": "/Users/ana", "projects": []}"#);
    assert_eq!(session.home.as_deref(), Some("/Users/ana"));
}

fn viewing(tab: Option<u64>, projection: u64) -> Session {
    let shown = Shown { project: Some(1), workspace: Some(2), tab, pane: None };
    Session { view: Some(ViewInfo { projection, shown, ..Default::default() }), ..Default::default() }
}

#[test]
fn a_view_that_shows_no_tab_hides_the_frame_it_showed_before() {
    assert!(viewing(None, 5).shows_no_tab(Some(4)));
    assert!(viewing(None, 5).shows_no_tab(None));
}

#[test]
fn a_frame_newer_than_the_session_shows_the_tab_it_came_for() {
    assert!(!viewing(None, 5).shows_no_tab(Some(6)));
}

#[test]
fn a_view_that_shows_a_tab_has_no_empty_page() {
    assert!(!viewing(Some(7), 5).shows_no_tab(None));
    assert!(!Session::default().shows_no_tab(None));
}

#[test]
fn the_grid_keeps_the_projection_of_its_last_frame() {
    let mut frame: serde_json::Value = parse(crate::test_util::fixture!("frame.json"));
    frame["projection"] = 4.into();
    let mut grid = Grid::default();
    assert!(matches!(grid.apply(serde_json::from_value(frame).expect("a frame")), Applied::Ok));
    assert_eq!(grid.projection, Some(4));
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
fn keeps_the_split_tree_of_the_fixture_frame() {
    let split = |first, second| Node::Split(Box::new(Branch { dir: "right".into(), first, second }));
    assert_eq!(fixture_grid().layout, Some(split(Node::Leaf(13), Node::Leaf(14))));
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

fn popup_grid() -> Grid {
    let mut grid = Grid::default();
    assert!(matches!(grid.apply(parse(crate::test_util::fixture!("surface/popup.json"))), Applied::Ok));
    grid
}

#[test]
fn a_popup_frame_names_its_plugin_and_holds_one_pane() {
    let grid = popup_grid();
    let popup = grid.popup.as_ref().expect("a popup");
    assert_eq!((popup.pane, popup.plugin.as_str(), popup.action.as_str()), (40, "acme.notes", "browse"));
    assert_eq!((grid.tab, grid.cols, grid.rows, grid.layout.is_none()), (0, 60, 10, true));
    assert_eq!(grid.panes.iter().map(|p| p.pane).collect::<Vec<_>>(), [40]);
    assert!(cell(&grid.panes[0], 0, 1).1.style.bold);
}

#[test]
fn a_popup_patch_keeps_the_popup_and_changes_its_rows() {
    let mut grid = popup_grid();
    assert!(matches!(grid.apply(parse(crate::test_util::fixture!("surface/popup-patch.json"))), Applied::Ok));
    assert_eq!(grid.popup.as_ref().map(|p| p.pane), Some(40));
    assert_eq!(cell(&grid.panes[0], 2, 0).0, "n");
    assert_eq!(grid.panes[0].cursor.map(|c| c.x), Some(3));
}

#[test]
fn a_tab_frame_has_no_popup() {
    assert!(fixture_grid().popup.is_none());
}

#[test]
fn the_app_feed_says_which_popup_is_open() {
    let app: serde_json::Value = parse(crate::test_util::fixture!("app/popup.json"));
    let open = OpenPopup::in_app(Some(&app)).expect("a popup");
    assert_eq!((open.plugin.as_str(), open.action.as_str(), open.cols, open.rows), ("acme.notes", "browse", 60, 10));
}

#[test]
fn no_popup_in_the_app_feed_means_none_is_open() {
    let app: serde_json::Value = parse(crate::test_util::fixture!("app/snapshot.json"));
    assert_eq!(OpenPopup::in_app(Some(&app)), None);
    assert_eq!(OpenPopup::in_app(Some(&serde_json::json!({"popup": null}))), None);
    assert_eq!(OpenPopup::in_app(None), None);
}

#[test]
fn the_session_tree_hangs_a_child_under_its_parent_in_another_workspace() {
    let pane =
        |id: u64, parent: Option<u64>| PaneInfo { id, agent: Some("claude".into()), parent, ..Default::default() };
    let ws = |id: u64, tab: u64, panes: Vec<PaneInfo>| Workspace {
        id,
        tabs: vec![Tab { id: tab, panes, ..Default::default() }],
        ..Default::default()
    };
    let project = |id: u64, workspaces: Vec<Workspace>| Project { id, workspaces, ..Default::default() };
    let session = Session {
        projects: vec![
            project(1, vec![ws(10, 11, vec![pane(20, None)]), ws(12, 13, vec![pane(21, Some(22))])]),
            project(2, vec![ws(14, 15, vec![pane(22, None)])]),
        ],
        ..Default::default()
    };
    let rows: Vec<(u64, u64, usize)> =
        session.agent_rows(&[2, 1]).iter().map(|r| (r.pane.id, r.workspace, r.depth)).collect();
    assert_eq!(rows, [(22, 14, 0), (21, 12, 1), (20, 10, 0)]);
}

#[test]
fn a_working_pane_counts_even_when_its_tab_shows_a_louder_status() {
    let pane = |status: &str| PaneInfo { status: Some(status.into()), ..Default::default() };
    let session = |panes: Vec<PaneInfo>| Session {
        projects: vec![Project {
            workspaces: vec![Workspace {
                tabs: vec![Tab { status: Some("waiting".into()), panes, ..Default::default() }],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert!(session(vec![pane("waiting"), pane("working")]).anyone_working());
    assert!(!session(vec![pane("waiting"), pane("done")]).anyone_working());
}
