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
