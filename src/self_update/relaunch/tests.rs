use super::*;

fn flags(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_string()).collect()
}

#[test]
fn the_window_reopens_on_the_tab_it_showed() {
    assert_eq!(relaunch_args(&[], Some(6)), ["--tab", "6"]);
    assert_eq!(relaunch_args(&[], None), Vec::<String>::new());
}

#[test]
fn a_window_on_another_server_reopens_there() {
    let remote = flags(&["--remote", "devbox", "--remote-command", "cornercase-dev"]);
    assert_eq!(
        relaunch_args(&remote, Some(6)),
        ["--tab", "6", "--remote", "devbox", "--remote-command", "cornercase-dev"]
    );
}
