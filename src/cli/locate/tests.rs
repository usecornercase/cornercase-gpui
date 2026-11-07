use std::cell::Cell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::{
    Binary, Inputs, MINIMUM, Missing, Origin, Probe, Problem, closed, installer_dirs, locate, merged, parse_version,
    shell_args, shell_path, unix_dirs,
};
use crate::platform::Os;

const MARK: &str = "cornercase-gpui-path-mark";

#[derive(Default)]
struct Fake {
    programs: Vec<PathBuf>,
    versions: HashMap<PathBuf, Result<Vec<String>, String>>,
    shell_path: Option<String>,
    asked: Cell<u32>,
}

impl Fake {
    fn with(mut self, path: &str, version: &str) -> Self {
        self.programs.push(PathBuf::from(path));
        self.versions.insert(PathBuf::from(path), Ok(vec![version.to_string()]));
        self
    }

    fn shell(mut self, path: &str) -> Self {
        self.programs.push(PathBuf::from("/bin/zsh"));
        self.shell_path = Some(path.to_string());
        self
    }
}

impl Probe for Fake {
    fn find(&self, dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
        dirs.iter().map(|dir| dir.join(name)).find(|path| self.programs.contains(path))
    }

    fn is_program(&self, path: &Path) -> bool {
        self.programs.iter().any(|program| program == path)
    }

    fn version(&self, path: &Path, _: Option<&str>) -> Result<Vec<String>, String> {
        self.versions.get(path).cloned().unwrap_or_else(|| Err("no such file".to_string()))
    }

    fn login_path(&self, _: &Path) -> Option<String> {
        self.asked.set(self.asked.get() + 1);
        self.shell_path.clone()
    }
}

const GOOD: &str = "cornercase 0.13.19";

fn inputs(os: Os) -> Inputs {
    Inputs {
        os,
        variable: None,
        chosen: None,
        path: vec![PathBuf::from("/usr/bin"), PathBuf::from("/bin")],
        shell: Some(PathBuf::from("/bin/zsh")),
        home: Some(PathBuf::from("/home/ana")),
        cargo_home: None,
    }
}

fn mac() -> Inputs {
    inputs(Os::Mac)
}

fn found(result: Result<Binary, Missing>) -> Binary {
    result.expect("a usable cornercase")
}

fn lines(items: &[&str]) -> Vec<String> {
    items.iter().map(ToString::to_string).collect()
}

#[test]
fn a_version_line_reads_as_its_numbers() {
    let version = parse_version("cornercase 0.12.287\n").expect("a version");
    assert_eq!(version.to_string(), "0.12.287");
}

#[test]
fn a_pre_release_or_build_suffix_keeps_its_version() {
    assert_eq!(parse_version("cornercase 0.12.290-dev+abc"), parse_version("cornercase 0.12.290"));
    assert!(parse_version("cornercase 0.12.290-dev+abc").is_some());
}

#[test]
fn output_of_another_program_has_no_version() {
    for line in ["git version 2.47.0", "", "cornercase", "cornercase 0.12", "cornercase 0.12.x"] {
        assert_eq!(parse_version(line), None, "{line}");
    }
}

#[test]
fn versions_compare_as_numbers() {
    let [a, b, c] = ["0.12.1000", "0.12.234", "0.9.999"].map(|v| parse_version(&format!("cornercase {v}")));
    assert!(a > b && b > c);
}

#[test]
fn every_platform_needs_the_release_with_every_command_the_window_uses() {
    assert_eq!(MINIMUM.to_string(), "0.13.19");
}

#[test]
fn the_installers_folders_on_a_mac() {
    let dirs = installer_dirs(Os::Mac, Some(Path::new("/Users/ana")), None);
    let expected = ["/Users/ana/.local/bin", "/Users/ana/.cargo/bin", "/opt/homebrew/bin", "/usr/local/bin"];
    assert_eq!(dirs, expected.map(PathBuf::from));
}

#[test]
fn the_installers_folders_on_linux() {
    let dirs = installer_dirs(Os::Linux, Some(Path::new("/home/ana")), None);
    let expected = [
        "/home/ana/.local/bin",
        "/home/ana/.cargo/bin",
        "/home/linuxbrew/.linuxbrew/bin",
        "/home/ana/.linuxbrew/bin",
        "/usr/local/bin",
    ];
    assert_eq!(dirs, expected.map(PathBuf::from));
}

#[test]
fn the_installers_folders_on_windows() {
    let dirs = installer_dirs(Os::Windows, Some(Path::new("/home/ana")), None);
    assert_eq!(dirs, ["/home/ana/.local/bin", "/home/ana/.cargo/bin"].map(PathBuf::from));
}

#[test]
fn cargo_home_moves_the_folder_of_a_build_from_source() {
    let dirs = installer_dirs(Os::Linux, Some(Path::new("/home/ana")), Some(Path::new("/opt/cargo")));
    assert!(dirs.contains(&PathBuf::from("/opt/cargo/bin")));
    assert!(!dirs.contains(&PathBuf::from("/home/ana/.cargo/bin")));
}

#[test]
fn without_a_home_only_the_system_folders_are_searched() {
    let dirs = installer_dirs(Os::Mac, None, None);
    assert_eq!(dirs, ["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
}

#[test]
fn cornercase_bin_is_the_only_candidate_when_set() {
    let probe = Fake::default().with("/usr/bin/cornercase", GOOD);
    let inputs = Inputs { variable: Some("/nope/cornercase".into()), ..mac() };
    let Err(Missing::Unusable { origin, problem, .. }) = locate(&inputs, &probe) else { panic!("unusable") };
    assert_eq!((origin, problem), (Origin::Variable, Problem::NotAProgram));
    assert_eq!(probe.asked.get(), 0);
}

#[test]
fn a_bare_cornercase_bin_is_looked_for_on_the_path() {
    let probe = Fake::default().with("/bin/cc-wrapper", GOOD);
    let inputs = Inputs { variable: Some("cc-wrapper".into()), ..mac() };
    assert_eq!(found(locate(&inputs, &probe)).path, PathBuf::from("/bin/cc-wrapper"));
}

#[test]
fn a_binary_chosen_by_hand_comes_before_the_path() {
    let probe = Fake::default().with("/usr/bin/cornercase", GOOD).with("/opt/cc/cornercase", GOOD);
    let inputs = Inputs { chosen: Some("/opt/cc/cornercase".into()), ..mac() };
    assert_eq!(found(locate(&inputs, &probe)).path, PathBuf::from("/opt/cc/cornercase"));
}

#[test]
fn a_chosen_binary_that_is_gone_is_passed_over() {
    let probe = Fake::default().with("/usr/bin/cornercase", GOOD);
    let inputs = Inputs { chosen: Some("/gone/cornercase".into()), ..mac() };
    assert_eq!(found(locate(&inputs, &probe)).path, PathBuf::from("/usr/bin/cornercase"));
}

#[test]
fn a_chosen_binary_that_is_too_old_gives_way_to_a_good_one_on_the_path() {
    let probe = Fake::default().with("/opt/cc/cornercase", "cornercase 0.12.100").with("/usr/bin/cornercase", GOOD);
    let inputs = Inputs { chosen: Some("/opt/cc/cornercase".into()), ..mac() };
    assert_eq!(found(locate(&inputs, &probe)).path, PathBuf::from("/usr/bin/cornercase"));
}

#[test]
fn a_chosen_binary_that_is_too_old_is_named_when_nothing_else_is_found() {
    let probe = Fake::default().with("/opt/cc/cornercase", "cornercase 0.12.100");
    let inputs = Inputs { chosen: Some("/opt/cc/cornercase".into()), ..mac() };
    let Err(Missing::Unusable { origin, problem, .. }) = locate(&inputs, &probe) else { panic!("unusable") };
    assert_eq!(origin, Origin::Chosen);
    assert!(matches!(problem, Problem::TooOld(_)));
}

#[test]
fn a_chosen_binary_that_is_gone_is_named_when_nothing_else_is_found() {
    let inputs = Inputs { chosen: Some("/gone/cornercase".into()), ..mac() };
    let Err(Missing::Unusable { path, origin, problem }) = locate(&inputs, &Fake::default()) else {
        panic!("unusable")
    };
    assert_eq!((path, origin, problem), (PathBuf::from("/gone/cornercase"), Origin::Chosen, Problem::NotAProgram));
}

#[test]
fn the_login_shell_is_asked_only_when_nothing_else_is_on_the_path() {
    let probe = Fake::default().with("/usr/bin/cornercase", GOOD).shell("/opt/homebrew/bin");
    assert!(locate(&mac(), &probe).is_ok());
    assert_eq!(probe.asked.get(), 0);
}

#[test]
fn windows_never_asks_a_login_shell() {
    let probe = Fake::default().shell("/opt/homebrew/bin");
    assert!(locate(&inputs(Os::Windows), &probe).is_err());
    assert_eq!(probe.asked.get(), 0);
}

#[test]
fn the_login_shell_comes_before_the_installers_folders() {
    let probe = Fake::default()
        .with("/opt/homebrew/bin/cornercase", GOOD)
        .with("/home/ana/.local/bin/cornercase", GOOD)
        .shell("/opt/homebrew/bin");
    let binary = found(locate(&mac(), &probe));
    assert_eq!(binary.path, PathBuf::from("/opt/homebrew/bin/cornercase"));
}

#[test]
fn a_shell_that_answers_nothing_leaves_the_installers_folders() {
    let mut probe = Fake::default().with("/home/ana/.local/bin/cornercase", GOOD);
    probe.programs.push(PathBuf::from("/bin/zsh"));
    let binary = found(locate(&mac(), &probe));
    assert_eq!((binary.path, probe.asked.get()), (PathBuf::from("/home/ana/.local/bin/cornercase"), 1));
}

#[test]
fn found_through_the_login_shell_runs_with_that_shell_s_path() {
    let probe = Fake::default().with("/opt/homebrew/bin/cornercase", GOOD).shell("/opt/homebrew/bin:/usr/bin");
    let binary = found(locate(&mac(), &probe));
    assert_eq!(binary.path_env.as_deref(), Some("/opt/homebrew/bin:/usr/bin:/bin"));
}

#[test]
fn found_elsewhere_keeps_the_window_s_own_path() {
    let probe = Fake::default().with("/usr/bin/cornercase", GOOD);
    assert_eq!(found(locate(&mac(), &probe)).path_env, None);
}

#[test]
fn the_shell_s_path_comes_first_and_keeps_the_system_folders() {
    let shell = unix_dirs("/opt/bin:/usr/bin:relative:");
    let process = unix_dirs("/usr/bin:/sbin");
    assert_eq!(merged(&shell, &process), "/opt/bin:/usr/bin:/sbin");
}

#[test]
fn the_shell_s_path_is_read_between_the_marks() {
    let output = lines(&["Last login: today", MARK, "/a:/b", MARK, "bye"]);
    assert_eq!(shell_path(&output).as_deref(), Some("/a:/b"));
    assert!(closed(&output));
}

#[test]
fn a_shell_that_printed_no_marks_gives_no_path() {
    assert_eq!(shell_path(&lines(&["/a:/b"])), None);
    assert_eq!(shell_path(&lines(&[MARK, "/a:/b"])), None);
}

#[test]
fn zsh_is_asked_as_an_interactive_login_shell() {
    assert_eq!(&shell_args(Path::new("/bin/zsh"))[..3], ["-l", "-i", "-c"]);
}

#[test]
fn csh_is_asked_without_dash_l() {
    assert_eq!(&shell_args(Path::new("/bin/tcsh"))[..2], ["-i", "-c"]);
    assert_eq!(shell_args(Path::new("/bin/tcsh")).len(), 3);
}

#[test]
fn an_old_binary_is_named_with_its_version() {
    let probe = Fake::default().with("/usr/bin/cornercase", "cornercase 0.12.100");
    let Err(Missing::Unusable { problem: Problem::TooOld(version), .. }) = locate(&mac(), &probe) else {
        panic!("too old")
    };
    assert_eq!(version.to_string(), "0.12.100");
}

#[test]
fn a_binary_that_cannot_start_says_why() {
    let mut probe = Fake::default().with("/usr/bin/cornercase", GOOD);
    probe.versions.insert(PathBuf::from("/usr/bin/cornercase"), Err("Permission denied".into()));
    let Err(Missing::Unusable { problem, .. }) = locate(&mac(), &probe) else { panic!("unusable") };
    assert_eq!(problem, Problem::DoesNotStart("Permission denied".into()));
}

#[test]
fn another_program_is_not_cornercase() {
    let probe = Fake::default().with("/usr/bin/cornercase", "git version 2.47.0");
    let Err(Missing::Unusable { problem, .. }) = locate(&mac(), &probe) else { panic!("unusable") };
    assert_eq!(problem, Problem::NotCornercase("git version 2.47.0".into()));
}

#[test]
fn nothing_found_names_the_places_looked_in() {
    let probe = Fake::default().shell("");
    let Err(Missing::NotFound { places, asked_shell }) = locate(&mac(), &probe) else { panic!("not found") };
    assert_eq!(places, installer_dirs(Os::Mac, Some(Path::new("/home/ana")), None));
    assert!(asked_shell);
    let Err(Missing::NotFound { asked_shell, .. }) = locate(&inputs(Os::Windows), &probe) else { panic!("not found") };
    assert!(!asked_shell);
}
