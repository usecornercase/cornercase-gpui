use std::path::Path;

use super::*;

#[test]
fn quick_look_writes_one_thumbnail_into_the_folder() {
    let args = args(Path::new("/a/doc.pdf"), Path::new("/tmp/out"));
    let args: Vec<&str> = args.iter().filter_map(|a| a.to_str()).collect();
    assert_eq!(args, ["-t", "-s", "900", "-o", "/tmp/out", "/a/doc.pdf"]);
}

#[test]
fn the_thumbnail_is_named_after_the_file() {
    assert_eq!(made(Path::new("/a/doc.pdf"), Path::new("/tmp/out")), Some("/tmp/out/doc.pdf.png".into()));
    assert_eq!(made(Path::new("/"), Path::new("/tmp/out")), None);
}

#[test]
fn a_helper_that_fails_says_how() {
    let shell = |script: &str| finished_within(Command::new("sh").args(["-c", script]), Duration::from_secs(5));
    assert_eq!(shell("exit 0"), Ok(()));
    assert_eq!(shell("exit 3"), Err("ended with exit status: 3".to_string()));
    let missing = finished_within(&mut Command::new("/nowhere/at-all"), Duration::from_secs(5));
    assert!(missing.expect_err("nothing runs").starts_with("could not start: "));
}

#[test]
fn a_helper_still_running_at_the_limit_is_stopped() {
    let late = finished_within(Command::new("sh").args(["-c", "exec sleep 30"]), Duration::from_millis(50));
    assert_eq!(late, Err("did not finish within 0 s".to_string()));
}

#[test]
fn a_picture_is_read_up_to_its_cap() {
    let dir = Scratch::folder("test").expect("the temporary folder is writable");
    let png = dir.path().join("thumbnail.png");
    assert_eq!(picture(&png, 4), Err("made no picture".to_string()));
    std::fs::write(&png, b"12345").expect("the folder is writable");
    assert_eq!(picture(&png, 5), Ok(b"12345".to_vec()));
    assert_eq!(picture(&png, 4), Err("made a picture over 4 B".to_string()));
}
