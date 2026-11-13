use super::fact_separators;
use crate::ui::Separator::Dot;

#[test]
fn every_fact_but_the_first_has_a_dot_before_it_and_a_tag_has_none() {
    assert_eq!(fact_separators(3, 2), vec![None, Some(Dot), Some(Dot), None, None]);
    assert_eq!(fact_separators(1, 0), vec![None]);
    assert_eq!(fact_separators(0, 2), vec![None, None]);
}
