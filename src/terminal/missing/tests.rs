use std::path::PathBuf;

use super::{Choice, choice, emblem_of, heading_of, lead, offers_choice, too_old_path, update_question};
use crate::cli::{Binary, Missing, Origin, Problem, parse_version};
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
    assert!(text.contains("0.13.19"), "{text}");
    assert_eq!(heading_of(&missing), "cornercase was not found");
}

#[test]
fn on_windows_it_looked_only_on_the_path_and_in_the_installers_folder() {
    let missing = Missing::NotFound { places: Vec::new(), asked_shell: false };
    let text = lead(&missing, Os::Windows, "");
    assert!(!text.contains("login shell"), "{text}");
    assert!(text.contains("0.13.19") && text.contains("PC"), "{text}");
}

#[test]
fn a_too_old_binary_names_its_version_and_the_minimum() {
    let missing = too_old("/usr/local/bin/cornercase", Origin::Path);
    assert_eq!(heading_of(&missing), "cornercase 0.12.100 is too old");
    assert!(lead(&missing, Os::Mac, "").contains("0.13.19"));
    assert!(lead(&missing, Os::Windows, "").contains("0.13.19"));
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

#[test]
fn a_good_pick_is_used_and_saved_by_its_path() {
    let binary = Binary::plain(PathBuf::from("/opt/cc/cornercase"));
    let Choice::Use(used, saved) = choice(Ok(binary.clone())) else { panic!("used") };
    assert_eq!((used, saved.as_str()), (binary, "/opt/cc/cornercase"));
}

#[test]
fn a_bad_pick_is_kept_out_of_the_settings() {
    let missing = too_old("/opt/cc", Origin::Chosen);
    assert!(matches!(choice(Err(missing.clone())), Choice::Stay(m) if m == missing));
}

#[cfg(unix)]
#[test]
fn a_pick_whose_path_is_not_text_is_not_saved() {
    use std::os::unix::ffi::OsStringExt;
    let path = PathBuf::from(std::ffi::OsString::from_vec(vec![b'/', 0xff, b'c']));
    assert!(matches!(choice(Ok(Binary::plain(path))), Choice::Stay(_)));
}

#[test]
fn only_a_cornercase_too_old_offers_its_update() {
    let path = std::path::Path::new("/opt/cc");
    assert_eq!(too_old_path(&too_old("/opt/cc", Origin::Path)), Some(path));
    let others = [
        Missing::NotFound { places: Vec::new(), asked_shell: false },
        unusable("/opt/cc", Origin::Path, Problem::NotAProgram),
        unusable("/opt/cc", Origin::Path, Problem::DoesNotStart("x".into())),
        unusable("/opt/cc", Origin::Path, Problem::NotCornercase("x".into())),
    ];
    for other in others {
        assert_eq!(too_old_path(&other), None, "{other:?}");
    }
}

#[test]
fn the_update_here_asks_first_and_names_the_binary() {
    let path = std::path::Path::new("/Users/ana/.local/bin/cornercase");
    let (title, message, button) = update_question(path, Os::Mac, "/Users/ana");
    assert_eq!(title, "Update cornercase on this Mac?");
    assert!(message.contains("~/.local/bin/cornercase") && message.contains("Homebrew"), "{message}");
    assert_eq!(button, "Update");
}

#[test]
fn a_too_old_binary_offers_to_update_it_here() {
    let text = lead(&too_old("/opt/cc", Origin::Path), Os::Mac, "/Users/ana");
    assert!(text.contains("Update it here"), "{text}");
}
