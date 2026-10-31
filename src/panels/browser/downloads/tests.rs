use std::collections::HashSet;

use super::*;

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
    assert_eq!(safe_name("../../etc/passwd"), "passwd");
    assert_eq!(safe_name("..\\evil.bat"), "evil.bat");
    assert_eq!(safe_name(".."), "download");
    assert_eq!(safe_name(""), "download");
}

#[test]
fn a_folder_in_the_home_is_shown_from_the_tilde() {
    assert_eq!(shown_dir(Path::new("/Users/me/Downloads"), Path::new("/Users/me")), "~/Downloads");
    assert_eq!(shown_dir(Path::new("/tmp/x"), Path::new("/Users/me")), "/tmp/x");
}
