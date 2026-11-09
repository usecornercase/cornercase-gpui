use super::*;

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
