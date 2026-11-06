use super::{Section, body_height, columns, width};
use crate::keymap::Group;
use crate::platform::Os;

#[test]
fn every_section_shows_once_on_each_platform() {
    let mut all: Vec<Section> = Group::ALL.into_iter().map(Section::Keys).collect();
    all.extend([Section::System, Section::Elsewhere]);
    for os in [Os::Mac, Os::Linux, Os::Windows] {
        let shown: Vec<Section> = columns(os).concat();
        assert_eq!(shown.len(), all.len(), "{os:?}");
        assert!(all.iter().all(|s| shown.contains(s)), "{os:?}");
    }
}

#[test]
fn on_linux_window_and_tabs_swap_columns_to_make_room_for_its_longer_everywhere_else() {
    assert_eq!(columns(Os::Mac)[2][0], Section::Keys(Group::TabsAndPanes));
    assert_eq!(columns(Os::Linux)[2][0], Section::Keys(Group::Window));
}

#[test]
fn linux_takes_a_wider_dialog_for_its_longer_keys_within_the_window() {
    assert!((width(Os::Mac, 1280.) - 940.).abs() < f32::EPSILON);
    assert!((width(Os::Linux, 1280.) - 1120.).abs() < f32::EPSILON);
    assert!((width(Os::Linux, 900.) - 836.).abs() < f32::EPSILON);
    assert!((width(Os::Mac, 400.) - 480.).abs() < f32::EPSILON);
}

#[test]
fn the_list_grows_with_the_window_and_scrolls_past_it() {
    assert!((body_height(800.) - 640.).abs() < f32::EPSILON);
    assert!((body_height(1000.) - 840.).abs() < f32::EPSILON);
    assert!((body_height(300.) - 200.).abs() < f32::EPSILON);
}
