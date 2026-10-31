use std::time::{Duration, Instant};

use super::{Toast, ToastKind, push_toast};

fn toast(id: u64, text: &str) -> Toast {
    Toast { id, text: text.into(), kind: ToastKind::Error, action: None, at: Instant::now(), count: 1 }
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
    let mut toasts = Vec::new();
    let mut old = toast(1, "the server stopped");
    old.at = Instant::now().checked_sub(Duration::from_secs(60)).expect("a minute ago");
    push_toast(&mut toasts, old);
    push_toast(&mut toasts, toast(2, "the server stopped"));
    assert_eq!(toasts.last().map(|t| t.count), Some(1));
}

#[test]
fn toasts_of_another_kind_stay_apart() {
    let mut toasts = Vec::new();
    push_toast(&mut toasts, toast(1, "Copied"));
    push_toast(&mut toasts, Toast { kind: ToastKind::Info, ..toast(2, "Copied") });
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
