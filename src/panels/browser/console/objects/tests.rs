use serde_json::json;

use super::*;

fn string(text: &str) -> Value {
    json!({ "type": "string", "value": text })
}

fn number(n: f64) -> Value {
    json!({ "type": "number", "value": n, "description": n.to_string() })
}

#[test]
fn plain_values_read_as_javascript_prints_them() {
    let args = [
        string("a"),
        json!({ "type": "undefined" }),
        json!({ "type": "object", "subtype": "null", "value": null }),
        json!({ "type": "boolean", "value": true }),
        json!({ "type": "number", "unserializableValue": "NaN", "description": "NaN" }),
        json!({ "type": "bigint", "unserializableValue": "10n", "description": "10n" }),
    ];
    assert_eq!(formatted(&args), "a undefined null true NaN 10n");
}

#[test]
fn an_array_reads_as_its_items() {
    let array = json!({ "type": "object", "subtype": "array", "description": "Array(3)", "preview": {
        "subtype": "array", "description": "Array(3)", "overflow": true,
        "properties": [{ "name": "0", "type": "number", "value": "1" }, { "name": "1", "type": "string", "value": "x" }]
    }});
    assert_eq!(shown(&array), "[1, \"x\", …]");
}

#[test]
fn an_instance_of_a_class_names_it() {
    let object = json!({ "type": "object", "className": "User", "description": "User", "preview": {
        "description": "User", "overflow": false, "properties": [{ "name": "id", "type": "number", "value": "7" }]
    }});
    assert_eq!(shown(&object), "User {id: 7}");
}

#[test]
fn an_object_without_a_preview_reads_as_its_description() {
    assert_eq!(shown(&json!({ "type": "object", "subtype": "node", "description": "div#app" })), "div#app");
}

#[test]
fn a_function_reads_as_its_first_line() {
    assert_eq!(
        shown(&json!({ "type": "function", "description": "function go() {\n  return 1;\n}" })),
        "function go() {"
    );
}

#[test]
fn integers_drop_their_fraction() {
    assert_eq!(
        formatted(&[string("%d items, %i left, %f done"), number(3.7), number(-2.2), number(0.5)]),
        "3 items, -2 left, 0.5 done"
    );
}

#[test]
fn a_styled_message_drops_its_css() {
    assert_eq!(formatted(&[string("%cbold%c plain"), string("font-weight: bold"), string("")]), "bold plain");
}

#[test]
fn a_specifier_without_an_argument_stays_as_written() {
    assert_eq!(formatted(&[string("100% sure, %s")]), "100% sure, %s");
}

#[test]
fn a_double_percent_is_one_percent() {
    assert_eq!(formatted(&[string("50%% of %s"), string("it")]), "50% of it");
}

#[test]
fn arguments_left_over_follow_the_message() {
    assert_eq!(formatted(&[string("%s:"), string("a"), number(1.0)]), "a: 1");
}

#[test]
fn a_first_argument_that_is_not_a_string_is_shown_like_the_rest() {
    assert_eq!(formatted(&[number(1.0), string("%s")]), "1 %s");
}
