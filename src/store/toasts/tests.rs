use std::time::{Duration, Instant};

use super::{Shown, Toast, ToastKind, drawn, push_toast};

fn toast(id: u64, text: &str) -> Toast {
    toast_of(id, text, ToastKind::Error)
}

fn toast_of(id: u64, text: &str, kind: ToastKind) -> Toast {
    Toast { id, text: text.into(), kind, action: None, shown: Shown::default(), count: 1 }
}

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

fn ids(toasts: &[Toast]) -> Vec<u64> {
    toasts.iter().map(|t| t.id).collect()
}

#[test]
fn a_repeated_toast_counts_instead_of_stacking() {
    let mut toasts = Vec::new();
    for id in 1..=3 {
        push_toast(&mut toasts, toast(id, "the server stopped"));
    }
    assert_eq!(toasts.len(), 1);
    assert_eq!((toasts[0].id, toasts[0].count), (3, 3));
}

#[test]
fn a_repeated_toast_moves_to_the_newest_place() {
    let mut toasts = Vec::new();
    push_toast(&mut toasts, toast(1, "first"));
    push_toast(&mut toasts, toast(2, "second"));
    push_toast(&mut toasts, toast(3, "first"));
    let texts: Vec<_> = toasts.iter().map(|t| t.text.as_str()).collect();
    assert_eq!(texts, ["second", "first"]);
}

#[test]
fn an_expired_toast_is_not_counted_again() {
    let long_ago = Instant::now().checked_sub(secs(60)).expect("a minute ago");
    let mut old = toast(1, "the server stopped");
    drawn(std::slice::from_mut(&mut old), &[1], long_ago);
    let mut toasts = vec![old];
    push_toast(&mut toasts, toast(2, "the server stopped"));
    assert_eq!(toasts.last().map(|t| t.count), Some(1));
}

#[test]
fn toasts_of_another_kind_stay_apart() {
    let mut toasts = Vec::new();
    push_toast(&mut toasts, toast(1, "Copied"));
    push_toast(&mut toasts, toast_of(2, "Copied", ToastKind::Info));
    assert_eq!(toasts.len(), 2);
}

#[test]
fn at_most_five_toasts_are_kept() {
    let mut toasts = Vec::new();
    for id in 1..=7 {
        push_toast(&mut toasts, toast(id, &id.to_string()));
    }
    assert_eq!(toasts.first().map(|t| t.id), Some(3));
}

#[test]
fn a_toast_never_drawn_does_not_expire() {
    let t0 = Instant::now();
    assert!(!toast(1, "a").expired_at(t0 + secs(3600)));
}

#[test]
fn a_toasts_seconds_count_only_while_it_is_drawn() {
    let t0 = Instant::now();
    let mut v = vec![toast_of(1, "a", ToastKind::Done)];
    drawn(&mut v, &[1], t0);
    drawn(&mut v, &[], t0 + secs(3));
    assert!(!v[0].expired_at(t0 + secs(60)));
    drawn(&mut v, &[1], t0 + secs(60));
    assert!(!v[0].expired_at(t0 + secs(71)));
    assert!(v[0].expired_at(t0 + secs(73)));
}

#[test]
fn each_kind_lasts_its_own_seconds() {
    let t0 = Instant::now();
    let kinds = [ToastKind::Info, ToastKind::Error, ToastKind::Done, ToastKind::NeedsYou];
    let mut v: Vec<_> = kinds.iter().zip(1..).map(|(k, id)| toast_of(id, "a", *k)).collect();
    drawn(&mut v, &[1, 2, 3, 4], t0);
    let expired = |at: u64| v.iter().map(|t| t.expired_at(t0 + secs(at))).collect::<Vec<_>>();
    assert_eq!(expired(5), [true, false, false, false]);
    assert_eq!(expired(9), [true, true, false, false]);
    assert_eq!(expired(16), [true, true, true, true]);
}

#[test]
fn a_toast_that_starts_showing_is_given_the_seconds_it_has_left() {
    let t0 = Instant::now();
    let mut v = vec![toast_of(1, "a", ToastKind::Done)];
    assert_eq!(drawn(&mut v, &[1], t0), [secs(15)]);
    drawn(&mut v, &[], t0 + secs(3));
    assert_eq!(drawn(&mut v, &[1], t0 + secs(10)), [secs(12)]);
}

#[test]
fn a_toast_still_drawn_is_not_given_its_seconds_twice() {
    let t0 = Instant::now();
    let mut v = vec![toast(1, "a")];
    drawn(&mut v, &[1], t0);
    assert_eq!(drawn(&mut v, &[1], t0 + secs(1)).len(), 0);
}

#[test]
fn a_repeat_counts_its_seconds_again() {
    let t0 = Instant::now();
    let mut v = vec![toast(1, "a")];
    drawn(&mut v, &[1], t0);
    push_toast(&mut v, toast(2, "a"));
    assert_eq!((v.len(), v[0].count, v[0].shown), (1, 2, Shown::default()));
}

#[test]
fn past_five_the_oldest_toast_already_drawn_makes_way() {
    let t0 = Instant::now();
    let mut v: Vec<_> = (1..=5).map(|id| toast(id, &id.to_string())).collect();
    drawn(&mut v, &[2, 3, 4, 5], t0);
    push_toast(&mut v, toast(6, "6"));
    assert_eq!(ids(&v), [1, 3, 4, 5, 6]);
}

#[test]
fn past_five_with_none_drawn_the_oldest_info_makes_way() {
    let mut v = vec![
        toast_of(1, "1", ToastKind::NeedsYou),
        toast_of(2, "2", ToastKind::Info),
        toast(3, "3"),
        toast(4, "4"),
        toast(5, "5"),
    ];
    push_toast(&mut v, toast(6, "6"));
    assert_eq!(ids(&v), [1, 3, 4, 5, 6]);
}

#[test]
fn the_new_toast_never_makes_way() {
    let mut v: Vec<_> = (1..=5).map(|id| toast(id, &id.to_string())).collect();
    push_toast(&mut v, toast_of(6, "6", ToastKind::Info));
    assert_eq!(ids(&v), [2, 3, 4, 5, 6]);
}
