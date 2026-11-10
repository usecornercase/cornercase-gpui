use super::{Check, Pill, Verdict};
use crate::panels::flows::draft::Saved;

fn problems(count: usize) -> Verdict {
    Verdict::Checked(Saved { problems: (0..count).map(|n| format!("problem {n}")).collect(), ..Saved::default() })
}

fn answered(verdict: Verdict) -> Check {
    let mut check = Check::default();
    let generation = check.changed();
    check.answer(generation, verdict);
    check
}

#[test]
fn the_pill_counts_the_problems() {
    let three = answered(problems(3));
    assert_eq!(three.pill(), Pill::Problems(3));
    assert_eq!(three.pill().words(), "3 problems");
    assert_eq!(answered(problems(1)).pill().words(), "1 problem");
    assert_eq!(answered(problems(0)).pill(), Pill::Ready);
    assert_eq!(Pill::Ready.words(), "Ready to save");
    assert_eq!(answered(Verdict::Invalid("x".into())).pill(), Pill::Problems(1));
}

#[test]
fn a_change_goes_back_to_checking() {
    let mut check = answered(problems(0));
    let old = check.changed();
    assert_eq!(check.pill(), Pill::Checking);
    let current = check.changed();
    check.answer(old, problems(0));
    assert_eq!(check.pill(), Pill::Checking);
    check.answer(current, problems(0));
    assert_eq!(check.pill(), Pill::Ready);
}

#[test]
fn save_rests_while_there_are_problems() {
    assert!(answered(problems(2)).rests());
    assert!(!answered(problems(0)).rests());
    assert!(answered(Verdict::Invalid("x".into())).rests());
    let mut check = answered(problems(2));
    check.changed();
    assert!(!check.rests());
    assert_eq!(check.problems().len(), 2);
}
