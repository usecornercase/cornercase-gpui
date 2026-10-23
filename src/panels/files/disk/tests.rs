use super::*;

#[test]
fn text_lines_drop_cr_and_expand_tabs() {
    assert_eq!(text("a\r\n\tb\n").lines, vec!["a", "    b"]);
    assert_eq!(text("").lines, vec![""]);
    assert_eq!(text("x\n\n").lines, vec!["x", ""]);
}

fn class(path: &str, bytes: Vec<u8>) -> String {
    match classify(Path::new(path), bytes) {
        Content::Image(image) => format!("{:?} image", image.format),
        other => format!("{other:?}").split('(').next().unwrap_or_default().to_string(),
    }
}

#[test]
fn classifies_by_content() {
    assert_eq!(class("f.txt", b"hello".to_vec()), "Text");
    assert_eq!(class("f.txt", b"he\0llo".to_vec()), "Binary");
    assert_eq!(class("f.txt", b"\x89PNG\r\n\x1a\nrest".to_vec()), "Png image");
    assert_eq!(class("a.SVG", b"<svg/>".to_vec()), "Svg image");
    let too_large = usize::try_from(MAX_BYTES).expect("the cap fits in memory") + 1;
    assert_eq!(class("f.txt", vec![b'a'; too_large]), "TooLarge");
    assert_eq!(class("f.txt", vec![0xff, b'a']), "Text");
}

#[test]
fn reads_only_regular_files() {
    assert!(matches!(read(Path::new("/dev/zero")).0, Content::NotRegular));
    assert!(matches!(read(Path::new("/definitely/not/here")).0, Content::Gone(_)));
    assert!(matches!(read(Path::new(env!("CARGO_MANIFEST_DIR"))).0, Content::NotRegular));
}

#[test]
fn sizes_read_well() {
    assert_eq!(human_size(12), "12 B");
    assert_eq!(human_size(1536), "1.5 KB");
    assert_eq!(human_size(3 * 1024 * 1024), "3.0 MB");
}
