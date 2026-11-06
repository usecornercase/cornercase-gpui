use std::collections::BTreeMap;

use super::{Places, blocks, label};
use crate::panels::issues::markdown::{Block, Inline, Mark};
use crate::panels::recap::data::{Answer, Named, Place};
use crate::test_util::fixture;

fn places() -> Places {
    let named = |label: &str| Some(Named { label: label.to_string(), path: format!("/home/ana/code/{label}") });
    let conversation = Place {
        kind: "conversation".to_string(),
        project: named("shop"),
        workspace: named("main"),
        tab: Some("claude".to_string()),
        ..Place::default()
    };
    let pull = Place { kind: "pull".to_string(), number: Some(212), ..Place::default() };
    BTreeMap::from([("c1".to_string(), conversation), ("p1".to_string(), pull)])
}

fn paragraph(text: &str) -> Inline {
    match blocks(text, &places()).remove(0) {
        Block::Paragraph(inline) => inline,
        other => panic!("{other:?}"),
    }
}

fn shown<'a>(inline: &'a Inline, target: &str) -> &'a str {
    let (range, _) = inline.links.iter().find(|(_, to)| to == target).expect("a link");
    &inline.text[range.clone()]
}

#[test]
fn a_conversation_becomes_a_link_named_for_its_place() {
    let inline = paragraph("[c1] asked");
    assert_eq!(inline.text, "shop › main › claude asked");
    assert_eq!(shown(&inline, "c1"), "shop › main › claude");
}

#[test]
fn a_pull_request_becomes_a_link_named_for_its_number() {
    let inline = paragraph("see [p1].");
    assert_eq!(inline.text, "see #212.");
    assert_eq!(shown(&inline, "p1"), "#212");
}

#[test]
fn a_link_is_drawn_as_a_link() {
    let inline = paragraph("[p1]");
    assert_eq!(inline.marks, [(0..4, Mark::Link)]);
}

#[test]
fn an_id_the_answer_does_not_place_stays_as_it_is() {
    let inline = paragraph("[c9] and [other] stay");
    assert_eq!(inline.text, "[c9] and [other] stay");
    assert_eq!(inline.links, Vec::new());
}

#[test]
fn emphasis_around_an_id_covers_its_name() {
    let inline = paragraph("**[p1]**");
    assert_eq!(inline.text, "#212");
    assert!(inline.marks.contains(&(0..4, Mark::Bold)));
    assert!(inline.marks.contains(&(0..4, Mark::Link)));
}

#[test]
fn emphasis_after_an_id_moves_with_the_text() {
    let inline = paragraph("[c1] says **yes**");
    let bold = inline.marks.iter().find(|(_, mark)| *mark == Mark::Bold).map(|(range, _)| range.clone());
    assert_eq!(&inline.text[bold.expect("bold")], "yes");
}

#[test]
fn emphasis_before_an_id_stays_where_it_is() {
    let inline = paragraph("**yes** to [p1]");
    let bold = inline.marks.iter().find(|(_, mark)| *mark == Mark::Bold).map(|(range, _)| range.clone());
    assert_eq!(&inline.text[bold.expect("bold")], "yes");
}

#[test]
fn a_web_link_keeps_its_address_after_an_id_moves_it() {
    let inline = paragraph("[c1] see the [notes](https://example.com/notes)");
    assert_eq!(shown(&inline, "https://example.com/notes"), "notes");
    assert_eq!(shown(&inline, "c1"), "shop › main › claude");
}

#[test]
fn two_ids_in_a_row_are_two_links() {
    let inline = paragraph("[c1] and [p1] and [p1]");
    assert_eq!(inline.text, "shop › main › claude and #212 and #212");
    assert_eq!(inline.links.len(), 3);
}

#[test]
fn an_id_followed_by_a_bracket_is_not_read_as_a_markdown_link() {
    let inline = paragraph("[c1]([p1])");
    assert_eq!(inline.text, "shop › main › claude (#212)");
    assert_eq!(inline.links.len(), 2);
}

#[test]
fn two_ids_written_together_do_not_run_into_each_other() {
    assert_eq!(paragraph("[c1][p1]").text, "shop › main › claude #212");
}

#[test]
fn an_id_in_a_heading_a_bullet_and_a_quote_is_a_link_there_too() {
    let read = blocks("## [p1]\n- [c1]\n\n> [p1]", &places());
    let [Block::Heading(2, heading), Block::Item(0, _, item), Block::Quote(quote)] = read.as_slice() else {
        panic!("{read:?}");
    };
    assert_eq!(shown(heading, "p1"), "#212");
    assert_eq!(shown(item, "c1"), "shop › main › claude");
    assert_eq!(shown(quote, "p1"), "#212");
}

#[test]
fn an_id_in_inline_code_is_a_link_with_its_code_mark() {
    let inline = paragraph("`[p1]`");
    assert!(inline.marks.contains(&(0..4, Mark::Code)));
    assert_eq!(shown(&inline, "p1"), "#212");
}

#[test]
fn a_code_block_and_a_table_name_the_id_without_a_link() {
    let read = blocks("```\nsee [p1]\n```\n| [c1] | [p1] |\n|---|---|", &places());
    assert_eq!(read[0], Block::Code("see #212".to_string()));
    assert_eq!(read[1], Block::Table(vec![vec!["shop › main › claude".to_string(), "#212".to_string()]]));
}

#[test]
fn a_place_with_no_names_is_called_by_its_id() {
    assert_eq!(label("c7", &Place::default()), "c7");
}

#[test]
fn a_pull_request_without_a_number_is_called_by_its_place() {
    let pull = Place { kind: "pull".to_string(), tab: Some("claude".to_string()), ..Place::default() };
    assert_eq!(label("p7", &pull), "claude");
}

#[test]
fn every_id_of_a_real_answer_is_named() {
    let answer: Answer = serde_json::from_str(fixture!("recap/now.json")).expect("an answer");
    let read = blocks(answer.text.as_deref().expect("a text"), &answer.places);
    let Block::Item(_, _, first) = &read[1] else { panic!("{:?}", read[1]) };
    assert_eq!(shown(first, "c3"), "shop › main › claude");
    assert_eq!(shown(first, "p1"), "#212");
    let Block::Item(_, _, unknown) = &read[2] else { panic!("{:?}", read[2]) };
    assert!(unknown.text.starts_with("[c9]"), "{}", unknown.text);
}
