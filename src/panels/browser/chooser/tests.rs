use serde_json::json;

use super::*;

#[test]
fn a_single_file_input_asks_for_one_file() {
    let params = json!({ "frameId": "F", "mode": "selectSingle", "backendNodeId": 6 });
    assert_eq!(chooser(&params), Some((6, false)));
}

#[test]
fn a_multiple_file_input_asks_for_several() {
    let params = json!({ "frameId": "F", "mode": "selectMultiple", "backendNodeId": 10 });
    assert_eq!(chooser(&params), Some((10, true)));
}

#[test]
fn a_chooser_without_its_input_is_ignored() {
    assert_eq!(chooser(&json!({ "mode": "selectSingle" })), None);
}
