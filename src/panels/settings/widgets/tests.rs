use super::*;

#[test]
fn capitalises_the_first_letter() {
    assert_eq!(capitalised("worktrees folder"), "Worktrees folder");
    assert_eq!(capitalised("TODO dot"), "TODO dot");
    assert_eq!(capitalised(""), "");
}
