use super::*;
use crate::cli::Remote;

#[test]
fn every_project_names_the_server_a_window_on_another_machine_is_on() {
    let window = Target::Remote(Remote::new("devbox", "").expect("a destination"));
    assert_eq!(every_project_on(&window), "uncommitted changes of every project on devbox");
}

#[test]
fn every_project_of_a_window_here_says_this_computer() {
    assert_eq!(
        every_project_on(&Target::Local),
        format!("uncommitted changes of every project on this {}", desktop::here())
    );
}
