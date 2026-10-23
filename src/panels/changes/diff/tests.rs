use super::*;
use crate::panels::changes::wire::{GapLine, Line};

fn line(kind: LineKind, old: Option<u32>, new: Option<u32>, text: &str) -> Line {
    Line { kind, old, new, text: text.into(), ..Line::default() }
}

fn hunk(old_start: u32, new_start: u32, lines: Vec<Line>) -> Hunk {
    Hunk { old_start, new_start, context: String::new(), lines }
}

fn sample() -> Vec<Hunk> {
    vec![
        hunk(
            3,
            3,
            vec![
                line(LineKind::Context, Some(3), Some(3), "a"),
                line(LineKind::Removed, Some(4), None, "b"),
                line(LineKind::Added, None, Some(4), "B"),
                line(LineKind::Added, None, Some(5), "C"),
                line(LineKind::Context, Some(5), Some(6), "d"),
            ],
        ),
        hunk(
            10,
            11,
            vec![line(LineKind::Context, Some(10), Some(11), "x"), line(LineKind::Removed, Some(11), None, "y")],
        ),
    ]
}

#[test]
fn counts_the_unchanged_lines_between_hunks() {
    let hunks = sample();
    assert_eq!(gap_before(&hunks, 0), 2);
    assert_eq!(gap_before(&hunks, 1), 4);
    assert_eq!(gap_before(&hunks, 2), 0);
    assert_eq!(gap_before(&[hunk(1, 0, vec![line(LineKind::Removed, Some(1), None, "z")])], 0), 0);
}

#[test]
fn lists_rows_with_gaps_folded_or_read() {
    let hunks = sample();
    let mut gaps = HashMap::new();
    let rows_folded = rows(&hunks, &gaps);
    assert_eq!(rows_folded[0], Row::Hunk(0));
    assert_eq!(rows_folded[6], Row::Gap { hunk: 1, lines: 4 });
    assert_eq!(rows_folded.len(), 1 + 5 + 1 + 1 + 2);
    let gap_line = |n| GapLine { old: n, new: n, text: "u".into(), syntax: None };
    gaps.insert(1, Gap { lines: vec![gap_line(7), gap_line(8)], ..Gap::default() });
    let rows_read = rows(&hunks, &gaps);
    assert_eq!(&rows_read[6..9], &[Row::GapLine { hunk: 1, line: 0 }, Row::GapLine { hunk: 1, line: 1 }, Row::Hunk(1)]);
}

#[test]
fn writes_a_hunk_as_a_patch() {
    let mut h = sample().remove(0);
    h.context = "fn main()".into();
    assert_eq!(header(&h), "@@ -3,3 +3,4 @@ fn main()");
    assert_eq!(patch(&h), "@@ -3,3 +3,4 @@ fn main()\n a\n-b\n+B\n+C\n d\n");
}

#[test]
fn references_the_changed_lines() {
    let hunks = sample();
    assert_eq!(changed(&hunks[0]), (4, 5));
    assert_eq!(changed(&hunks[1]), (12, 12));
    assert_eq!(reference("src/a.rs", (4, 5)), "src/a.rs:4-5 ");
    assert_eq!(reference("src/a.rs", (12, 12)), "src/a.rs:12 ");
}

#[test]
fn quotes_the_path_for_the_editor() {
    assert_eq!(editor_command("src/a b.rs", 7), "${VISUAL:-${EDITOR:-vi}} +7 'src/a b.rs'");
    assert_eq!(editor_command("it's.txt", 1), r"${VISUAL:-${EDITOR:-vi}} +1 'it'\''s.txt'");
}

#[test]
fn asks_in_cornercase_words() {
    let file = |status, old: Option<&str>| FileInfo {
        path: "new.rs".into(),
        old_path: old.map(Into::into),
        status,
        ..FileInfo::default()
    };
    assert!(discard_question(&file(FileStatus::Untracked, None)).starts_with("Delete new.rs?"));
    assert!(discard_question(&file(FileStatus::Added, None)).contains("It is new, so it is deleted"));
    assert!(discard_question(&file(FileStatus::Renamed, Some("old.rs"))).contains("old.rs → new.rs"));
    assert_eq!(
        discard_question(&file(FileStatus::Modified, None)),
        "Discard the changes to new.rs? This cannot be undone."
    );
}

fn seg(style: usize, text: &str) -> Segment {
    Segment { style, text: text.into() }
}

#[test]
fn runs_follow_the_highlighting() {
    let text = "let x = 1;";
    let syntax = [seg(1, "let"), seg(0, " x = "), seg(4, "1"), seg(0, ";")];
    let r = runs(text, Some(&syntax), &[]);
    assert_eq!(r.len(), 4);
    assert_eq!((r[0].range.clone(), r[0].style), (0..3, 1));
    assert_eq!((r[2].range.clone(), r[2].style), (8..9, 4));
    assert!(r.iter().all(|r| !r.emphasis));
}

#[test]
fn runs_are_cut_by_the_emphasis() {
    let text = "    let y = 2;";
    let syntax = [seg(0, "    "), seg(1, "let"), seg(0, " y = "), seg(4, "2"), seg(0, ";")];
    let r = runs(text, Some(&syntax), &[[8, 9], [12, 13]]);
    let emphasised: Vec<_> = r.iter().filter(|r| r.emphasis).map(|r| &text[r.range.clone()]).collect();
    assert_eq!(emphasised, vec!["y", "2"]);
    let joined: String = r.iter().map(|r| &text[r.range.clone()]).collect();
    assert_eq!(joined, text);
    assert!(r.windows(2).all(|w| w[0].range.end == w[1].range.start));
}

#[test]
fn runs_count_characters_not_bytes() {
    let text = "é → ü";
    let r = runs(text, None, &[[4, 5]]);
    let emphasised: Vec<_> = r.iter().filter(|r| r.emphasis).map(|r| &text[r.range.clone()]).collect();
    assert_eq!(emphasised, vec!["ü"]);
    assert_eq!(r.first().map(|r| r.style), Some(0));
}

#[test]
fn runs_ignore_highlighting_that_does_not_match() {
    let r = runs("abc", Some(&[seg(2, "xyz")]), &[]);
    assert_eq!(r, vec![Run { range: 0..3, style: 0, emphasis: false }]);
    assert_eq!(runs("", None, &[]), Vec::new());
}
