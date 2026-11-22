use super::*;
use crate::panels::usage::data::{Counts, Merged, Window};

fn window(agents: u64, waited: f64) -> Window {
    Window { counts: Counts { agents, waited_seconds: waited, ..Counts::default() }, ..Window::default() }
}

fn stats() -> Stats {
    let mut stats = Stats::default();
    for (key, agents) in [("24h", 14), ("7d", 70), ("total", 1_284)] {
        stats.windows.insert(key.into(), window(agents, 600.));
    }
    stats
}

#[test]
fn the_tiles_end_on_waited_time_without_merged_pull_requests() {
    let labels: Vec<&str> = figures(&stats()).iter().map(|f| f.label).collect();
    assert_eq!(labels, ["agents started", "prompts", "tokens out", "waited for you"]);
}

#[test]
fn merged_pull_requests_take_the_last_tile_with_their_lower_bound() {
    let mut stats = stats();
    stats.merged_pull_requests = Some(Merged { day: 3, week: 19, total: 287, complete: false, ..Merged::default() });
    let last = figures(&stats).pop().expect("four tiles");
    assert_eq!((last.label, last.values, last.suffix), ("merged PRs", [3., 19., 287.], "+"));
}

#[test]
fn the_last_24_hours_are_compared_with_the_week() {
    let started = figures(&stats()).remove(0);
    assert_eq!(started.sub(0, 30).map(|v| v.words), Some("40% above your 7-day average".into()));
}

#[test]
fn a_longer_window_says_how_much_a_day() {
    let figures = figures(&stats());
    assert_eq!(figures[0].sub(1, 30).map(|v| v.words), Some("10 a day".into()));
    assert_eq!(figures[0].sub(2, 107).map(|v| v.words), Some("12 a day".into()));
    assert_eq!(figures[3].sub(1, 30).map(|v| v.words), Some("1m a day".into()));
}

#[test]
fn a_tile_counts_up_to_its_value() {
    assert_eq!([Shape::Count.show(1_284. * 0.5), Shape::Count.show(1_284.)], ["642", "1284"]);
    assert_eq!(Shape::Tokens.show(1_900_000.), "1.9M");
}

#[test]
fn an_incomplete_count_is_not_compared_and_its_daily_figure_is_a_lower_bound() {
    let mut stats = stats();
    stats.merged_pull_requests = Some(Merged { day: 3, week: 7, total: 287, complete: false, ..Merged::default() });
    let merged = figures(&stats).pop().expect("four tiles");
    assert_eq!(merged.sub(0, 30), None);
    assert_eq!(merged.sub(1, 30).map(|v| v.words), Some("1.0+ a day".into()));
}
