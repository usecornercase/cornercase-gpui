use std::path::Path;

use super::*;
use crate::platform;

#[test]
fn this_process_is_alive() {
    assert!(alive(std::process::id()));
}

#[test]
fn a_process_that_ended_is_not_alive() {
    let mut child = platform::command("cmd").args(["/c", "exit"]).spawn().expect("cmd starts");
    let pid = child.id();
    child.wait().expect("cmd ends");
    assert!(!alive(pid));
}

#[test]
fn the_singleton_window_is_named_after_the_whole_profile_path_without_a_trailing_separator() {
    let profile = Path::new("C:/Users/me/AppData/Local/cornercase-gpui/browser\\");
    assert_eq!(singleton_name(profile), Path::new("C:\\Users\\me\\AppData\\Local\\cornercase-gpui\\browser"));
}

#[test]
fn a_profile_no_browser_runs_has_no_singleton_window() {
    let profile = std::env::temp_dir().join(format!("cornercase-gpui-no-browser-{}", std::process::id()));
    assert_eq!(singleton_pid(&profile), None);
}
