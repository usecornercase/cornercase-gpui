use super::saved_label;

const NOON: i64 = 1_794_744_000;

#[test]
fn a_secret_saved_today_says_so_with_its_time() {
    let label = saved_label(NOON, NOON).expect("a label");
    assert!(label.starts_with("saved today "), "{label}");
}

#[test]
fn a_secret_saved_on_another_day_says_which() {
    let label = saved_label(NOON - 40 * 86_400, NOON).expect("a label");
    assert!(label.starts_with("saved ") && !label.contains("today"), "{label}");
}

#[test]
fn a_time_that_does_not_read_says_nothing() {
    assert_eq!(saved_label(0, NOON), None);
    assert_eq!(saved_label(-5, NOON), None);
}
