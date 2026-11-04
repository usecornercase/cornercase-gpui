use super::{MARK, mark_lead, rail};
use crate::app::GAP;
use crate::platform::Os;
use crate::ui::brand::mark_size;

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
