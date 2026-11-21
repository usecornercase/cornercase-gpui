use super::*;
use crate::ui::highlight::Kind;

fn texts(code: &Code) -> Vec<(String, Kind)> {
    let shown = code.shown().unwrap_or_default();
    code.spans().iter().map(|(range, kind)| (shown[range.clone()].to_string(), *kind)).collect()
}

#[test]
fn a_code_block_is_one_text_that_keeps_blank_lines() {
    assert_eq!(code_text("fn main() {\n\n\tlet a = 1;\n}\n").as_deref(), Some("fn main() {\n\n    let a = 1;\n}"));
}

#[test]
fn a_blank_code_block_keeps_its_one_line() {
    assert_eq!(code_text("\n").as_deref(), Some(""));
}

#[test]
fn an_empty_code_block_has_no_line() {
    assert_eq!(code_text(""), None);
}

#[test]
fn a_fences_language_is_its_first_word() {
    assert_eq!(language("rust"), "rust");
    assert_eq!(language("Rust,no_run"), "rust");
    assert_eq!(language(" sh"), "sh");
    assert_eq!(language("ts title=\"a\""), "ts");
    assert_eq!(language("{.python}"), "python");
    assert_eq!(language("toml:Cargo.toml"), "toml");
    assert_eq!(language(""), "");
}

#[test]
fn tabs_become_spaces_before_the_spans_are_cut() {
    let code = Code::new("rust", "\tlet a = 1;");
    assert_eq!(code.text(), "\tlet a = 1;");
    assert!(code.spans().contains(&(4..7, Kind::Keyword)));
}

#[test]
fn a_changed_text_is_highlighted_again() {
    let mut code = Code::new("rust", "let a = 1;");
    code.retext("fn b() {}");
    assert_eq!(code.text(), "fn b() {}");
    assert_eq!(code.lang(), "rust");
    assert!(texts(&code).contains(&("fn".to_string(), Kind::Keyword)));
    assert!(!texts(&code).iter().any(|(text, _)| text == "let"));
}

#[test]
fn the_same_text_keeps_its_spans() {
    let mut code = Code::new("rust", "let a = 1;");
    let before = code.clone();
    code.retext("let a = 1;");
    assert_eq!(code, before);
}

#[test]
fn a_plain_block_has_no_spans() {
    let code = Code::plain("let a = 1;");
    assert_eq!(code.spans(), []);
    assert_eq!(code.lang(), "");
}
