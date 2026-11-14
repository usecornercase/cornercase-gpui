use std::cell::RefCell;
use std::fs;

use anyhow::anyhow;

use std::sync::atomic::Ordering;

use super::super::tree;
use super::super::{Place, Search};
use super::*;
use crate::store::{OlderNote, Reach};
use crate::test_util::{fixture, reads_with_boot, vps_aim as aim, vps_read, vps_target as target};

fn there() -> There {
    There::new(target(), &aim(), ReadReleases::default())
}

struct Machine {
    answer: String,
    asked: RefCell<Vec<(Target, Vec<String>)>>,
}

impl Machine {
    fn answering(answer: &str) -> Self {
        Self { answer: answer.to_string(), asked: RefCell::default() }
    }

    fn ask(&self, target: &Target, args: &[String]) -> String {
        self.asked.borrow_mut().push((target.clone(), args.to_vec()));
        self.answer.clone()
    }

    fn only_request(&self) -> (Target, Vec<String>) {
        let asked = self.asked.borrow();
        assert_eq!(asked.len(), 1, "one command per question");
        asked[0].clone()
    }

    fn asked_which_run_first(&self) -> (Target, Vec<String>) {
        let asked = self.asked.borrow();
        assert_eq!(asked.len(), 2, "which run, then the question");
        assert_eq!(asked[0].1[..4], ["files", "list", "--workspace", "7"]);
        assert_eq!(asked[0].1.last().map(String::as_str), Some("."));
        asked[1].clone()
    }
}

fn down(_: &Target, _: &[String]) -> Result<String> {
    Err(anyhow!("could not reach cornercase on `vps-host`"))
}

fn folder_here(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("cc-gpui-there-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&folder);
    fs::create_dir_all(folder.join("only-here")).expect("the temporary files are writable");
    fs::write(folder.join("README.md"), "the copy on this computer\n").expect("the temporary files are writable");
    folder
}

fn text_of(content: &Content) -> Vec<String> {
    match content {
        Content::Text(text) => text.lines.as_ref().clone(),
        other => panic!("not text: {other:?}"),
    }
}

#[test]
fn a_file_of_a_machine_is_asked_of_the_machine_though_the_same_path_exists_here() {
    let rel = folder_here("read").join("README.md");
    let json = r#"{"kind":"text","bytes":14,"modified":null,"image":null,"text":"on the machine\n"}"#;
    let machine = Machine::answering(json);
    let (content, _) = there().read_with(&|t, a| Ok(machine.ask(t, a)), &rel);
    assert_eq!(text_of(&content), ["on the machine"]);
    let (asked_on, args) = machine.only_request();
    assert_eq!(asked_on, target());
    assert_eq!(args[..4], ["files", "read", "--workspace", "7"]);
    assert_eq!(args.last().map(String::as_str), rel.to_str());
}

#[test]
fn a_machine_that_does_not_answer_is_never_answered_from_this_disk() {
    let (content, stamp) = there().read_with(&down, &folder_here("down").join("README.md"));
    assert!(matches!(content, Content::Lost(_)), "{content:?}");
    assert_eq!(stamp, None);
}

#[test]
fn a_folder_of_a_machine_lists_what_the_machine_says_though_the_same_path_exists_here() {
    let machine = Machine::answering(fixture!("files/list.json"));
    let listed = there().list_with(&|t, a| Ok(machine.ask(t, a)), vec![folder_here("list")]);
    let names: Vec<String> =
        listed[0].1.as_ref().expect("listed").entries.iter().map(|entry| entry.name.clone()).collect();
    assert!(names.contains(&"docs".to_string()), "{names:?}");
    assert!(!names.contains(&"only-here".to_string()), "{names:?}");
    assert_eq!(machine.only_request().0, target());
}

#[test]
fn a_machine_that_stops_answering_loses_no_folder() {
    let listed = there().list_with(&down, vec![PathBuf::new(), PathBuf::from("src")]);
    assert_eq!(lost_in(&listed).as_deref(), Some("could not reach cornercase on `vps-host`"));
    assert!(listed.iter().all(|(_, listing)| listing.as_ref().is_err_and(is_lost)));
}

#[test]
fn a_folder_that_fails_there_is_not_taken_for_a_machine_lost() {
    let machine = Machine::answering(fixture!("files/list.json"));
    let folders = vec![PathBuf::new(), PathBuf::from("src"), PathBuf::from("nope")];
    let listed = there().list_with(&|t, a| Ok(machine.ask(t, a)), folders);
    assert!(listed[2].1.is_err());
    assert_eq!(lost_in(&listed), None);
}

#[test]
fn a_stamp_is_read_from_the_machines_listing() {
    let machine = Machine::answering(fixture!("files/list.json"));
    let stamp = there().stamp_with(&|t, a| Ok(machine.ask(t, a)), Path::new("a.svg"));
    assert!(stamp.is_some());
    let args = machine.only_request().1;
    assert_eq!(args[..4], ["files", "list", "--workspace", "7"]);
    assert_eq!(args.last().map(String::as_str), Some("."));
}

#[test]
fn the_margin_of_a_machines_file_is_asked_of_the_machine() {
    let machine = Machine::answering(fixture!("changes/file.json"));
    let gutter = there().gutter_with(&|t, a| Ok(machine.ask(t, a)), Path::new("a.txt"));
    assert!(gutter.is_some());
    let (asked_on, args) = machine.asked_which_run_first();
    assert_eq!(asked_on, target());
    assert_eq!(args, ["changes", "file", "--workspace", "7", "--json", "--", "a.txt"]);
}

#[test]
fn a_machine_that_does_not_answer_gives_no_margin() {
    assert!(there().gutter_with(&down, Path::new("a.txt")).is_none());
}

#[test]
fn the_marks_of_a_machines_tree_come_with_its_listing() {
    assert!(Source::There(there()).lists_marks());
}

#[test]
fn a_search_goes_to_the_machine_with_its_mode_and_never_walks_here() {
    let machine = Machine::answering(fixture!("files/search-names.json"));
    let results = there().search_with(&|t, a| Ok(machine.ask(t, a)), "main", Mode::Names);
    assert!(matches!(results, Results::Names(_)));
    let (asked_on, args) = machine.asked_which_run_first();
    assert_eq!(asked_on, target());
    assert_eq!(args[..5], ["files", "search", "--workspace", "7", "--names"]);
}

#[test]
fn a_search_a_machine_does_not_answer_says_so() {
    assert!(matches!(there().search_with(&down, "main", Mode::Text), Results::Failed(_)));
}

#[test]
fn an_answer_from_a_server_that_restarted_is_not_believed() {
    let machine = Machine::answering(r#"{"boot":"99-1","kind":"text","bytes":3,"text":"new"}"#);
    let (content, _) = there().read_with(&|t, a| Ok(machine.ask(t, a)), Path::new("README.md"));
    assert!(matches!(&content, Content::Lost(why) if why == "vps restarted"), "{content:?}");
}

#[test]
fn a_listing_from_a_server_that_restarted_is_not_believed() {
    let machine = Machine::answering(r#"{"boot":"99-1","folders":[{"entries":[],"error":null,"more":false}]}"#);
    let listed = there().list_with(&|t, a| Ok(machine.ask(t, a)), vec![PathBuf::new()]);
    assert_eq!(lost_in(&listed).as_deref(), Some("vps restarted"));
}

#[test]
fn a_machines_place_never_takes_the_key_of_a_workspace_here() {
    let key = place_key(&vps_read(Reach::Up(target())));
    assert!(on_a_machine(key));
    assert_ne!(key, 7);
    assert!(!on_a_machine(7));
    assert_eq!(key, place_key(&vps_read(Reach::Up(target()))));
}

#[test]
fn another_boot_of_the_same_workspace_is_another_place() {
    let read = vps_read(Reach::Up(target()));
    let restarted = MachineRead { aim: Aim { boot: "99-1".into(), ..aim() }, ..read.clone() };
    assert_ne!(place_key(&read), place_key(&restarted));
}

#[test]
fn another_destination_is_another_place_so_a_late_answer_finds_nothing() {
    let read = vps_read(Reach::Up(target()));
    let moved = MachineRead { destination: "ana@other.example.com".into(), ..read.clone() };
    assert_ne!(place_key(&read), place_key(&moved));
}

#[test]
fn a_machine_going_down_keeps_its_place() {
    let down = vps_read(Reach::Down("vps unreachable".into()));
    assert_eq!(place_key(&down), place_key(&vps_read(Reach::Up(target()))));
}

#[test]
fn only_the_disk_source_is_on_this_disk() {
    assert!(Source::Disk { root: PathBuf::from("/home/ana/code/shop"), workspace: 7 }.on_this_disk());
    assert!(!Source::Server(7).on_this_disk());
    assert!(!Source::There(there()).on_this_disk());
}

#[test]
fn a_machines_workspace_is_read_through_the_machine_though_its_folder_exists_here() {
    let here = folder_here("source");
    let read = MachineRead { path: here.to_string_lossy().into_owned(), ..vps_read(Reach::Up(target())) };
    assert_eq!(source_of(&read, ReadReleases::default()), Some(Source::There(there())));
}

#[test]
fn a_machine_down_gives_no_way_to_read_rather_than_this_disk() {
    let here = folder_here("source-down");
    let read =
        MachineRead { path: here.to_string_lossy().into_owned(), ..vps_read(Reach::Down("vps unreachable".into())) };
    assert_eq!(source_of(&read, ReadReleases::default()), None);
}

fn place_with_a_folder_open() -> Place {
    let mut place = Place::new(PathBuf::from("/home/ana/code/shop"), Source::There(there()));
    let folder = PathBuf::from("src");
    place.expanded.insert(folder.clone());
    place.listings.insert(folder, tree::Listing::default());
    place
}

#[test]
fn a_folder_stays_open_while_its_machine_does_not_answer() {
    let mut place = place_with_a_folder_open();
    let lost = Error::new(ErrorKind::NotConnected, "ssh: connection refused");
    assert!(!place.apply_listing(PathBuf::from("src"), Err(lost)));
    assert!(place.expanded.contains(Path::new("src")));
    assert!(place.listings.contains_key(Path::new("src")));
}

#[test]
fn a_root_that_stops_answering_says_nothing_in_place_of_its_tree() {
    let mut place = place_with_a_folder_open();
    let lost = Error::new(ErrorKind::NotConnected, "ssh: connection refused");
    assert!(!place.apply_listing(PathBuf::new(), Err(lost)));
    assert_eq!(place.error, None);
}

#[test]
fn a_folder_the_machine_says_is_gone_closes() {
    let mut place = place_with_a_folder_open();
    assert!(place.apply_listing(PathBuf::from("src"), Err(Error::other("no such folder"))));
    assert!(!place.expanded.contains(Path::new("src")));
}

#[test]
fn on_this_disk_a_lost_listing_closes_its_folder_as_any_failure() {
    let mut place = place_with_a_folder_open();
    place.source = Source::Disk { root: place.root.clone(), workspace: 7 };
    let lost = Error::new(ErrorKind::NotConnected, "transport endpoint is not connected");
    assert!(place.apply_listing(PathBuf::from("src"), Err(lost)));
    assert!(!place.expanded.contains(Path::new("src")));
}

#[test]
fn a_search_left_behind_cannot_answer_or_open_what_it_found() {
    let mut search = Search::new();
    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    search.running = Some(running.clone());
    search.enter = true;
    let asked = search.generation;
    search.forget();
    assert_ne!(search.generation, asked);
    assert!(!search.enter);
    assert!(search.running.is_none());
    assert!(running.load(Ordering::Relaxed));
}

#[test]
fn a_search_left_behind_is_not_asked_again_when_the_machine_comes_back() {
    let mut search = Search::new();
    search.refused = true;
    search.forget();
    assert!(!search.refused);
}

#[test]
fn a_search_is_not_asked_of_a_server_that_restarted_since_its_answer_could_not_say() {
    let machine = Machine::answering(r#"{"boot":"99-1","folders":[]}"#);
    let results = there().search_with(&|t, a| Ok(machine.ask(t, a)), "main", Mode::Names);
    assert!(matches!(&results, Results::Failed(why) if why == "vps restarted"));
    assert_eq!(machine.only_request().1[1], "list");
}

#[test]
fn a_margin_is_not_asked_of_a_server_that_restarted_since_its_answer_could_not_say() {
    let machine = Machine::answering(r#"{"boot":"99-1","folders":[]}"#);
    assert!(there().gutter_with(&|t, a| Ok(machine.ask(t, a)), Path::new("a.txt")).is_none());
    assert_eq!(machine.only_request().1[1], "list");
}

#[test]
fn a_tree_never_read_says_so_only_while_waiting_or_older_and_keeps_what_it_read() {
    let said = |reach, listed| unread_root(&vps_read(reach), listed, &Retry::default());
    assert_eq!(said(Reach::Waiting("w".into()), false).as_deref(), Some("Nothing read from vps"));
    assert_eq!(
        said(Reach::Older(OlderNote { line: "l".into(), tip: "t".into(), ..OlderNote::default() }), false).as_deref(),
        Some("Nothing read from vps")
    );
    assert_eq!(said(Reach::Older(OlderNote { line: "l".into(), tip: "t".into(), ..OlderNote::default() }), true), None);
    assert_eq!(said(Reach::Down("d".into()), false), None);
    assert_eq!(said(Reach::Refused("r".into()), false), None);
    assert_eq!(said(Reach::Up(target()), false), None);
}

fn with_boot() -> There {
    There::new(target(), &aim(), reads_with_boot())
}

fn refused(_: &Target, _: &[String]) -> Result<String> {
    Err(crate::cli::failure("files", Some(4), b"error: this request is for another run"))
}

fn boot_flags(args: &[String]) -> &[String] {
    let at = args.iter().position(|arg| arg == "--").expect("a double dash");
    &args[at - 2..=at]
}

#[test]
fn every_read_on_a_machine_carries_the_aims_boot_before_the_double_dash() {
    let boot = aim().boot;
    let expected = ["--boot", boot.as_str(), "--"];
    let listing = Machine::answering(fixture!("files/list.json"));
    with_boot().list_with(&|t, a| Ok(listing.ask(t, a)), vec![PathBuf::new()]);
    assert_eq!(boot_flags(&listing.only_request().1), expected);
    let read = Machine::answering(r#"{"kind":"text","bytes":3,"modified":null,"image":null,"text":"x"}"#);
    with_boot().read_with(&|t, a| Ok(read.ask(t, a)), Path::new("README.md"));
    assert_eq!(boot_flags(&read.only_request().1), expected);
    let stamp = Machine::answering(fixture!("files/list.json"));
    with_boot().stamp_with(&|t, a| Ok(stamp.ask(t, a)), Path::new("a.svg"));
    assert_eq!(boot_flags(&stamp.only_request().1), expected);
    let search = Machine::answering(fixture!("files/search-names.json"));
    with_boot().search_with(&|t, a| Ok(search.ask(t, a)), "main", Mode::Names);
    assert_eq!(boot_flags(&search.only_request().1), expected);
    let margin = Machine::answering(fixture!("changes/file.json"));
    with_boot().gutter_with(&|t, a| Ok(margin.ask(t, a)), Path::new("a.txt"));
    assert_eq!(boot_flags(&margin.only_request().1), expected);
}

#[test]
fn below_the_gate_no_read_on_a_machine_carries_a_boot() {
    let search = Machine::answering(fixture!("files/search-names.json"));
    there().search_with(&|t, a| Ok(search.ask(t, a)), "main", Mode::Names);
    assert!(search.asked.borrow().iter().all(|(_, args)| !args.contains(&"--boot".to_string())));
}

#[test]
fn from_0_13_66_a_search_and_the_margin_ask_no_root_listing_first() {
    let search = Machine::answering(fixture!("files/search-names.json"));
    with_boot().search_with(&|t, a| Ok(search.ask(t, a)), "main", Mode::Names);
    assert_eq!(search.only_request().1[1], "search");
    let margin = Machine::answering(fixture!("changes/file.json"));
    with_boot().gutter_with(&|t, a| Ok(margin.ask(t, a)), Path::new("a.txt"));
    assert_eq!(margin.only_request().1[..2], ["changes", "file"]);
}

#[test]
fn below_0_13_66_a_search_still_lists_the_root_first() {
    let search = Machine::answering(fixture!("files/search-names.json"));
    there().search_with(&|t, a| Ok(search.ask(t, a)), "main", Mode::Names);
    assert_eq!(search.asked_which_run_first().1[1], "search");
}

#[test]
fn a_listing_refused_for_its_boot_is_told_by_its_type_not_its_words() {
    let listed = with_boot().list_with(&refused, vec![PathBuf::new()]);
    assert!(restarted_in(&listed));
    assert_eq!(lost_in(&listed).as_deref(), Some("vps restarted"));
    let words_only = |_: &Target, _: &[String]| -> Result<String> { Err(anyhow!("vps restarted")) };
    assert!(!restarted_in(&with_boot().list_with(&words_only, vec![PathBuf::new()])));
}

#[test]
fn a_read_refused_for_its_boot_says_the_machine_restarted() {
    let (content, stamp) = with_boot().read_with(&refused, Path::new("README.md"));
    assert!(matches!(&content, Content::Restarted(why) if why == "vps restarted"), "{content:?}");
    assert_eq!(stamp, None);
}

#[test]
fn a_search_refused_for_its_boot_fails_with_the_machines_restart() {
    let results = with_boot().search_with(&refused, "main", Mode::Names);
    assert!(matches!(&results, Results::Failed(why) if why == "vps restarted"));
}

#[test]
fn a_refused_read_is_kept_apart_from_a_lost_link_by_its_type() {
    let (refused, _) = with_boot().read_with(&refused, Path::new("README.md"));
    assert!(matches!(&refused, Content::Restarted(why) if why == "vps restarted"), "{refused:?}");
    let (lost, _) = with_boot().read_with(&down, Path::new("README.md"));
    assert!(matches!(lost, Content::Lost(_)));
    let words_only = |_: &Target, _: &[String]| -> Result<String> { Err(anyhow!("vps restarted")) };
    let (plain, _) = with_boot().read_with(&words_only, Path::new("README.md"));
    assert!(matches!(plain, Content::Lost(_)));
}

#[test]
fn a_tree_with_nothing_listed_says_nothing_was_read_once_the_machine_refused() {
    let mut retry = Retry::default();
    let read = vps_read(Reach::Up(target()));
    assert_eq!(unread_root(&read, false, &retry), None);
    retry.restarted();
    assert_eq!(unread_root(&read, false, &retry).as_deref(), Some("Nothing read from vps"));
    assert_eq!(unread_root(&read, true, &retry), None);
}
