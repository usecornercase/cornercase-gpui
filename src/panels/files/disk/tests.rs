use super::*;

#[test]
fn text_lines_drop_cr_and_expand_tabs() {
    assert_eq!(*text("a\r\n\tb\n").lines, vec!["a", "    b"]);
    assert_eq!(*text("").lines, vec![""]);
    assert_eq!(*text("x\n\n").lines, vec!["x", ""]);
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
    let missing = Path::new(env!("CARGO_MANIFEST_DIR")).join("definitely").join("not").join("here");
    assert!(matches!(read(&missing).0, Content::Gone(_)));
    assert!(matches!(read(Path::new(env!("CARGO_MANIFEST_DIR"))).0, Content::NotRegular));
}

#[cfg(unix)]
#[test]
fn a_device_is_never_read() {
    assert!(matches!(read(Path::new("/dev/zero")).0, Content::NotRegular));
}

#[test]
fn sizes_read_well() {
    assert_eq!(human_size(12), "12 B");
    assert_eq!(human_size(1536), "1.5 KB");
    assert_eq!(human_size(3 * 1024 * 1024), "3.0 MB");
}

#[test]
fn a_line_without_tabs_gives_what_is_shown() {
    assert_eq!(text("plain line\n").own(0, 6..10), "line");
}

#[test]
fn a_whole_line_with_tabs_gives_the_file_s_own_text() {
    assert_eq!(text("\tb\tc\n").own(0, 0.."    b    c".len()), "\tb\tc");
}

#[test]
fn part_of_a_line_with_tabs_maps_back_to_the_file_s_own_text() {
    assert_eq!(text("\tab\tc\n").own(0, 4..6), "ab");
}

#[test]
fn any_of_a_tab_s_spaces_stands_for_the_tab() {
    assert_eq!(text("\tab\tc\n").own(0, 2..6), "\tab");
    assert_eq!(text("\tab\tc\n").own(0, 5..7), "b\t");
}

#[test]
fn an_empty_part_of_a_line_with_tabs_gives_nothing() {
    assert_eq!(text("\tab\n").own(0, 5..5), "");
}

#[test]
fn a_line_ending_in_a_carriage_return_loses_it_in_its_own_text() {
    assert_eq!(text("\ta\r\n").own(0, 0..5), "\ta");
}

#[test]
fn non_ascii_text_after_a_tab_keeps_whole_characters() {
    assert_eq!(text("\tñu\n").own(0, 4..6), "ñ");
}

#[test]
fn a_line_past_the_end_gives_nothing() {
    assert_eq!(text("a\n").own(9, 0..1), "");
}
