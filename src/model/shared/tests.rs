use crate::model::{Session, Shared, Workspace};
use crate::test_util::fixture;

fn bench() -> Session {
    serde_json::from_str(fixture!("merge_check/view.json")).expect("a view line")
}

fn workspace(session: &Session, id: u64) -> &Workspace {
    session.workspace(id).expect("a workspace").1
}

#[test]
fn a_view_line_gives_each_workspace_what_it_shares_with_each_other() {
    let session = bench();
    let checkout = workspace(&session, 15);
    assert_eq!(checkout.shared.len(), 2);
    assert_eq!(checkout.shared[1].workspace, 18);
    assert_eq!(checkout.shared[1].count, 2);
    assert_eq!(checkout.shared[1].files, ["README.md", "notes.txt"]);
}

#[test]
fn a_workspace_that_shares_nothing_has_no_entry() {
    let session: Session =
        serde_json::from_str(r#"{"projects":[{"id":1,"workspaces":[{"id":2,"name":"main"}]}]}"#).expect("a view line");
    assert_eq!(workspace(&session, 2).shared_count(), 0);
}

#[test]
fn the_row_counts_files_and_not_workspaces() {
    let session = bench();
    assert_eq!(workspace(&session, 15).shared_count(), 2);
    assert_eq!(workspace(&session, 7).shared_count(), 1);
}

#[test]
fn a_pair_cut_short_still_counts_every_file_it_shares() {
    let ws = Workspace {
        shared: vec![Shared { workspace: 2, count: 140, files: vec!["a.rs".into(), "b.rs".into()] }],
        ..Workspace::default()
    };
    assert_eq!(ws.shared_count(), 140);
}

#[test]
fn the_row_tip_lists_the_files_each_pair_shares() {
    let session = bench();
    assert_eq!(
        session.shared_tip(workspace(&session, 15)),
        "With main: 1 file\n  README.md\nWith mc/pricing: 2 files\n  README.md\n  notes.txt\nClick for the merge check"
    );
}

#[test]
fn the_row_tip_says_how_many_it_leaves_out() {
    let files: Vec<String> = (0..8).map(|n| format!("{n}.rs")).collect();
    let ws = Workspace { shared: vec![Shared { workspace: 9, count: 8, files }], ..Workspace::default() };
    let tip = Session::default().shared_tip(&ws);
    assert!(tip.starts_with("With workspace 9: 8 files\n  0.rs\n"), "{tip}");
    assert!(tip.contains("  5.rs\n  … and 2 more\n"), "{tip}");
    assert!(!tip.contains("6.rs"), "{tip}");
}

#[test]
fn the_row_tip_counts_files_past_the_listed_ones() {
    let ws = Workspace {
        shared: vec![Shared { workspace: 9, count: 140, files: vec!["a.rs".into()] }],
        ..Workspace::default()
    };
    assert!(Session::default().shared_tip(&ws).contains("  a.rs\n  … and 139 more\n"));
}

#[test]
fn a_file_names_the_workspaces_that_change_it_too() {
    let session = bench();
    assert_eq!(session.file_shared_tip(15, "README.md", None).as_deref(), Some("Also changed in main, mc/pricing"));
    assert_eq!(session.file_shared_tip(15, "notes.txt", None).as_deref(), Some("Also changed in mc/pricing"));
}

#[test]
fn a_file_no_other_workspace_changes_has_no_mark() {
    assert_eq!(bench().file_shared_tip(15, "cart.txt", None), None);
}

#[test]
fn a_rename_counts_by_its_old_path() {
    assert_eq!(
        bench().file_shared_tip(15, "docs/README.md", Some("README.md")).as_deref(),
        Some("Also changed in main, mc/pricing")
    );
}

#[test]
fn a_workspace_the_session_does_not_list_has_no_marks() {
    assert_eq!(bench().file_shared_tip(99, "README.md", None), None);
}
