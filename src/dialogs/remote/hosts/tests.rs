use std::path::{Path, PathBuf};

use super::{Parsed, glob, included, parse};

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(ToString::to_string).collect()
}

#[test]
fn hosts_are_the_names_without_patterns() {
    let text = "Host devbox gpu-box\n  HostName 10.0.0.5\nHost *.internal !bastion web?\nhost=build\n# Host commented\nHost \"quoted\"\n";
    assert_eq!(parse(text).hosts, words(&["devbox", "gpu-box", "build", "quoted"]));
}

#[test]
fn git_forges_are_not_servers() {
    assert_eq!(parse("Host github.com GitLab.com devbox\nHost bitbucket.org\n").hosts, words(&["devbox"]));
}

#[test]
fn includes_are_listed_in_order() {
    let text = "Include ~/.orbstack/ssh/config\nHost a\n  include config.d/* other\n";
    assert_eq!(
        parse(text),
        Parsed { hosts: words(&["a"]), includes: words(&["~/.orbstack/ssh/config", "config.d/*", "other"]) }
    );
}

#[test]
fn an_included_path_is_read_from_home_or_from_the_ssh_folder() {
    let home = Path::new("/Users/me");
    assert_eq!(included("~/.colima/ssh_config", home), [PathBuf::from("/Users/me/.colima/ssh_config")]);
    assert_eq!(included("work", home), [PathBuf::from("/Users/me/.ssh/work")]);
    assert_eq!(included("/etc/ssh/extra", home), [PathBuf::from("/etc/ssh/extra")]);
}

#[test]
fn a_glob_takes_stars_and_question_marks() {
    let chars = |text: &str| text.chars().collect::<Vec<_>>();
    assert!(glob(&chars("*.conf"), &chars("work.conf")));
    assert!(glob(&chars("host-?"), &chars("host-1")));
    assert!(glob(&chars("*"), &chars("")));
    assert!(!glob(&chars("*.conf"), &chars("work.conf.bak")));
    assert!(!glob(&chars("host-?"), &chars("host-12")));
}
