use super::PathKind;

#[test]
fn asking_for_a_folder_names_the_machine_and_the_folder_picker() {
    assert_eq!(PathKind::Folder.label("devbox"), "Folder on devbox");
    assert_eq!(
        PathKind::Folder.hint("Mac"),
        "From its home folder, or its full path there. The folder picker only sees this Mac's disk."
    );
}

#[test]
fn asking_for_a_vs_code_workspace_asks_for_a_file() {
    assert_eq!(PathKind::CodeWorkspace.label("devbox"), "File on devbox");
    assert_eq!(PathKind::CodeWorkspace.placeholder(), "~/src/team.code-workspace");
    assert_eq!(
        PathKind::CodeWorkspace.hint("PC"),
        "From its home folder, or its full path there. The file picker only sees this PC's disk."
    );
}
