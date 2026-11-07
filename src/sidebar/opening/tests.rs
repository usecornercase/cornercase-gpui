use super::{Imported, import_args, imported_name, is_code_workspace, not_a_code_workspace, open_args};
use crate::store::ToastKind;
use crate::test_util::fixture;

fn imported(text: &str) -> Imported {
    serde_json::from_str(text).expect("an answer of open")
}

fn one_failed() -> Imported {
    imported(r#"{"failed":1,"group":10,"missing":0,"name":"team","projects":[14]}"#)
}

#[test]
fn a_folder_opens_and_shows() {
    assert_eq!(open_args("/code/shop", None), ["open", "/code/shop", "--focus"]);
}

#[test]
fn a_folder_for_a_group_goes_into_it_in_one_command() {
    assert_eq!(open_args("/code/shop", Some(3)), ["open", "/code/shop", "--into", "3", "--focus"]);
}

#[test]
fn an_import_asks_for_what_it_will_summarise() {
    assert_eq!(
        import_args("/code/team.code-workspace", None),
        ["open", "/code/team.code-workspace", "--focus", "--json"]
    );
}

#[test]
fn an_import_from_a_group_goes_into_that_group() {
    assert_eq!(
        import_args("~/team.code-workspace", Some(3)),
        ["open", "~/team.code-workspace", "--into", "3", "--focus", "--json"]
    );
}

#[test]
fn a_whole_import_says_how_many_projects_went_into_the_group() {
    let whole = imported(fixture!("code_workspace/whole.json"));
    assert_eq!(whole.message(), "2 projects imported into whole");
    assert!(whole.kind() == ToastKind::Done);
}

#[test]
fn a_folder_that_is_gone_is_said_beside_what_went_in() {
    let partial = imported(fixture!("code_workspace/one-gone.json"));
    assert_eq!(partial.message(), "2 projects imported into team, 1 not found");
    assert!(partial.kind() == ToastKind::Done);
}

#[test]
fn a_folder_that_could_not_open_is_an_error_and_says_where_to_look() {
    assert_eq!(one_failed().message(), "1 project imported into team, 1 could not open, see server.log");
    assert!(one_failed().kind() == ToastKind::Error);
}

#[test]
fn an_import_of_nothing_is_an_error_that_names_the_file() {
    let nothing = imported(fixture!("code_workspace/nothing.json"));
    assert_eq!(nothing.message(), "nothing imported from none, 1 not found");
    assert!(nothing.kind() == ToastKind::Error);
}

#[test]
fn an_import_unfolds_the_group_its_projects_went_into() {
    assert_eq!(imported(fixture!("code_workspace/one-gone.json")).group_to_unfold(), Some(10));
}

#[test]
fn an_import_of_nothing_unfolds_no_group() {
    let into_a_group = imported(r#"{"failed":0,"group":9,"missing":1,"name":"Existing","projects":[]}"#);
    assert_eq!(into_a_group.group_to_unfold(), None);
    assert_eq!(imported(fixture!("code_workspace/nothing.json")).group_to_unfold(), None);
}

#[test]
fn a_path_ending_in_the_extension_is_a_code_workspace_on_any_system() {
    for path in ["/code/team.code-workspace", "~/team.code-workspace", "C:\\code\\team.code-workspace"] {
        assert!(is_code_workspace(path), "{path}");
    }
}

#[test]
fn a_folder_or_another_file_is_no_code_workspace() {
    for path in ["/code/shop", "/code/team.txt", "~/.code-workspace/shop"] {
        assert!(!is_code_workspace(path), "{path}");
    }
}

#[test]
fn another_file_is_refused_by_its_name() {
    assert_eq!(
        not_a_code_workspace("/code/notes.txt"),
        "notes.txt is not a VS Code workspace: choose a .code-workspace file"
    );
}

#[test]
fn an_import_is_named_after_the_file_without_its_extension() {
    assert_eq!(imported_name("/code/team.code-workspace"), "team");
}

#[test]
fn a_file_with_nothing_before_the_extension_keeps_its_whole_name() {
    assert_eq!(imported_name("/code/.code-workspace"), ".code-workspace");
}
