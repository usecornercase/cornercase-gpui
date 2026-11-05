use super::*;
use crate::test_util::fixture;

const LATEST: &str = fixture!("releases/latest.json");
const CHANGELOG: &str = fixture!("releases/changelog.md");

fn latest() -> Latest {
    let answer: Value = serde_json::from_str(LATEST).expect("the fixture is JSON");
    parse(&answer).expect("a release")
}

fn versions(notes: &[Notes]) -> Vec<&str> {
    notes.iter().map(|n| n.version.as_str()).collect()
}

#[test]
fn a_release_is_read_from_githubs_answer() {
    let latest = latest();
    assert_eq!(latest.version, "0.2.0");
    assert_eq!(latest.page, "https://github.com/usecornercase/cornercase-gpui/releases/tag/v0.2.0");
    assert!(latest.body.starts_with("- A change of 0.2.0."));
    assert_eq!(latest.assets.len(), 6);
    assert!(latest.asset("CHANGELOG.md").is_some());
}

#[test]
fn a_release_without_a_tag_is_none() {
    assert_eq!(parse(&serde_json::json!({ "message": "Not Found" })), None);
}

#[test]
fn a_newer_patch_is_newer() {
    assert!(newer("0.1.1", "0.1.0"));
}

#[test]
fn the_same_version_is_not_newer() {
    assert!(!newer("0.1.0", "0.1.0"));
}

#[test]
fn an_older_version_is_not_newer() {
    assert!(!newer("0.1.0", "0.2.0"));
}

#[test]
fn a_release_is_newer_than_its_pre_release() {
    assert!(newer("0.2.0", "0.2.0-rc.1"));
    assert!(!newer("0.2.0-rc.1", "0.2.0"));
}

#[test]
fn an_unreadable_version_is_never_newer() {
    assert!(!newer("soon", "0.1.0"));
    assert!(!newer("0.2.0", "soon"));
}

#[test]
fn the_notes_are_the_sections_after_the_running_version() {
    let notes = notes_since(CHANGELOG, "0.1.0", "0.3.0");
    assert_eq!(versions(&notes), ["0.3.0", "0.2.0"]);
    assert_eq!(notes[1].text, "- A change of 0.2.0.\n- Another change of 0.2.0.");
}

#[test]
fn sections_after_the_release_are_left_out() {
    assert_eq!(versions(&notes_since(CHANGELOG, "0.1.0", "0.2.0")), ["0.2.0"]);
}

#[test]
fn unreleased_and_empty_sections_are_left_out() {
    let notes = notes_since(CHANGELOG, "0.0.1", "0.3.0");
    assert_eq!(versions(&notes), ["0.3.0", "0.2.0", "0.1.0"]);
}

#[test]
fn the_body_stands_in_when_the_changelog_lacks_the_release() {
    assert_eq!(notes_since(CHANGELOG, "0.3.0", "0.4.0"), []);
    let notes = body_notes("0.4.0", "- Something.\n");
    assert_eq!(notes, [Notes { version: "0.4.0".into(), text: "- Something.".into() }]);
    assert_eq!(body_notes("0.4.0", "  \n"), []);
}

#[test]
fn each_platform_downloads_its_own_asset() {
    assert_eq!(asset_name(Os::Mac, "aarch64"), Some("cornercase-gpui-universal-apple-darwin.dmg"));
    assert_eq!(asset_name(Os::Windows, "x86_64"), Some("cornercase-gpui-x86_64-pc-windows-msvc.zip"));
    assert_eq!(asset_name(Os::Linux, "x86_64"), Some("cornercase-gpui-x86_64-unknown-linux-gnu.tar.gz"));
    assert_eq!(sums_name("a.zip"), "a.zip.sha256");
}

#[test]
fn a_machine_without_a_download_has_no_asset() {
    assert_eq!(asset_name(Os::Linux, "aarch64"), None);
    assert_eq!(asset_name(Os::Windows, "aarch64"), None);
}

#[test]
fn a_download_matches_its_checksum() {
    let sums = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824  hello.zip\n";
    assert!(matches(b"hello", sums));
    assert!(matches(b"hello", &sums.to_uppercase()));
}

#[test]
fn a_download_that_differs_from_its_checksum_is_refused() {
    let sums = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824  hello.zip\n";
    assert!(!matches(b"hellO", sums));
    assert!(!matches(b"hello", ""));
}

#[test]
fn the_version_line_names_the_program() {
    assert_eq!(version_line(), format!("cornercase-gpui {}", env!("CARGO_PKG_VERSION")));
    assert_eq!(printed_version(&version_line()), Some(env!("CARGO_PKG_VERSION")));
}

#[test]
fn another_programs_version_line_is_not_read() {
    assert_eq!(printed_version("cornercase 0.13.1"), None);
}

#[test]
fn a_build_from_source_is_updated_with_cargo() {
    assert_eq!(
        source_command("0.2.0"),
        "cargo install --locked --git https://github.com/usecornercase/cornercase-gpui --tag v0.2.0"
    );
}
