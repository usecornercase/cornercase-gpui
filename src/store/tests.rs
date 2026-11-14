use super::editor_command;

#[test]
fn quotes_the_path_for_the_editor() {
    assert_eq!(editor_command("src/a b.rs", 7), "${VISUAL:-${EDITOR:-vi}} +7 'src/a b.rs'");
    assert_eq!(editor_command("it's.txt", 1), r"${VISUAL:-${EDITOR:-vi}} +1 'it'\''s.txt'");
}
