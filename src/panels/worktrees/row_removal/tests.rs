use super::{Whose, WorktreeRow, check_job, checked, loading_title};
use crate::panels::worktrees::removal::{Answer, Folder};
use crate::store::Busy;
use crate::test_util::{fixture, vps_aim};

const FOLDER: &str = "/home/ana/.cornercase/worktrees/shop/feat-clean";

fn row(whose: Whose) -> WorktreeRow {
    WorktreeRow { whose, path: FOLDER.to_string(), name: "feat/clean".to_string() }
}

fn clean(machine: Option<&str>) -> Folder {
    let answer = serde_json::from_str::<Answer>(fixture!("worktrees/remove-check.json")).expect("the check parses");
    let found = answer.folders.into_iter().find(|folder| folder.path.ends_with("feat-clean"));
    Folder::asked(FOLDER, machine).answered(Ok(Answer { folders: found.into_iter().collect() }))
}

#[test]
fn a_worktree_here_is_asked_about_by_its_rows_name() {
    assert_eq!(loading_title(&row(Whose::Here)), "Remove the worktree feat/clean?");
}

#[test]
fn a_worktree_on_a_machine_names_the_machine() {
    assert_eq!(loading_title(&row(Whose::There(vps_aim()))), "Remove the worktree feat/clean on vps?");
}

#[test]
fn the_check_says_what_it_checks() {
    assert_eq!(
        check_job(&row(Whose::There(vps_aim()))),
        Busy::new(format!("check-removal:{FOLDER}"), "Checking what removing feat/clean on vps would lose…")
    );
}

#[test]
fn a_removal_here_waits_under_its_workspace() {
    let folder = clean(None);
    let workspace = folder.workspace().expect("the folder's open workspace");
    assert_eq!(Whose::Here.job(&folder).key(), Busy::remove_worktree(workspace, "x").key());
}

#[test]
fn a_removal_on_a_machine_waits_under_its_leaving_key() {
    let folder = clean(Some("vps"));
    let busy = Whose::There(vps_aim()).job(&folder);
    assert_eq!(busy, Busy::leaving(&vps_aim(), "Removing shop › feat/clean on vps…"));
    assert_ne!(busy.key(), Busy::remove_worktree(9, "x").key());
}

#[test]
fn a_machines_worktree_is_removed_by_its_folder_and_no_id() {
    let command = clean(Some("vps")).remove_command(true);
    assert!(command.windows(2).any(|pair| pair == ["--path", FOLDER]));
    assert!(command.contains(&"--delete-branch".to_string()));
    assert!(!command.contains(&"--workspace".to_string()));
    assert!(!command.contains(&"--boot".to_string()));
}

fn answer() -> anyhow::Result<String> {
    let mut answer: serde_json::Value = serde_json::from_str(fixture!("worktrees/remove-check.json"))?;
    if let Some(folders) = answer["folders"].as_array_mut() {
        folders.retain(|folder| folder["path"] == FOLDER);
    }
    Ok(answer.to_string())
}

#[test]
fn a_check_on_a_machine_gives_a_folder_that_names_it() {
    assert_eq!(checked(&row(Whose::There(vps_aim())), answer()).title(), "shop › feat/clean on vps");
}

#[test]
fn a_check_here_gives_a_folder_that_names_no_machine() {
    assert_eq!(checked(&row(Whose::Here), answer()).title(), "shop › feat/clean");
}
