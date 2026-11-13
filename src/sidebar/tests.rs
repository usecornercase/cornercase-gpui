use gpui::{DefiniteLength, Length, Styled as _, px};

use super::{MARK, corner, mark_lead, rail, tabs_lead, title_strip};
use crate::app::GAP;
use crate::platform::Os;
use crate::ui::brand::mark_size;

#[test]
fn the_title_strip_is_pulled_and_padded_as_its_platform_needs() {
    for os in [Os::Mac, Os::Linux] {
        let (pull, lead) = mark_lead(os);
        let mut strip = title_strip(os);
        let style = strip.style();
        let pulled: Option<Length> = (pull != 0.).then(|| px(pull).into());
        let padded: Option<DefiniteLength> = Some(px(lead).into());
        assert!(style.margin.left == pulled, "{os:?} margin");
        assert!(style.padding.left == padded, "{os:?} padding");
    }
}

#[test]
fn on_a_mac_the_mark_clears_the_traffic_lights() {
    assert_eq!(mark_lead(Os::Mac), (0., 34.));
}

#[test]
fn on_linux_the_title_strip_reaches_over_the_rail_from_the_windows_edge() {
    let (pull, _) = mark_lead(Os::Linux);
    assert!((pull + rail::COLUMN).abs() < f32::EPSILON);
}

#[test]
fn on_linux_the_mark_sits_centred_over_the_rails_icons() {
    let (pull, lead) = mark_lead(Os::Linux);
    let centre = rail::COLUMN + pull + lead + mark_size(MARK) / 2.;
    let rail_centre = GAP + (rail::COLUMN - 2. * GAP) / 2.;
    assert!((centre - rail_centre).abs() < f32::EPSILON);
}

#[test]
fn with_the_sidebar_aside_the_tab_row_starts_where_the_mark_did_on_a_mac() {
    assert!((tabs_lead(Os::Mac) - mark_lead(Os::Mac).1).abs() < f32::EPSILON);
    assert!(tabs_lead(Os::Linux).abs() < f32::EPSILON);
    assert!(tabs_lead(Os::Windows).abs() < f32::EPSILON);
}

#[test]
fn with_the_sidebar_aside_the_mark_keeps_the_corner_off_a_mac() {
    assert!(corner(Os::Mac).is_none());
    assert!(corner(Os::Linux).is_some());
    assert!(corner(Os::Windows).is_some());
}
