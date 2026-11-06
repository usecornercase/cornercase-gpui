use super::*;
use crate::test_util::fixture;

fn remote(name: &str, url: &str) -> GitRemote {
    GitRemote { name: name.into(), url: url.into() }
}

#[test]
fn the_address_is_upstreams_first() {
    let remotes = [remote("origin", "git@github.com:me/shop.git"), remote("upstream", "https://github.com/acme/shop")];
    assert_eq!(clone_address(&remotes), Some("https://github.com/acme/shop"));
}

#[test]
fn the_address_is_origins_without_an_upstream() {
    let remotes = [remote("fork", "git@github.com:me/shop.git"), remote("origin", "https://github.com/acme/shop.git")];
    assert_eq!(clone_address(&remotes), Some("https://github.com/acme/shop.git"));
}

#[test]
fn the_address_is_the_first_remote_otherwise() {
    let remotes = [remote("fork", "git@github.com:me/shop.git"), remote("other", "https://github.com/acme/shop.git")];
    assert_eq!(clone_address(&remotes), Some("git@github.com:me/shop.git"));
}

#[test]
fn a_path_on_this_computer_is_no_address_for_another() {
    let remotes = [remote("origin", "/srv/git/shop.git"), remote("fork", "gh-work:acme/shop")];
    assert_eq!(clone_address(&remotes), Some("gh-work:acme/shop"));
    assert_eq!(clone_address(&[remote("origin", "/srv/git/shop.git")]), None);
    assert_eq!(clone_address(&[]), None);
}

#[test]
fn the_clones_name_comes_from_the_last_part_of_the_address() {
    for (url, name) in [
        ("https://github.com/acme/shop.git", "shop"),
        ("https://github.com/acme/shop/", "shop"),
        ("git@github.com:acme/shop.git", "shop"),
        ("gh-work:shop", "shop"),
        ("ssh://git@host/acme/shop.bundle", "shop"),
        ("/srv/git/shop/.git", "shop"),
        ("https://user:token@host/acme/shop.git?token=abc", "shop"),
    ] {
        assert_eq!(folder_name(url).as_deref(), Some(name), "{url}");
    }
}

#[test]
fn an_address_with_no_usable_name_has_none() {
    for url in ["", "/", "https://host/.git.git", "."] {
        assert_eq!(folder_name(url), None, "{url}");
    }
}

#[test]
fn the_folder_there_is_the_machines_folder_and_the_clones_name() {
    assert_eq!(destination("~/code", "https://github.com/acme/shop.git"), "~/code/shop");
    assert_eq!(destination("~/code/", "https://github.com/acme/shop.git"), "~/code/shop");
    assert_eq!(destination("", "https://github.com/acme/shop.git"), "~/shop");
    assert_eq!(destination("~/code", ""), "~/code/…");
}

#[test]
fn the_command_gives_the_address_and_the_machines_folder() {
    assert_eq!(clone_args(" https://host/a.git ", "~/code"), ["clone", "https://host/a.git", "~/code", "--json"]);
}

#[test]
fn a_machine_without_a_folder_clones_in_its_home() {
    assert_eq!(clone_args("https://host/a.git", " "), ["clone", "https://host/a.git", "--json"]);
}

#[test]
fn a_clones_answer_names_the_project_there() {
    assert_eq!(parse_clone(fixture!("machines/there_cloned.json")), Ok(ThereId::heard(30)));
}

#[test]
fn an_answer_with_no_path_or_project_is_a_failure() {
    let unread = Err(UNREADABLE.to_string());
    assert_eq!(parse_clone("{}"), unread);
    assert_eq!(parse_clone(r#"{"project": 3}"#), unread);
    assert_eq!(parse_clone(r#"{"path": "/x"}"#), unread);
    assert_eq!(parse_clone("no json"), unread);
}

#[test]
fn a_clone_counts_for_its_machine_its_run_and_its_project() {
    let seen = MachineSeen { name: "vps".into(), boot: "1".into(), ..MachineSeen::default() };
    let cloned = Cloned { machine: "vps".into(), boot: "1".into(), project: 8, there: ThereId::heard(3) };
    assert!(cloned.of(&seen, 8));
    assert!(!cloned.of(&seen, 9));
    assert!(!Cloned { boot: "2".into(), ..cloned.clone() }.of(&seen, 8));
    assert!(!Cloned { machine: "other".into(), ..cloned }.of(&seen, 8));
}
