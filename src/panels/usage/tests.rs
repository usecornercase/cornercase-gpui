use super::Load;

fn refused() -> anyhow::Result<u32> {
    Err(anyhow::anyhow!("no stats from `vps`: not read yet"))
}

#[test]
fn a_cached_answer_shows_while_the_fresh_one_comes() {
    let mut load = Load { loading: true, ..Load::default() };
    load.cached(Ok(1));
    assert_eq!((load.value, load.loading), (Some(1), true));
}

#[test]
fn a_cached_answer_does_not_count_as_read() {
    let mut load = Load::default();
    load.cached(Ok(1));
    assert_eq!(load.at, None);
}

#[test]
fn a_fresh_answer_replaces_the_cached_one() {
    let mut load = Load::default();
    load.cached(Ok(1));
    load.done(Ok(2));
    assert_eq!(load.value, Some(2));
}

#[test]
fn a_cached_answer_after_the_fresh_one_is_dropped() {
    let mut load = Load::default();
    load.done(Ok(2));
    load.cached(Ok(1));
    assert_eq!(load.value, Some(2));
}

#[test]
fn a_cached_refusal_leaves_the_reading_waiting_without_an_error() {
    let mut load = Load { loading: true, ..Load::default() };
    load.cached(refused());
    assert_eq!((load.value, load.error, load.loading), (None, None, true));
}

#[test]
fn a_fresh_failure_keeps_the_cached_numbers_and_says_why() {
    let mut load = Load::default();
    load.cached(Ok(1));
    load.done(refused());
    assert_eq!(load.value, Some(1));
    assert!(load.error.is_some());
}

#[test]
fn a_cached_answer_after_a_fresh_failure_still_shows() {
    let mut load = Load::default();
    load.done(refused());
    load.cached(Ok(1));
    assert_eq!(load.value, Some(1));
    assert!(load.error.is_some());
}
