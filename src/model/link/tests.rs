use super::*;
use crate::model::{Run, Style};
use crate::test_util::fixture;

const INSIDE: &str = fixture!("link/inside.json");

const OUTSIDE: &str = r#"{"pane":4,"link":{"path":"/home/me/plan.md","inside":false,"workspace":2,
    "row":1,"start":0,"end":9}}"#;

fn link(json: &str) -> Link {
    Link::parse(json).expect("a link")
}

fn pane_showing(rows: &[&[&str]]) -> PaneGrid {
    let lines = rows
        .iter()
        .map(|cells| {
            let cells = cells.iter().map(|c| (*c).to_string()).collect();
            vec![Run { style: Style::default(), link: None, cells }]
        })
        .collect();
    PaneGrid { lines, ..PaneGrid::default() }
}

#[test]
fn reads_what_a_click_on_a_cell_opens() {
    let link = link(INSIDE);
    assert_eq!((link.path.as_str(), link.inside, link.workspace), ("/Users/me/shop/src/main.rs", true, Some(36)));
    assert_eq!((link.row, link.start, link.end), (0, 0, 19));
    assert_eq!(link.lines, Some(Lines { first: 120, last: 130 }));
}

#[test]
fn a_cell_on_no_file_reads_as_none() {
    assert_eq!(Link::parse(fixture!("link/none.json")), None);
    assert_eq!(Link::parse("not json"), None);
}

#[test]
fn an_empty_span_reads_as_none() {
    assert_eq!(Link::parse(r#"{"pane":4,"link":{"path":"/a","row":0,"start":3,"end":3}}"#), None);
}

#[test]
fn the_span_covers_its_cells_and_not_the_one_after() {
    let link = link(INSIDE);
    assert!(link.span().covers((0, 0)) && link.span().covers((18, 0)));
    assert!(!link.span().covers((19, 0)) && !link.span().covers((10, 1)));
}

#[test]
fn a_file_inside_the_workspace_shows_its_relative_path() {
    assert_eq!(link(INSIDE).shown_path(), "src/main.rs");
    assert_eq!(link(OUTSIDE).shown_path(), "/home/me/plan.md");
}

#[test]
fn lines_are_counted_from_zero() {
    let inside = link(INSIDE);
    assert_eq!((inside.first_line(), inside.last_line()), (Some(119), Some(129)));
    assert_eq!(link(OUTSIDE).first_line(), None);
}

#[test]
fn the_name_is_the_last_part_of_the_path() {
    assert_eq!(link(OUTSIDE).name(), "plan.md");
}

#[test]
fn the_same_file_at_other_lines_is_another_peek() {
    let (a, mut b) = (link(INSIDE), link(INSIDE));
    b.start = 0;
    assert!(a.same_file(&b));
    b.lines = None;
    assert!(!a.same_file(&b));
}

#[test]
fn a_row_reads_as_its_cells_with_wide_characters_whole() {
    let pane = pane_showing(&[&["a", "界", "", "b"]]);
    assert_eq!(text_between(&row_cells(&pane, 0), 0, 4), "a界b");
    assert_eq!(row_cells(&pane, 3), Vec::<&str>::new());
}

#[test]
fn a_span_still_shows_its_text_until_the_row_changes() {
    let pane = pane_showing(&[&["x", " ", "a", ".", "r", "s"]]);
    assert!(shows(&pane, (0, 2, 6), "a.rs"));
    assert!(!shows(&pane, (0, 2, 6), "b.rs"));
    assert!(!shows(&pane, (1, 2, 6), "a.rs"));
}

fn cells(text: &str) -> Vec<String> {
    text.chars().map(String::from).collect()
}

fn url_of(text: &str) -> Option<String> {
    let owned = cells(text);
    let cells: Vec<&str> = owned.iter().map(String::as_str).collect();
    url_in(&cells, 0, cells.len()).map(|(from, to)| text_between(&cells, from, to))
}

#[test]
fn a_web_address_needs_its_scheme_and_a_host() {
    assert!(is_web("https://example.com") && is_web("http://localhost:3000"));
    assert!(!is_web("https://") && !is_web("ftp://example.com") && !is_web("example.com"));
}

#[test]
fn a_word_that_holds_an_address_names_it() {
    assert_eq!(url_of("http://localhost:5173/").as_deref(), Some("http://localhost:5173/"));
    assert_eq!(url_of("url=https://a.dev/x?y=1#z").as_deref(), Some("https://a.dev/x?y=1#z"));
    assert_eq!(url_of("src/main.rs:12"), None);
    assert_eq!(url_of("https://"), None);
}

#[test]
fn the_punctuation_that_ends_a_sentence_is_left_out() {
    assert_eq!(url_of("https://a.dev/docs.").as_deref(), Some("https://a.dev/docs"));
    assert_eq!(url_of("https://a.dev:").as_deref(), Some("https://a.dev"));
    assert_eq!(url_of("https://a.dev/?").as_deref(), Some("https://a.dev/"));
}

fn hyperlinked(runs: &[(&str, Option<&str>)]) -> PaneGrid {
    let line = runs
        .iter()
        .map(|(text, link)| Run { style: Style::default(), link: link.map(Into::into), cells: cells(text) })
        .collect();
    PaneGrid { lines: vec![line], ..PaneGrid::default() }
}

#[test]
fn a_hyperlink_spans_its_runs_even_across_blanks() {
    let docs = Some("https://a.dev/docs");
    let pane = hyperlinked(&[("see ", None), ("the ", docs), ("docs", docs), (" now", None)]);
    let (span, url) = web_hyperlink(&pane, (9, 0)).expect("a hyperlink");
    assert_eq!((span.row, span.start, span.end, url.as_str()), (0, 4, 12, "https://a.dev/docs"));
    assert_eq!(web_hyperlink(&pane, (2, 0)), None);
}

#[test]
fn a_hyperlink_to_a_file_is_no_web_address() {
    let pane = hyperlinked(&[("plan.md", Some("file:///home/me/plan.md"))]);
    assert_eq!(web_hyperlink(&pane, (0, 0)), None);
}

#[test]
fn a_web_address_still_shows_until_its_text_changes() {
    let pane = hyperlinked(&[("go https://a.dev", None)]);
    let span = Span { row: 0, start: 3, end: 16 };
    let hovered = Hovered::new(&pane, span, Target::Web("https://a.dev".into()));
    assert_eq!(hovered.text, "https://a.dev");
    assert!(hovered.at(0, (3, 0)) && !hovered.at(0, (2, 0)));
}

#[test]
fn an_address_of_this_machines_own_servers_is_loopback() {
    for url in ["http://localhost:5173/", "http://127.0.0.1:8000", "http://[::1]:3000/x", "http://app.localhost/"] {
        assert!(is_loopback(url), "{url}");
    }
    for url in ["https://example.com/localhost", "https://localhost.example.com/", "file:///tmp/x"] {
        assert!(!is_loopback(url), "{url}");
    }
}
