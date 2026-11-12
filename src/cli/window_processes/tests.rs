use std::ffi::OsStr;

use super::command;

#[test]
fn the_command_runs_here_whatever_server_the_window_is_on() {
    let command = command();
    let args: Vec<&OsStr> = command.get_args().collect();
    assert_eq!(args, ["plugin", "windows", "--json"]);
    assert!(command.get_envs().any(|(name, value)| name == "CORNERCASE_REMOTE" && value.is_none()));
}
