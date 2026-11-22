use super::*;

fn last(kind: &str, text: &str) -> Last {
    Last { kind: kind.into(), text: text.into(), at: 1_700_000_000 }
}

#[test]
fn an_answer_previews_its_first_line_without_its_marks() {
    let answer = last("bot", "\n## **Three** PRs wait\n\nThe first is `#12`.");
    assert_eq!(says(Some(&answer), (false, false)), Says::Line("Three PRs wait".into()));
}

#[test]
fn your_own_message_is_marked_as_yours() {
    assert_eq!(
        says(Some(&last("you", "which are blocked?")), (false, false)),
        Says::Line("You: which are blocked?".into())
    );
}

#[test]
fn a_list_item_loses_its_marker_and_box() {
    assert_eq!(says(Some(&last("bot", "- [ ] review #4")), (false, false)), Says::Line("review #4".into()));
}

#[test]
fn a_long_line_is_cut_with_an_ellipsis() {
    let long = "word ".repeat(60);
    let Says::Line(shown) = says(Some(&last("bot", &long)), (false, false)) else { panic!("a line") };
    assert!(shown.ends_with('…') && shown.chars().count() <= PREVIEW_MAX + 1);
}

#[test]
fn a_bot_that_asks_for_a_secret_says_so_before_anything_else() {
    assert_eq!(says(Some(&last("bot", "hi")), (true, true)), Says::Asks);
}

#[test]
fn a_thinking_bot_says_it_thinks() {
    assert_eq!(says(Some(&last("bot", "hi")), (false, true)), Says::Thinking);
}

#[test]
fn a_bot_with_no_line_or_only_blank_text_says_nothing() {
    assert_eq!(says(None, (false, false)), Says::Nothing);
    assert_eq!(says(Some(&last("bot", "  \n ")), (false, false)), Says::Nothing);
}

#[test]
fn a_last_line_reads_leniently() {
    let read = |value: Value| last_or_none(value).ok().flatten();
    assert_eq!(read(Value::Null), None);
    assert_eq!(read(serde_json::json!("text")), None);
    assert_eq!(
        read(serde_json::json!({"kind": "bot", "text": "hi", "at": 5})),
        Some(Last { kind: "bot".into(), text: "hi".into(), at: 5 })
    );
}

#[test]
fn a_bot_keeps_its_face_colour() {
    assert_eq!(face_colour("prs"), face_colour("prs"));
    assert!(FACE_COLOURS.contains(&face_colour("news")));
}

#[test]
fn bots_get_different_face_colours() {
    let ids = ["prs", "news", "ops", "univ", "gestor", "issues"];
    let colours: std::collections::HashSet<u8> = ids.iter().map(|id| face_colour(id)).collect();
    assert!(colours.len() > 3);
}
