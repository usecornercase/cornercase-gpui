use std::collections::HashSet;

use super::*;
use crate::platform::Os;

fn taken(names: &[&str]) -> impl Fn(&Path) -> bool {
    let names: HashSet<PathBuf> = names.iter().map(|n| Path::new("/d").join(n)).collect();
    move |path| names.contains(path)
}

#[test]
fn a_free_name_is_kept() {
    assert_eq!(free_path(Path::new("/d"), "report.pdf", taken(&[])), Path::new("/d/report.pdf"));
}

#[test]
fn a_taken_name_gets_the_next_number_before_its_extension() {
    let path = free_path(Path::new("/d"), "report.pdf", taken(&["report.pdf", "report (1).pdf"]));
    assert_eq!(path, Path::new("/d/report (2).pdf"));
}

#[test]
fn a_tarball_keeps_both_its_extensions() {
    let path = free_path(Path::new("/d"), "src.tar.gz", taken(&["src.tar.gz"]));
    assert_eq!(path, Path::new("/d/src (1).tar.gz"));
}

#[test]
fn a_name_without_an_extension_or_with_a_leading_dot_is_numbered_at_its_end() {
    assert_eq!(free_path(Path::new("/d"), "LICENSE", taken(&["LICENSE"])), Path::new("/d/LICENSE (1)"));
    assert_eq!(free_path(Path::new("/d"), ".env", taken(&[".env"])), Path::new("/d/.env (1)"));
}

#[test]
fn a_suggested_name_cannot_leave_the_folder() {
    for os in [Os::Mac, Os::Linux, Os::Windows] {
        assert_eq!(safe_name("../../etc/passwd", os), "passwd");
        assert_eq!(safe_name("..\\evil.bat", os), "evil.bat");
        assert_eq!(safe_name("..", os), "download");
        assert_eq!(safe_name("", os), "download");
    }
}

#[test]
fn on_windows_characters_it_refuses_in_a_name_become_underscores() {
    assert_eq!(safe_name("a<b>c:d\"e|f?g*h.txt", Os::Windows), "a_b_c_d_e_f_g_h.txt");
    assert_eq!(safe_name("notes.txt:hidden", Os::Windows), "notes.txt_hidden");
    assert_eq!(safe_name("tab\there.txt", Os::Windows), "tab_here.txt");
    assert_eq!(safe_name("a:b?.txt", Os::Mac), "a:b?.txt");
}

#[test]
fn on_windows_a_name_loses_its_trailing_dots_and_spaces() {
    assert_eq!(safe_name("report. . .", Os::Windows), "report");
    assert_eq!(safe_name("...", Os::Windows), "download");
    assert_eq!(safe_name("report.", Os::Linux), "report.");
}

#[test]
fn on_windows_a_device_name_gets_an_underscore_whatever_its_case_or_extension() {
    assert_eq!(safe_name("CON", Os::Windows), "_CON");
    assert_eq!(safe_name("nul.tar.gz", Os::Windows), "_nul.tar.gz");
    assert_eq!(safe_name("Com1.log", Os::Windows), "_Com1.log");
    assert_eq!(safe_name("lpt\u{b9}", Os::Windows), "_lpt\u{b9}");
    assert_eq!(safe_name("console.txt", Os::Windows), "console.txt");
    assert_eq!(safe_name("COM10.txt", Os::Windows), "COM10.txt");
    assert_eq!(safe_name("CON", Os::Linux), "CON");
}

#[test]
fn a_folder_in_the_home_is_shown_from_the_tilde() {
    assert_eq!(shown_dir(Path::new("/Users/me/Downloads"), Path::new("/Users/me"), Os::Mac), "~/Downloads");
    assert_eq!(shown_dir(Path::new("/tmp/x"), Path::new("/Users/me"), Os::Linux), "/tmp/x");
}

#[test]
fn on_windows_the_downloads_folder_is_shown_whole() {
    let downloads = Path::new("/Users/me/Downloads");
    assert_eq!(shown_dir(downloads, Path::new("/Users/me"), Os::Windows), downloads.display().to_string());
}
