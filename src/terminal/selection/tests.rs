use super::*;
use crate::cli::failure;

#[test]
fn a_copy_refused_for_its_run_copies_nothing_and_says_nothing() {
    let stale = failure("selection", Some(4), b"error: this request is for another run\n");
    assert!(kept(Board::Clipboard, Err(stale), true).is_none());
}

#[test]
fn a_copy_that_fails_otherwise_says_why() {
    let other = failure("selection", Some(1), b"there is no pane 3");
    assert!(matches!(kept(Board::Clipboard, Err(other), true), Some(Msg::Error(_))));
}
