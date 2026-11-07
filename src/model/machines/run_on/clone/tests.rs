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
    assert_eq!(
        clone_args(" https://host/a.git ", "~/code", &[], "/here/a"),
        ["clone", "https://host/a.git", "~/code", "--json"]
    );
}

#[test]
fn a_machine_without_a_folder_clones_in_its_home() {
    assert_eq!(clone_args("https://host/a.git", " ", &[], "/here/a"), ["clone", "https://host/a.git", "--json"]);
}

#[test]
fn the_ticked_files_go_with_the_folder_they_are_read_from() {
    let files = [".env".to_string(), "web/.env.local".to_string()];
    assert_eq!(
        clone_args("https://host/a.git", "~/code", &files, "/here/a"),
        ["clone", "https://host/a.git", "~/code", "--file=.env", "--file=web/.env.local", "--from=/here/a", "--json"]
    );
}

#[test]
fn a_file_whose_name_starts_with_a_dash_stays_a_file() {
    let args = clone_args("https://host/a.git", "~/code", &["-weird".to_string()], "/here/a");
    assert!(args.contains(&"--file=-weird".to_string()));
}

#[test]
fn the_list_is_asked_of_the_projects_folder() {
    assert_eq!(ignored_args("/here/a"), ["clone", "--list-ignored", "--from=/here/a", "--json"]);
}

#[test]
fn a_clones_answer_names_the_project_there() {
    assert_eq!(parse_clone(fixture!("machines/there_cloned.json")), Ok((ThereId::heard(30), Vec::new())));
}

#[test]
fn a_clones_answer_names_what_it_could_not_write() {
    let (there, unwritten) = parse_clone(fixture!("machines/there_cloned_unwritten.json")).expect("a clone");
    assert_eq!(there, ThereId::heard(14));
    assert_eq!(unwritten, [unwritten_file("README.md", "it is already in the clone")]);
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
    let cloned =
        Cloned { machine: "vps".into(), boot: "1".into(), project: 8, there: ThereId::heard(3), at: Instant::now() };
    assert!(cloned.of(&seen, 8));
    assert!(!cloned.of(&seen, 9));
    assert!(!Cloned { boot: "2".into(), ..cloned.clone() }.of(&seen, 8));
    assert!(!Cloned { machine: "other".into(), ..cloned }.of(&seen, 8));
}

fn unwritten_file(path: &str, why: &str) -> Unwritten {
    Unwritten { path: path.into(), why: why.into() }
}

#[test]
fn a_clone_with_every_file_only_says_it_cloned() {
    assert_eq!(cloned_note("shop", "vps", &[]), "Cloned shop on vps");
}

#[test]
fn a_clone_names_what_it_did_not_copy_and_why() {
    let left = [unwritten_file(".env", "it is already in the clone")];
    assert_eq!(cloned_note("shop", "vps", &left), "Cloned shop on vps · not copied: .env (it is already in the clone)");
}

#[test]
fn past_three_files_the_note_counts_the_rest() {
    let left = ["a", "b", "c", "d", "e"].map(|path| unwritten_file(path, "no"));
    assert_eq!(
        cloned_note("shop", "vps", &left),
        "Cloned shop on vps · not copied: a (no), b (no), c (no) … and 2 more"
    );
}

#[test]
fn a_file_with_no_name_is_left_out_of_the_note() {
    assert_eq!(cloned_note("shop", "vps", &[unwritten_file(" \n", "no")]), "Cloned shop on vps");
}

#[test]
fn a_file_with_no_reason_says_the_machine_gave_none() {
    let left = [unwritten_file(".env", " ")];
    assert_eq!(cloned_note("shop", "vps", &left), "Cloned shop on vps · not copied: .env (vps gave no reason)");
}

fn listed() -> FileList {
    FileList::heard(Ok(fixture!("machines/clone_ignored.json").to_string()))
}

#[test]
fn every_box_starts_unticked() {
    let FileList::Read { files, left_out } = listed() else { panic!("a list") };
    assert_eq!(files.len(), 3);
    assert!(files.iter().all(|(_, ticked)| !ticked));
    assert_eq!(left_out, 0);
    assert_eq!(listed().ticked(), Vec::<String>::new());
}

#[test]
fn a_tick_comes_and_goes() {
    let mut list = listed();
    assert_eq!(list.toggle(0), Ok(()));
    assert_eq!(list.toggle(2), Ok(()));
    assert_eq!(list.ticked(), [".env", "web/.env.local"]);
    assert_eq!(list.toggle(0), Ok(()));
    assert_eq!(list.ticked(), ["web/.env.local"]);
}

#[test]
fn a_file_too_big_cannot_be_ticked() {
    let mut list = listed();
    assert_eq!(list.toggle(1), Ok(()));
    assert_eq!(list.ticked(), Vec::<String>::new());
}

#[test]
fn a_file_too_big_says_so_beside_its_size() {
    let FileList::Read { files, .. } = listed() else { panic!("a list") };
    assert_eq!(files[0].0.size_words(), "9 B");
    assert_eq!(files[1].0.size_words(), "2.0 MB · too big to copy");
}

fn mib_files(count: usize) -> FileList {
    let file = |at: usize| (IgnoredFile { path: format!("{at}"), size: 1024 * 1024, too_big: false }, false);
    FileList::Read { files: (0..count).map(file).collect(), left_out: 0 }
}

#[test]
fn a_tick_past_eight_mib_in_all_is_refused() {
    let mut list = mib_files(9);
    for at in 0..8 {
        assert_eq!(list.toggle(at), Ok(()));
    }
    assert_eq!(list.toggle(8), Err(OVER_THE_TOTAL.to_string()));
    assert_eq!(list.ticked().len(), 8);
}

#[test]
fn unticking_is_never_refused_at_the_limit() {
    let mut list = mib_files(9);
    for at in 0..8 {
        assert_eq!(list.toggle(at), Ok(()));
    }
    assert_eq!(list.toggle(7), Ok(()));
    assert_eq!(list.toggle(8), Ok(()));
    assert_eq!(list.ticked().len(), 8);
}

#[test]
fn what_is_past_the_200th_is_counted() {
    let list = FileList::heard(Ok(r#"{"root": "/x", "files": [], "left_out": 12}"#.to_string()));
    assert_eq!(list, FileList::Read { files: Vec::new(), left_out: 12 });
    assert_eq!(not_offered(12), "… and 12 more not offered (200 at most)");
}

#[test]
fn a_list_that_failed_keeps_the_commands_words_and_offers_nothing() {
    let why = "copying git-ignored files into a clone is not supported on Windows yet";
    let list = FileList::heard(Err(why.to_string()));
    assert_eq!(list, FileList::Failed(why.to_string()));
    assert!(!list.offers_any());
    assert_eq!(list.ticked(), Vec::<String>::new());
}

#[test]
fn an_answer_that_cannot_be_read_is_a_failed_list() {
    assert_eq!(FileList::heard(Ok("no json".to_string())), FileList::Failed(UNREADABLE.to_string()));
}

#[test]
fn only_a_file_that_can_be_ticked_counts_as_offered() {
    assert!(listed().offers_any());
    assert!(!FileList::Reading.offers_any());
    let big = IgnoredFile { path: "big".into(), size: 1 << 21, too_big: true };
    assert!(!FileList::Read { files: vec![(big, false)], left_out: 0 }.offers_any());
}

#[test]
fn the_intro_says_what_the_clone_copies() {
    assert_eq!(
        intro("vps", Os::Linux),
        "vps clones it from the address, then copies the git-ignored files you tick (such as .env) into the clone. Ignored folders, such as node_modules, are never offered."
    );
    assert_eq!(intro("vps", Os::Mac), intro("vps", Os::Linux));
}

#[test]
fn on_windows_the_intro_says_the_files_stay_behind() {
    assert_eq!(
        intro("vps", Os::Windows),
        "vps clones it from the address, without git-ignored files (such as .env): this computer runs Windows, where reading them to copy is not supported yet."
    );
}
