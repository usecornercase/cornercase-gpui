use gpui::MouseButton::{Left, Middle, Navigate, Right};
use gpui::NavigationDirection;

use super::{Press, buttons, file_of, press_for};
use crate::platform::Os;

#[test]
fn a_file_link_names_its_path_whatever_its_host() {
    assert_eq!(file_of("file:///home/me/a.rs").as_deref(), Some("/home/me/a.rs"));
    assert_eq!(file_of("file://devbox/home/me/a.rs").as_deref(), Some("/home/me/a.rs"));
    assert_eq!(file_of("file:/home/me/a.rs").as_deref(), Some("/home/me/a.rs"));
}

#[test]
fn a_file_link_reads_its_escapes() {
    assert_eq!(file_of("file:///home/me/my%20notes%2Fold.md").as_deref(), Some("/home/me/my notes/old.md"));
    assert_eq!(file_of("file:///home/me/caf%C3%A9.txt").as_deref(), Some("/home/me/café.txt"));
    assert_eq!(file_of("file:///home/me/100%.txt").as_deref(), Some("/home/me/100%.txt"));
    assert_eq!(file_of("file:///home/me/a%+1").as_deref(), Some("/home/me/a%+1"));
}

#[test]
fn a_file_link_to_a_drive_names_a_windows_path() {
    assert_eq!(file_of("file:///C:/Users/me/a.rs").as_deref(), Some(r"C:\Users\me\a.rs"));
    assert_eq!(file_of("file://devbox/D:/my%20notes/a.md").as_deref(), Some(r"D:\my notes\a.md"));
    assert_eq!(file_of("file:///C:").as_deref(), Some("/C:"));
}

#[test]
fn only_a_file_link_names_a_path() {
    assert_eq!(file_of("https://example.com/a"), None);
    assert_eq!(file_of("file://devbox"), None);
    assert_eq!(file_of("file:relative"), None);
}

#[test]
fn the_middle_button_is_heard_only_where_it_pastes_the_primary_selection() {
    assert_eq!(buttons(Os::Mac), [Left, Right]);
    assert_eq!(buttons(Os::Linux), [Left, Middle, Right]);
}

#[test]
fn a_program_that_reads_the_mouse_gets_the_press_unless_shift_is_held() {
    assert_eq!(press_for(Left, true, false), Press::Program);
    assert_eq!(press_for(Middle, true, false), Press::Program);
    assert_eq!(press_for(Left, true, true), Press::Select);
    assert_eq!(press_for(Middle, true, true), Press::PastePrimary);
}

#[test]
fn otherwise_the_left_button_selects_and_the_middle_one_pastes() {
    assert_eq!(press_for(Left, false, false), Press::Select);
    assert_eq!(press_for(Middle, false, false), Press::PastePrimary);
    assert_eq!(press_for(Navigate(NavigationDirection::Back), false, false), Press::Nothing);
}

#[test]
fn a_right_press_asks_for_the_menu_unless_the_program_reads_the_mouse() {
    assert_eq!(press_for(Right, false, false), Press::Menu);
    assert_eq!(press_for(Right, true, true), Press::Menu);
    assert_eq!(press_for(Right, true, false), Press::Program);
}
