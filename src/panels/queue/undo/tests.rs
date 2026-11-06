use serde_json::json;

use super::Removal;

#[test]
fn a_removal_keeps_the_number_restore_takes() {
    let out = json!({
        "entries": [{ "id": 4, "label": "gpui-undo-check", "state": "pending", "zone": "Europe/Madrid" }],
        "undo": 265_413_034_352_970_u64,
    });
    let removal = Removal::from_answer(&out.to_string()).expect("a removal");
    assert_eq!((removal.undo, removal.text.as_str()), (265_413_034_352_970, "Removed “gpui-undo-check”"));
}

#[test]
fn clearing_started_entries_counts_them() {
    let out = json!({ "entries": [{ "id": 1, "label": "a" }, { "id": 2, "label": "b" }], "undo": 7 });
    let removal = Removal::from_answer(&out.to_string()).expect("a removal");
    assert_eq!(removal.text, "Removed 2 started entries");
}

#[test]
fn removing_a_routine_says_that_its_runs_go_with_it() {
    let routine = json!({ "id": 1, "label": "deps", "repeat": "day 09:00" });
    let run = |id: u64| json!({ "id": id, "label": "deps", "routine": 1, "state": "started" });
    let text = |entries: serde_json::Value| {
        let out = json!({ "entries": entries, "undo": 7 });
        Removal::from_answer(&out.to_string()).expect("a removal").text
    };
    assert_eq!(text(json!([routine])), "Removed “deps”");
    assert_eq!(text(json!([run(2), routine])), "Removed “deps” and its run");
    assert_eq!(text(json!([run(2), run(3), routine])), "Removed “deps” and its 2 runs");
}

#[test]
fn removing_a_run_or_clearing_runs_says_so() {
    let run = |id: u64| json!({ "id": id, "label": "deps", "routine": 1, "state": "kept" });
    let one = json!({ "entries": [run(2)], "undo": 7 });
    assert_eq!(Removal::from_answer(&one.to_string()).expect("a removal").text, "Removed a run of “deps”");
    let two = json!({ "entries": [run(2), run(3)], "undo": 7 });
    assert_eq!(Removal::from_answer(&two.to_string()).expect("a removal").text, "Removed 2 finished entries");
}

#[test]
fn nothing_to_undo_without_a_number() {
    assert_eq!(Removal::from_answer(&json!({ "entries": [] }).to_string()), None);
    assert_eq!(Removal::from_answer("4028901700366324\n"), None);
}
