use std::sync::atomic::AtomicBool;

use super::super::search;
use crate::ui::selection::Fit;

use super::{Hit, all_hits, count, first_from, on_line, stepped};

fn lines(text: &[&str]) -> Vec<String> {
    text.iter().map(ToString::to_string).collect()
}

fn found(query: &str, text: &[&str]) -> Vec<Hit> {
    let matcher = search::matcher(query).expect("a query");
    all_hits(&matcher, &lines(text), &AtomicBool::new(false)).expect("not cancelled")
}

fn hit(line: usize, start: usize, end: usize) -> Hit {
    Hit::new(line, start..end)
}

#[test]
fn every_match_of_every_line_is_found_in_order() {
    assert_eq!(found("ab", &["ab ab", "x", "cab"]), vec![hit(0, 0, 2), hit(0, 3, 5), hit(2, 1, 3)]);
}

#[test]
fn a_query_in_lower_case_ignores_case() {
    assert_eq!(found("total", &["Total", "TOTAL"]).len(), 2);
}

#[test]
fn a_query_with_an_upper_case_letter_keeps_case() {
    assert_eq!(found("Total", &["total", "Total"]), vec![hit(1, 0, 5)]);
}

#[test]
fn a_query_is_a_literal_not_a_pattern() {
    assert_eq!(found("a.c", &["abc", "a.c"]), vec![hit(1, 0, 3)]);
}

#[test]
fn a_cancelled_search_gives_nothing() {
    let matcher = search::matcher("a").expect("a query");
    assert_eq!(all_hits(&matcher, &lines(&["a"]), &AtomicBool::new(true)), None);
}

#[test]
fn matching_starts_at_the_first_hit_on_or_after_a_line() {
    let hits = [hit(2, 0, 1), hit(5, 0, 1), hit(5, 3, 4), hit(9, 0, 1)];
    assert_eq!(first_from(&hits, 0), Some(0));
    assert_eq!(first_from(&hits, 3), Some(1));
    assert_eq!(first_from(&hits, 5), Some(1));
}

#[test]
fn matching_past_the_last_hit_wraps_to_the_first() {
    assert_eq!(first_from(&[hit(2, 0, 1)], 3), Some(0));
}

#[test]
fn no_hits_have_no_first() {
    assert_eq!(first_from(&[], 3), None);
}

#[test]
fn stepping_forward_wraps_at_the_end() {
    assert_eq!(stepped(Some(0), 3, false), Some(1));
    assert_eq!(stepped(Some(2), 3, false), Some(0));
}

#[test]
fn stepping_back_wraps_at_the_start() {
    assert_eq!(stepped(Some(1), 3, true), Some(0));
    assert_eq!(stepped(Some(0), 3, true), Some(2));
}

#[test]
fn stepping_with_no_current_hit_goes_to_an_end() {
    assert_eq!(stepped(None, 3, false), Some(0));
    assert_eq!(stepped(None, 3, true), Some(2));
}

#[test]
fn stepping_through_no_hits_stays_on_none() {
    assert_eq!(stepped(None, 0, false), None);
    assert_eq!(stepped(Some(4), 0, true), None);
}

#[test]
fn a_drawn_line_finds_only_its_own_hits() {
    let hits = [hit(2, 0, 1), hit(5, 0, 1), hit(5, 3, 4), hit(9, 0, 1)];
    assert_eq!(on_line(&hits, 5, 4096), &hits[1..3]);
    assert_eq!(on_line(&hits, 6, 4096), &[]);
}

#[test]
fn hits_far_past_the_edge_of_a_huge_line_are_not_drawn() {
    let hits = [hit(1, 0, 1), hit(1, 4_000, 4_001), hit(1, 500_000, 500_001), hit(2, 0, 1)];
    assert_eq!(on_line(&hits, 1, 4096), &hits[..2]);
}

#[test]
fn a_wrapped_line_lights_its_hits_as_far_as_it_is_drawn() {
    let hits = [hit(1, 0, 1), hit(1, 9_000, 9_001), hit(1, 500_000, 500_001)];
    assert_eq!(on_line(&hits, 1, Fit::Wrap { columns: 100 }.lit_bytes()), &hits[..2]);
}

#[test]
fn the_count_says_which_hit_of_how_many() {
    assert_eq!(count(Some(2), 12), "3 of 12");
}

#[test]
fn the_count_says_when_nothing_matches() {
    assert_eq!(count(None, 0), "No matches");
}

#[test]
fn a_stale_hit_cut_inside_a_character_is_not_drawn() {
    assert_eq!(hit(0, 1, 2).range_in("ñb"), None);
    assert_eq!(hit(0, 0, 9).range_in("ñb"), None);
    assert_eq!(hit(0, 0, 2).range_in("ñb"), Some(0..2));
}
