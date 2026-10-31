use std::io::Cursor;
use std::path::Path;

use super::*;
use crate::panels::files::disk::Text;

fn png(w: u32, h: u32) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    image::RgbaImage::new(w, h).write_to(&mut bytes, image::ImageFormat::Png).expect("encodes");
    bytes.into_inner()
}

fn note_of(still: &Still) -> Option<&'static str> {
    match still {
        Still::Note(note) => Some(note.title),
        _ => None,
    }
}

#[test]
fn videos_and_sounds_are_told_by_their_extension() {
    assert_eq!(media_of(Path::new("demo/clip.MOV")), Some(Media::Video));
    assert_eq!(media_of(Path::new("a.mp4")), Some(Media::Video));
    assert_eq!(media_of(Path::new("voice.m4a")), Some(Media::Audio));
    assert_eq!(media_of(Path::new("clip.webm")), None);
    assert_eq!(media_of(Path::new("Makefile")), None);
}

#[test]
fn a_picture_knows_its_size_from_its_header() {
    assert_eq!(image_size(&png(30, 20)), Some((30., 20.)));
    assert_eq!(image_size(b"<svg/>"), None);
}

#[test]
fn text_shows_as_code_in_its_language() {
    let text = Text { lines: vec!["fn main() {}".into()] };
    let Still::Code(code) = described(Content::Text(text), Path::new("a.rs"), false) else { panic!("code") };
    assert_eq!((code.lines.len(), code.language), (1, Some("Rust")));
}

#[test]
fn an_image_shows_as_a_picture_with_its_size() {
    let image = Arc::new(Image::from_bytes(ImageFormat::Png, png(8, 4)));
    let Still::Picture(picture) = described(Content::Image(image), Path::new("a.png"), false) else {
        panic!("a picture")
    };
    assert_eq!((picture.size, picture.label.as_str()), (Some((8., 4.)), "PNG image"));
}

#[test]
fn another_machines_binary_file_says_it_has_no_preview() {
    let still = described(Content::Binary, Path::new("a.pdf"), true);
    assert_eq!(note_of(&still), Some("No preview from another machine"));
}

#[test]
fn what_cannot_be_read_says_why() {
    assert_eq!(note_of(&described(Content::TooLarge, Path::new("a.log"), true)), Some("Too large to preview"));
    assert_eq!(note_of(&described(Content::NotRegular, Path::new("fifo"), false)), Some("Not a regular file"));
    let gone = described(Content::Gone("No such file".into()), Path::new("a"), false);
    assert_eq!(note_of(&gone), Some("Cannot read this file"));
}
