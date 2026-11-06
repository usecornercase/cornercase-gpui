use super::super::Mode;
use super::*;
use crate::store::Reach;
use crate::test_util::{vps_aim as aim, vps_read as read, vps_target as on_machine};

fn on_vps() -> Whose {
    Whose::There(aim(), "ana@vps.example.com".into())
}

fn there(mode: Mode) -> Follow {
    Follow { scope: Scope::Project(mode), workspace: Some(on_vps()) }
}

fn every() -> Follow {
    Follow { scope: Scope::Every, workspace: None }
}

#[test]
fn a_machines_workspace_is_listed_with_its_own_id_and_the_panels_mode() {
    assert_eq!(
        list_args(&there(Mode::Commits)),
        ["changes", "list", "--follow", "--workspace", "7", "--mode", "commits"]
    );
}

#[test]
fn a_workspace_here_is_listed_as_it_always_was() {
    let follow = Follow { scope: Scope::Project(Mode::All), workspace: Some(Whose::Here(43)) };
    assert_eq!(list_args(&follow), ["changes", "list", "--follow", "--workspace", "43", "--mode", "all"]);
}

#[test]
fn a_machines_workspace_is_followed_on_that_machine() {
    let read = read(Reach::Up(on_machine()));
    let target = stream_target(Some(&there(Mode::Uncommitted)), Some(&read), &Target::Local);
    assert_eq!(target, Some(on_machine()));
}

#[test]
fn a_machine_down_is_not_followed_at_all() {
    let read = read(Reach::Down("vps unreachable".into()));
    assert_eq!(stream_target(Some(&there(Mode::Uncommitted)), Some(&read), &Target::Local), None);
}

#[test]
fn a_workspace_that_is_a_machines_is_never_followed_on_the_window_server() {
    assert_eq!(stream_target(Some(&there(Mode::Uncommitted)), None, &Target::Local), None);
}

#[test]
fn a_workspace_here_is_followed_on_the_window_server() {
    let follow = Follow { scope: Scope::Project(Mode::All), workspace: Some(Whose::Here(43)) };
    assert_eq!(stream_target(Some(&follow), None, &Target::Local), Some(Target::Local));
}

#[test]
fn every_project_stays_on_the_window_server_with_a_machine_picked() {
    let read = read(Reach::Up(on_machine()));
    assert_eq!(stream_target(Some(&every()), Some(&read), &Target::Local), Some(Target::Local));
}

#[test]
fn a_closed_panel_follows_nothing() {
    assert_eq!(stream_target(None, None, &Target::Local), None);
}

#[test]
fn a_picked_machine_takes_the_place_of_the_workspace_shown_here() {
    let read = read(Reach::Up(on_machine()));
    assert_eq!(shown_whose(None, Some(&read)), Some(on_vps()));
}

#[test]
fn a_machine_down_stays_the_workspace_shown() {
    let read = read(Reach::Down("vps unreachable".into()));
    assert_eq!(shown_whose(None, Some(&read)), Some(on_vps()));
}

#[test]
fn a_summary_of_the_boot_the_place_was_picked_on_is_believed() {
    let summary = Summary { boot: aim().boot, ..Summary::default() };
    assert!(!from_another_server(&summary, Some(&there(Mode::Uncommitted))));
}

#[test]
fn a_summary_from_a_server_that_restarted_is_not_believed() {
    let summary = Summary { boot: "99-1".into(), ..Summary::default() };
    assert!(from_another_server(&summary, Some(&there(Mode::Uncommitted))));
}

#[test]
fn a_summary_for_a_workspace_here_has_no_boot_to_compare() {
    let follow = Follow { scope: Scope::Project(Mode::All), workspace: Some(Whose::Here(43)) };
    let summary = Summary { boot: "b2".into(), ..Summary::default() };
    assert!(!from_another_server(&summary, Some(&follow)));
}

#[test]
fn a_machines_workspace_is_no_workspace_here() {
    assert_eq!(on_vps().here(), None);
    assert_eq!(Whose::Here(43).here(), Some(43));
}

#[test]
fn the_files_a_machine_lists_are_its_own_whatever_their_numbers_mean_here() {
    assert!(on_vps().holds(43));
    assert!(!Whose::Here(7).holds(43));
}

#[test]
fn a_row_of_a_machines_list_stays_the_machines() {
    let (listed, here) = (43, Whose::Here(43));
    assert_eq!(whose_listed(Some(&there(Mode::All)), listed), on_vps());
    assert_eq!(whose_listed(Some(&every()), listed), here);
}

#[test]
fn the_first_line_of_a_reopened_stream_is_only_starting() {
    let line = r#"{"boot":"b1","mode":"uncommitted","revision":1,"state":"loading","workspaces":[]}"#;
    assert!(only_starting(&wire::summary(line).expect("parses")));
}

#[test]
fn a_ready_line_with_no_files_is_news() {
    let line = r#"{"boot":"b1","mode":"uncommitted","revision":2,"state":"ready","workspaces":[]}"#;
    assert!(!only_starting(&wire::summary(line).expect("parses")));
}

#[test]
fn another_destination_is_another_place_to_read() {
    let read = MachineRead { destination: "ana@other.example.com".into(), ..read(Reach::Up(on_machine())) };
    assert_ne!(shown_whose(None, Some(&read)), Some(on_vps()));
}

fn up() -> MachineRead {
    read(Reach::Up(on_machine()))
}

fn not_yet() -> Result<Target, String> {
    Err("vps has not answered yet".to_string())
}

fn hearing_the_machine() -> Hearing {
    let mut hearing = Hearing::default();
    hearing.start_over();
    hearing.opened(Some(on_machine()));
    hearing.believed();
    hearing
}

#[test]
fn a_stream_that_said_its_boot_lets_the_machine_be_read() {
    assert_eq!(hearing_the_machine().may_read(&up()), Ok(on_machine()));
}

#[test]
fn a_new_stream_has_said_nothing_yet() {
    let mut hearing = Hearing::default();
    hearing.start_over();
    hearing.opened(Some(on_machine()));
    assert_eq!(hearing.may_read(&up()), not_yet());
}

#[test]
fn a_machine_down_and_back_is_not_read_before_it_says_its_boot() {
    let mut hearing = hearing_the_machine();
    hearing.start_over();
    hearing.opened(None);
    assert_eq!(hearing.may_read(&read(Reach::Down("vps unreachable".into()))), Err("vps unreachable".to_string()));
    hearing.start_over();
    hearing.opened(Some(on_machine()));
    assert_eq!(hearing.may_read(&up()), not_yet());
    hearing.believed();
    assert_eq!(hearing.may_read(&up()), Ok(on_machine()));
}

#[test]
fn closing_and_opening_the_panel_is_not_read_before_it_says_its_boot() {
    let mut hearing = hearing_the_machine();
    hearing.start_over();
    hearing.opened(None);
    hearing.start_over();
    hearing.opened(Some(on_machine()));
    assert_eq!(hearing.may_read(&up()), not_yet());
    hearing.believed();
    assert_eq!(hearing.may_read(&up()), Ok(on_machine()));
}

#[test]
fn a_restart_seen_before_closing_the_panel_still_holds_after_opening_it() {
    let mut hearing = hearing_the_machine();
    hearing.ended("vps restarted".into(), Instant::now());
    hearing.start_over();
    hearing.opened(None);
    hearing.start_over();
    hearing.opened(Some(on_machine()));
    assert!(hearing.may_read(&up()).is_err());
}

#[test]
fn a_stream_that_ended_is_not_read_until_another_says_its_boot() {
    let mut hearing = hearing_the_machine();
    hearing.ended("ssh: connection refused".into(), Instant::now());
    assert_eq!(hearing.may_read(&up()), Err("vps did not answer".to_string()));
    hearing.opened(Some(on_machine()));
    assert_eq!(hearing.may_read(&up()), Err("vps did not answer".to_string()));
    hearing.believed();
    assert_eq!(hearing.may_read(&up()), Ok(on_machine()));
}
