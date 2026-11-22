use super::*;
use crate::panels::usage::data::Block;

fn stats(seconds: &[f64]) -> Stats {
    Stats { blocks: seconds.iter().map(|&agent_seconds| Block { agent_seconds }).collect(), ..Stats::default() }
}

#[test]
fn the_chart_reads_the_blocks_oldest_first() {
    let days = stats(&[30., 20., 10.]).oldest_first();
    assert_eq!(days.len(), SHOWN);
    assert_eq!(&days[SHOWN - 3..], &[10., 20., 30.]);
    assert!(days[..SHOWN - 3].iter().all(|d| *d == 0.));
}

#[test]
fn the_streak_counts_back_from_the_newest_block() {
    assert_eq!(stats(&[5., 4., 3., 0., 9.]).streak(), 3);
    assert_eq!(stats(&[0., 4., 3.]).streak(), 0);
    assert_eq!(Stats::default().streak(), 0);
}

#[test]
fn the_best_block_is_the_newest_of_the_largest() {
    assert_eq!(stats(&[5., 9., 0., 9.]).best(), Some((1, 9.)));
    assert_eq!(stats(&[0., 0.]).best(), None);
    assert_eq!(stats(&[5., 9., 0., 9.]).active(), 3);
}

#[test]
fn a_negative_block_counts_as_no_work() {
    let negative = stats(&[-3., 2.]);
    assert_eq!((negative.seconds_ago(0), negative.streak(), negative.active()), (0., 0, 1));
}

#[test]
fn the_newest_block_is_named_the_last_24_hours() {
    assert_eq!(day_name(0, 1_800_000_000), "last 24 h");
    assert_eq!(day_tip(0, 5_400., 1_800_000_000), "last 24 h · 1h 30m");
    assert_eq!(day_tip(0, 0., 1_800_000_000), "last 24 h · no agent time");
}

#[test]
fn months_are_marked_where_they_change() {
    assert_eq!(month_marks(&[10, 10, 10, 11, 11, 12], 3), [(0, "Oct"), (3, "Nov"), (5, "Dec")]);
}

#[test]
fn a_month_with_no_room_before_the_next_is_not_marked() {
    assert_eq!(month_marks(&[10, 11, 11, 11], 3), [(1, "Nov")]);
    assert_eq!(month_marks(&[7, 7], 3), [(0, "Jul")]);
    assert_eq!(month_marks(&[], 3), []);
}

#[test]
fn the_average_trails_over_its_span() {
    assert_eq!(trailing_mean(&[2., 4., 6., 8.], 2), [2., 3., 5., 7.]);
    assert_eq!(trailing_mean(&[1., 2.], 0), [1., 2.]);
}

#[test]
fn the_scale_takes_at_most_four_round_steps() {
    assert_eq!(scale(31.3), (40., 10.));
    assert_eq!(scale(3.5), (4., 1.));
    assert_eq!(scale(0.), (1., 1.));
    assert_eq!(scale(150.), (150., 50.));
}

#[test]
fn the_gauge_ends_on_an_even_factor_of_at_least_four() {
    assert_eq!([gauge_top(0.), gauge_top(3.4), gauge_top(4.1), gauge_top(7.)], [4, 4, 6, 8]);
    assert_eq!(gauge_ticks(4), [0, 1, 2, 3, 4]);
    assert_eq!(gauge_ticks(8), [0, 2, 4, 6, 8]);
}

#[test]
fn a_day_is_compared_with_the_weeks_daily_average() {
    let above = versus(14., 70.).expect("an average");
    assert_eq!((above.words.as_str(), above.way), ("40% above your 7-day average", Ordering::Greater));
    let below = versus(5., 70.).expect("an average");
    assert_eq!((below.words.as_str(), below.way), ("50% below your 7-day average", Ordering::Less));
    assert_eq!(versus(10., 70.).map(|v| v.way), Some(Ordering::Equal));
    assert!(versus(3., 0.).is_none());
}

#[test]
fn a_daily_figure_keeps_one_decimal_below_ten() {
    assert_eq!([decimal(2.74), decimal(57.4), decimal(12_400.)], ["2.7", "57", "12.4k"]);
    assert!((per_day(70., 7) - 10.).abs() < 1e-9);
    assert!((per_day(5., 0) - 5.).abs() < 1e-9);
}

#[test]
fn a_step_is_placed_along_its_line() {
    assert_eq!([along(0, 4), along(2, 4), along(4, 4), along(9, 4), along(3, 0)], [0., 0.5, 1., 1., 0.]);
}
