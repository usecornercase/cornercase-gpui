use super::*;

const ROW: f32 = 20.;

fn at(lines: &[(usize, usize)], y: f32) -> Option<(usize, usize, f32)> {
    line_at(lines, px(ROW), px(y)).map(|(ix, start, top)| (ix, start, f32::from(top)))
}

#[test]
fn a_point_in_the_first_line_picks_it() {
    assert_eq!(at(&[(10, 1), (5, 1)], 4.), Some((0, 0, 0.)));
}

#[test]
fn a_point_in_a_wrapped_lines_second_row_stays_in_that_line() {
    assert_eq!(at(&[(80, 3), (5, 1)], 45.), Some((0, 0, 0.)));
}

#[test]
fn a_point_in_a_later_line_counts_the_breaks_before_it() {
    assert_eq!(at(&[(80, 3), (5, 1), (7, 2)], 85.), Some((2, 87, 80.)));
}

#[test]
fn a_point_below_the_last_line_picks_the_last_line() {
    assert_eq!(at(&[(10, 1), (5, 1)], 500.), Some((1, 11, 20.)));
}

#[test]
fn an_empty_line_between_two_takes_one_row() {
    let lines = [(4, 1), (0, 1), (6, 1)];
    assert_eq!(at(&lines, 25.), Some((1, 5, 20.)));
    assert_eq!(at(&lines, 45.), Some((2, 6, 40.)));
}

#[test]
fn no_line_picks_nothing() {
    assert_eq!(at(&[], 5.), None);
}
