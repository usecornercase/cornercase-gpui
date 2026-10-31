use super::Stopped;

#[test]
fn a_stopped_row_waits_for_a_sample_begun_after_it() {
    let mut stopped = Stopped::default();
    stopped.began();
    stopped.mark("stop-process:7");
    stopped.ended();
    assert!(stopped.holds("stop-process:7"));
    stopped.began();
    stopped.ended();
    assert!(!stopped.holds("stop-process:7"));
}

#[test]
fn a_stop_between_samples_clears_with_the_next_one() {
    let mut stopped = Stopped::default();
    stopped.began();
    stopped.ended();
    stopped.mark("stop-port:3000-9");
    stopped.began();
    stopped.ended();
    assert!(!stopped.holds("stop-port:3000-9"));
}
