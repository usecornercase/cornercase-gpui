use super::*;

fn link(lines: Option<(usize, usize)>) -> Link {
    let lines = lines.map(|(first, last)| format!(r#","lines":{{"first":{first},"last":{last}}}"#));
    let json = format!(r#"{{"link":{{"path":"/r/a.rs","row":0,"start":0,"end":4{}}}}}"#, lines.unwrap_or_default());
    Link::parse(&json).expect("a link")
}

#[test]
fn one_line_or_a_range_is_named() {
    assert_eq!(lines_label(&link(Some((12, 12)))), Some("line 12".to_string()));
    assert_eq!(lines_label(&link(Some((12, 30)))), Some("lines 12–30".to_string()));
    assert_eq!(lines_label(&link(None)), None);
}

#[test]
fn a_time_reads_as_minutes_and_seconds() {
    assert_eq!(clock(0.4), "0:00");
    assert_eq!(clock(75.9), "1:15");
    assert_eq!(clock(f64::NAN), "0:00");
    assert_eq!(clock(-3.), "0:00");
}

#[test]
fn the_folder_is_what_comes_before_the_name() {
    assert_eq!(folder_of("src/panels/files.rs"), "src/panels");
    assert_eq!(folder_of("notes.md"), "");
}
