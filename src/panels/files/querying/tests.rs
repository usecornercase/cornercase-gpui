use super::*;

#[test]
fn text_rows_list_files_then_lines() {
    let hit = |line| search::LineHit { line, text: String::new(), lit: Vec::new() };
    let results = search::TextResults {
        files: vec![
            search::FileHits { path: "a".into(), lines: vec![hit(1), hit(2)] },
            search::FileHits { path: "b".into(), lines: vec![hit(3)] },
        ],
        capped: false,
    };
    let rows = text_rows(&results);
    assert_eq!(rows.len(), 5);
    assert!(matches!(rows[3], TextRow::File(1)));
    assert!(matches!(rows[4], TextRow::Line(1, 0)));
}
