use std::time::Duration;

use super::{Remote, backoff, from_home};

fn remote(program: &str) -> Remote {
    Remote::new("devbox", program).expect("a destination")
}

#[test]
fn a_remote_passes_its_destination_to_cornercase() {
    assert_eq!(remote("").flags(), ["--remote", "devbox"]);
}

#[test]
fn a_program_of_its_own_goes_as_the_remote_command_unchanged() {
    let remote = Remote::new("me@devbox", " env HOME=~/x cornercase ").expect("a destination");
    assert_eq!(remote.flags(), ["--remote", "me@devbox", "--remote-command", "env HOME=~/x cornercase"]);
}

#[test]
fn a_destination_is_one_word_that_is_not_an_option() {
    assert_eq!(Remote::new(" me@devbox ", "").map(|r| r.destination().to_string()).as_deref(), Some("me@devbox"));
    assert!(Remote::new("", "").is_none());
    assert!(Remote::new("-oProxyCommand=evil", "").is_none());
    assert!(Remote::new("dev box", "").is_none());
    assert_eq!(remote("  ").program(), None);
}

fn from_args(list: &[&str]) -> anyhow::Result<Option<Remote>> {
    Remote::from_args(&list.iter().map(ToString::to_string).collect::<Vec<_>>())
}

#[test]
fn the_command_line_names_the_remote_and_how_to_run_cornercase_there() {
    let found = from_args(&["--tab", "6", "--remote", "devbox", "--remote-command", "~/bin/cc"]).expect("valid");
    assert_eq!(found, Remote::new("devbox", "~/bin/cc"));
    assert_eq!(from_args(&["--tab", "6"]).expect("valid"), None);
}

#[test]
fn a_remote_without_a_usable_destination_is_refused_rather_than_ignored() {
    assert!(from_args(&["--remote"]).is_err());
    assert!(from_args(&["--remote", "-oProxyCommand=evil"]).is_err());
}

#[test]
fn reconnecting_waits_longer_each_time_up_to_fifteen_seconds() {
    let waits: Vec<u64> = [0, 1, 2, 3, 4, 9, 70].into_iter().map(|n| backoff(n).as_secs()).collect();
    assert_eq!(waits, [1, 2, 4, 8, 15, 15, 15]);
    assert_eq!(backoff(0), Duration::from_secs(1));
}

#[test]
fn a_path_typed_from_home_becomes_relative_to_it() {
    assert_eq!(from_home("~/src/shop"), "src/shop");
    assert_eq!(from_home(" ~ "), ".");
    assert_eq!(from_home("~/"), ".");
    assert_eq!(from_home("/srv/app"), "/srv/app");
    assert_eq!(from_home("src"), "src");
}
