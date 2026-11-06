use super::{Query, files_label};
use crate::panels::changes::wire::FileInfo;

fn file(path: &str) -> FileInfo {
    FileInfo { path: path.to_string(), ..FileInfo::default() }
}

fn kept<'a>(query: &str, paths: &[&'a str]) -> Vec<&'a str> {
    let query = Query::parse(query);
    paths.iter().copied().filter(|path| query.keeps(&file(path))).collect()
}

#[test]
fn an_empty_query_keeps_every_file() {
    assert!(Query::parse("   ").is_empty());
    assert_eq!(kept("", &["a.rs", "b/c.rs"]), ["a.rs", "b/c.rs"]);
}

#[test]
fn a_word_is_a_piece_of_the_path() {
    assert_eq!(
        kept("order", &["src/order/mod.rs", "src/reorder.rs", "src/cart.rs"]),
        ["src/order/mod.rs", "src/reorder.rs"]
    );
}

#[test]
fn a_file_stays_when_any_word_keeps_it() {
    assert_eq!(kept("cart order", &["cart.rs", "order.rs", "user.rs"]), ["cart.rs", "order.rs"]);
}

#[test]
fn a_bang_leaves_files_out() {
    assert_eq!(kept("!*.snap !*.lock", &["a.snap", "Cargo.lock", "a.rs"]), ["a.rs"]);
    assert_eq!(kept("src !test", &["src/a.rs", "src/a_test.rs", "docs/a.md"]), ["src/a.rs"]);
}

#[test]
fn a_lone_bang_is_looked_for_as_it_is() {
    assert!(!Query::parse("!").is_empty());
    assert_eq!(kept("!", &["a!.rs", "b.rs"]), ["a!.rs"]);
}

#[test]
fn lower_case_ignores_case_and_a_capital_asks_for_it() {
    assert_eq!(kept("readme", &["README.md", "readme.txt"]), ["README.md", "readme.txt"]);
    assert_eq!(kept("README", &["README.md", "readme.txt"]), ["README.md"]);
    assert_eq!(kept("*.MD", &["a.MD", "b.md"]), ["a.MD"]);
}

#[test]
fn a_glob_without_a_slash_matches_any_part_of_the_path() {
    assert_eq!(kept("*.test.js", &["a.test.js", "src/b.test.js", "c.js"]), ["a.test.js", "src/b.test.js"]);
    assert_eq!(kept("sr?", &["src/a.rs", "a/sr/b.rs", "a/srcs"]), ["src/a.rs"]);
}

#[test]
fn a_glob_with_a_slash_matches_the_whole_path_and_what_is_under_it() {
    assert_eq!(kept("src/*.rs", &["src/a.rs", "src/b/c.rs", "x/src/a.rs"]), ["src/a.rs"]);
    assert_eq!(
        kept("src/api/**", &["src/api/a.rs", "src/api/b/c.rs", "src/web/a.rs"]),
        ["src/api/a.rs", "src/api/b/c.rs"]
    );
    assert_eq!(kept("/sr?", &["src/a.rs", "lib/src/a.rs"]), ["src/a.rs"]);
}

#[test]
fn two_stars_cross_folders_and_may_stand_for_none() {
    assert_eq!(
        kept("**/fixtures/*.json", &["fixtures/a.json", "tests/fixtures/b.json", "tests/c.json"]),
        ["fixtures/a.json", "tests/fixtures/b.json"]
    );
}

#[test]
fn a_trailing_slash_matches_only_folders() {
    assert_eq!(kept("buil?/", &["build/a.o", "lib/build", "builds"]), ["build/a.o"]);
}

#[test]
fn a_renamed_file_is_found_by_its_old_path_too() {
    let renamed = FileInfo { old_path: Some("old/name.rs".into()), ..file("new/name.rs") };
    assert!(Query::parse("old/").keeps(&renamed));
    assert!(!Query::parse("!old/").keeps(&renamed));
}

#[test]
fn the_summary_says_how_many_are_left() {
    assert_eq!(files_label(3, 31, true), "3 of 31 files");
    assert_eq!(files_label(31, 31, false), "31 files");
    assert_eq!(files_label(1, 1, false), "1 file");
}
