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
fn nothing_to_undo_without_a_number() {
    assert_eq!(Removal::from_answer(&json!({ "entries": [] }).to_string()), None);
    assert_eq!(Removal::from_answer("4028901700366324\n"), None);
}
