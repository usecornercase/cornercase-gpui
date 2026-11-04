use std::path::PathBuf;

use super::{emblem_of, heading_of, lead, offers_choice};
use crate::cli::{Missing, Origin, Problem, parse_version};
use crate::platform::Os;
use crate::ui::Icon;

fn unusable(path: &str, origin: Origin, problem: Problem) -> Missing {
    Missing::Unusable { path: PathBuf::from(path), origin, problem }
}

fn too_old(path: &str, origin: Origin) -> Missing {
    let version = parse_version("cornercase 0.12.100").expect("a version");
    unusable(path, origin, Problem::TooOld(version))
}

#[test]
fn not_found_names_where_it_looked_with_the_home_as_a_tilde() {
    let missing = Missing::NotFound { places: vec![PathBuf::from("/Users/ana/.local/bin")], asked_shell: true };
    let text = lead(&missing, Os::Mac, "/Users/ana");
    assert!(text.contains("~/.local/bin"), "{text}");
    assert!(text.contains("login shell's PATH"), "{text}");
    assert!(text.contains("0.12.234"), "{text}");
    assert_eq!(heading_of(&missing), "cornercase was not found");
}

#[test]
fn on_windows_it_looked_only_on_the_path_and_in_the_installers_folder() {
    let missing = Missing::NotFound { places: Vec::new(), asked_shell: false };
    let text = lead(&missing, Os::Windows, "");
    assert!(!text.contains("login shell"), "{text}");
    assert!(text.contains("0.12.287") && text.contains("PC"), "{text}");
}

#[test]
fn a_too_old_binary_names_its_version_and_the_minimum() {
    let missing = too_old("/usr/local/bin/cornercase", Origin::Path);
    assert_eq!(heading_of(&missing), "cornercase 0.12.100 is too old");
    assert!(lead(&missing, Os::Mac, "").contains("0.12.234"));
    assert!(lead(&missing, Os::Windows, "").contains("0.12.287"));
    assert_eq!(emblem_of(&missing), Icon::Alert);
}

#[test]
fn cornercase_bin_says_so_and_offers_no_choice() {
    let missing = too_old("/opt/cc", Origin::Variable);
    assert!(lead(&missing, Os::Mac, "").contains("(CORNERCASE_BIN)"));
    assert!(lead(&missing, Os::Mac, "").contains("CORNERCASE_BIN decides"));
    assert!(!offers_choice(&missing));
    assert!(offers_choice(&Missing::NotFound { places: Vec::new(), asked_shell: false }));
}

#[test]
fn a_binary_chosen_by_hand_is_said_to_be() {
    let missing = too_old("/opt/cc", Origin::Chosen);
    assert!(lead(&missing, Os::Mac, "").contains("(chosen by hand)"));
}
