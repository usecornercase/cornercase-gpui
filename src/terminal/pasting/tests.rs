use std::path::{Path, PathBuf};

use gpui::ImageFormat;

use super::{file_args, image_args, paths_text, shell_word};

#[test]
fn a_pasted_image_goes_on_standard_input_with_a_name_of_its_kind() {
    assert_eq!(image_args(12, ImageFormat::Png), ["paste-file", "--pane", "12", "--name", "pasted.png", "-"]);
    assert_eq!(image_args(12, ImageFormat::Jpeg)[4], "pasted.jpg");
}

#[test]
fn a_dropped_file_is_read_here_by_paste_file() {
    let args = file_args(7, Path::new("/Users/me/Desktop/Screenshot 2026.png"));
    assert_eq!(args, ["paste-file", "--pane", "7", "/Users/me/Desktop/Screenshot 2026.png"]);
}

#[test]
fn a_plain_path_is_pasted_bare() {
    assert_eq!(shell_word("/tmp/a-b_c.d/e+f"), "/tmp/a-b_c.d/e+f");
}

#[test]
fn a_path_with_spaces_or_quotes_is_quoted_for_the_shell() {
    assert_eq!(shell_word("/tmp/my shot.png"), "'/tmp/my shot.png'");
    assert_eq!(shell_word("/tmp/it's"), r"'/tmp/it'\''s'");
    assert_eq!(shell_word(""), "''");
}

#[test]
fn dropped_paths_are_pasted_each_followed_by_a_space() {
    let paths = [PathBuf::from("/tmp/a"), PathBuf::from("/tmp/b c")];
    assert_eq!(paths_text(&paths), "/tmp/a '/tmp/b c' ");
}
