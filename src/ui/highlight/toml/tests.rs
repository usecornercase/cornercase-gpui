use super::*;

fn read(source: &str) -> Vec<Vec<(String, Kind)>> {
    let lines: Vec<&str> = source.split('\n').collect();
    spans(&lines)
        .iter()
        .zip(&lines)
        .map(|(spans, line)| spans.iter().map(|(range, kind)| (line[range.clone()].to_string(), *kind)).collect())
        .collect()
}

fn pairs(list: &[(&str, Kind)]) -> Vec<(String, Kind)> {
    list.iter().map(|(text, kind)| ((*text).to_string(), *kind)).collect()
}

#[test]
fn a_table_and_an_array_of_tables_are_headings() {
    let lines = read("[package]\n  [[bin]] # one\n[a.\"b c\"]");
    assert_eq!(lines[0], pairs(&[("[package]", Kind::Function)]));
    assert_eq!(lines[1], pairs(&[("[[bin]]", Kind::Function), ("# one", Kind::Comment)]));
    assert_eq!(lines[2], pairs(&[("[a.\"b c\"]", Kind::Function)]));
}

#[test]
fn bare_quoted_and_dotted_keys_are_tags() {
    let lines = read("name = 1\n\"a b\" = 2\nsite.page = 3\n'lit' = 4");
    assert_eq!(lines[0], pairs(&[("name", Kind::Tag), ("1", Kind::Number)]));
    assert_eq!(lines[1], pairs(&[("\"a b\"", Kind::Tag), ("2", Kind::Number)]));
    assert_eq!(lines[2], pairs(&[("site.page", Kind::Tag), ("3", Kind::Number)]));
    assert_eq!(lines[3], pairs(&[("'lit'", Kind::Tag), ("4", Kind::Number)]));
}

#[test]
fn a_hash_inside_a_string_is_no_comment() {
    let lines = read("a = \"x # y\" # z");
    assert_eq!(lines[0], pairs(&[("a", Kind::Tag), ("\"x # y\"", Kind::String), ("# z", Kind::Comment)]));
}

#[test]
fn an_escaped_quote_stays_in_its_string() {
    let lines = read(r#"a = "say \"hi\"" # c"#);
    assert_eq!(lines[0], pairs(&[("a", Kind::Tag), (r#""say \"hi\"""#, Kind::String), ("# c", Kind::Comment)]));
}

#[test]
fn a_multi_line_string_carries_across_lines() {
    let lines = read("a = \"\"\"one\ntwo # not a comment\nthree\"\"\" # c\nb = 1");
    assert_eq!(lines[0], pairs(&[("a", Kind::Tag), ("\"\"\"one", Kind::String)]));
    assert_eq!(lines[1], pairs(&[("two # not a comment", Kind::String)]));
    assert_eq!(lines[2], pairs(&[("three\"\"\"", Kind::String), ("# c", Kind::Comment)]));
    assert_eq!(lines[3], pairs(&[("b", Kind::Tag), ("1", Kind::Number)]));
}

#[test]
fn dates_floats_and_signed_numbers_are_numbers() {
    let lines = read("d = 1979-05-27T07:32:00Z\nf = 3.14\nn = -17\np = +inf\nq = nan\nt = false");
    let values: Vec<String> = lines.iter().filter_map(|line| line.get(1)).map(|(text, _)| text.clone()).collect();
    assert_eq!(values, ["1979-05-27T07:32:00Z", "3.14", "-17", "+inf", "nan", "false"]);
    assert!(lines.iter().all(|line| line.get(1).is_some_and(|(_, kind)| *kind == Kind::Number)));
}

#[test]
fn an_inline_tables_keys_are_tags() {
    let lines = read("dep = { version = \"1\", features = [\"a\", \"b\"], optional = true }");
    assert_eq!(
        lines[0],
        pairs(&[
            ("dep", Kind::Tag),
            ("version", Kind::Tag),
            ("\"1\"", Kind::String),
            ("features", Kind::Tag),
            ("\"a\"", Kind::String),
            ("\"b\"", Kind::String),
            ("optional", Kind::Tag),
            ("true", Kind::Number),
        ])
    );
}

#[test]
fn an_array_across_lines_keeps_its_values() {
    let lines = read("list = [\n  \"a\",\n  2,\n]\nnext = 1");
    assert_eq!(lines[1], pairs(&[("\"a\"", Kind::String)]));
    assert_eq!(lines[2], pairs(&[("2", Kind::Number)]));
    assert_eq!(lines[4], pairs(&[("next", Kind::Tag), ("1", Kind::Number)]));
}

#[test]
fn a_line_that_is_no_toml_stays_plain() {
    let lines = read("this is just words\n[unclosed");
    assert_eq!(lines[0], pairs(&[]));
    assert_eq!(lines[1], pairs(&[]));
}
