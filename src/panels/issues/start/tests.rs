use super::{Last, last_field};

#[test]
fn the_workspace_name_ends_the_form_in_a_new_worktree() {
    assert_eq!(last_field(false, true, true), Last::Name);
    assert_eq!(last_field(false, true, false), Last::Name);
}

#[test]
fn effort_ends_the_form_in_a_tab_for_an_agent_that_takes_it() {
    assert_eq!(last_field(false, false, true), Last::Effort);
}

#[test]
fn model_ends_the_form_in_a_tab_for_an_agent_without_effort() {
    assert_eq!(last_field(false, false, false), Last::Model);
}

#[test]
fn the_name_ends_the_form_when_scheduling() {
    assert_eq!(last_field(true, false, false), Last::Name);
    assert_eq!(last_field(true, true, true), Last::Name);
}
