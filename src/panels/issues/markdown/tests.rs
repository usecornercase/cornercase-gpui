use super::*;

const SAMPLE: &str = "## Problem\n\nA plugin's popup (#435) runs\na program.\n\n- one\n  more\n- [x] done\n1. first\n\n> quoted\n> text\n\n```rust\nfn main() {}\n```\n---\n| a | b |\n|---|---|\n| 1 | 2 |\n<!-- hidden -->";

#[test]
fn blocks() {
    let blocks = parse(SAMPLE);
    assert_eq!(blocks.len(), 9);
    check_text_blocks(&blocks);
    check_other_blocks(&blocks);
}

fn check_text_blocks(blocks: &[Block]) {
    assert_eq!(blocks[0], Block::Heading(2, inline("Problem")));
    assert_eq!(blocks[1], Block::Paragraph(inline("A plugin's popup (#435) runs a program.")));
    let Block::Item(0, marker, text) = &blocks[2] else { panic!("{:?}", blocks[2]) };
    assert_eq!((marker.as_str(), text.text.as_str()), ("•", "one more"));
    assert!(matches!(&blocks[3], Block::Item(0, m, t) if m == "☑" && t.text == "done"));
    assert!(matches!(&blocks[4], Block::Item(0, m, t) if m == "1." && t.text == "first"));
}

fn check_other_blocks(blocks: &[Block]) {
    assert!(matches!(&blocks[5], Block::Quote(t) if t.text == "quoted text"));
    assert_eq!(blocks[6], Block::Code("fn main() {}".into()));
    assert_eq!(blocks[7], Block::Rule);
    assert_eq!(blocks[8], Block::Table(vec![vec!["a".into(), "b".into()], vec!["1".into(), "2".into()]]));
}

#[test]
fn setext_heading() {
    assert_eq!(parse("Title\n=====\nbody"), vec![Block::Heading(1, inline("Title")), Block::Paragraph(inline("body"))]);
}

#[test]
fn emphasis_code_and_links() {
    let line = inline("Use **bold**, *it*, `surface --view` and [the doc](https://x.dev/a) for snake_case_names");
    assert_eq!(line.text, "Use bold, it, surface --view and the doc for snake_case_names");
    let at = |s: &str| line.text.find(s).map(|i| i..i + s.len()).expect("in the text");
    assert!(line.marks.contains(&(at("bold"), Mark::Bold)));
    let italic = at("it,");
    assert!(line.marks.contains(&(italic.start..italic.end - 1, Mark::Italic)));
    assert!(line.marks.contains(&(at("surface --view"), Mark::Code)));
    assert_eq!(line.links, vec![(at("the doc"), "https://x.dev/a".to_string())]);
    assert_eq!(line.marks.iter().filter(|(_, m)| *m == Mark::Italic).count(), 1);
}

#[test]
fn bare_urls_images_and_html() {
    let line = inline("See https://github.com/a/b/issues/1. ![shot](https://i.png) <br>end <b>x</b>");
    assert_eq!(line.text, "See https://github.com/a/b/issues/1. [image: shot]  end x");
    assert_eq!(line.links[0].1, "https://github.com/a/b/issues/1");
    assert_eq!(line.links[1].1, "https://i.png");
}

#[test]
fn unmatched_markers_stay() {
    assert_eq!(inline("2 * 3 = 6").text, "2 * 3 = 6");
    assert_eq!(inline("a ` b").text, "a ` b");
    assert_eq!(inline(r"\*not\*").text, "*not*");
    let open = inline("a *b c and **d** e");
    assert_eq!(open.text, "a *b c and d e");
    let d = open.text.find(" d ").map(|i| i + 1..i + 2);
    assert_eq!(open.marks, vec![(d.unwrap_or_default(), Mark::Bold)]);
}

#[test]
fn overlapping_marks_become_runs() {
    let line = inline("x **bold [link](https://a) more** y");
    assert_eq!(line.text, "x bold link more y");
    let runs = line.runs();
    let texts: Vec<(&str, Vec<Mark>)> = runs.iter().map(|(r, m)| (&line.text[r.clone()], m.clone())).collect();
    assert_eq!(
        texts,
        vec![("bold ", vec![Mark::Bold]), ("link", vec![Mark::Bold, Mark::Link]), (" more", vec![Mark::Bold])]
    );
}

#[test]
fn multibyte_text() {
    let line = inline("héllo **wörld** ✓");
    assert_eq!(line.text, "héllo wörld ✓");
    let start = line.text.find("wörld").expect("in the text");
    assert_eq!(line.marks, vec![(start..start + "wörld".len(), Mark::Bold)]);
}
