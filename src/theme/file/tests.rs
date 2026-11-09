use super::*;

#[test]
fn a_theme_of_one_line_is_a_theme() {
    let file = parse(r#"{"extends": "nord"}"#).expect("theme");
    assert_eq!(file.extends.as_deref(), Some("nord"));
    assert!(file.chrome.is_empty());
}

#[test]
fn keys_this_version_does_not_know_survive_a_round_trip() {
    let file = parse(r##"{"name": "x", "fancy": {"a": 1}, "chrome": {"later": "#fff"}}"##).expect("theme");
    let again = parse(&file.text()).expect("theme");
    assert_eq!(again.other.get("fancy"), file.other.get("fancy"));
    assert!(again.chrome.contains_key("later"));
}

#[test]
fn a_file_that_is_not_json_says_where() {
    let error = parse("{\n  \"name\": \n}").expect_err("broken");
    assert!(format!("{error:#}").contains("line 3"), "{error:#}");
}
