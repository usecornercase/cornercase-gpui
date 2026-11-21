use super::*;
use crate::ui::highlight::Kind;

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
    let Block::Code(code) = &blocks[6] else { panic!("{:?}", blocks[6]) };
    assert_eq!((code.lang(), code.text()), ("rust", "fn main() {}"));
    assert_eq!(code_kinds(code)[..2], [("fn".to_string(), Kind::Keyword), ("main".to_string(), Kind::Function)]);
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

#[test]
fn a_changelog_section_reads_as_items_with_their_marks() {
    let blocks = parse("- **Each update says what it updates.** It reads `↑ cornercase 0.13.19` now.\n- Another.");
    let [Block::Item(0, bullet, first), Block::Item(0, _, second)] = blocks.as_slice() else { panic!("{blocks:?}") };
    assert_eq!((bullet.as_str(), second.text.as_str()), ("•", "Another."));
    assert_eq!(first.text, "Each update says what it updates. It reads ↑ cornercase 0.13.19 now.");
    let at = |s: &str| first.text.find(s).map(|i| i..i + s.len()).expect("in the text");
    assert!(first.marks.contains(&(at("Each update says what it updates."), Mark::Bold)));
    assert!(first.marks.contains(&(at("↑ cornercase 0.13.19"), Mark::Code)));
}

fn code_kinds(code: &Code) -> Vec<(String, Kind)> {
    code.spans().iter().map(|(range, kind)| (code.text()[range.clone()].to_string(), *kind)).collect()
}

fn only_code(source: &str) -> Code {
    let blocks = parse(source);
    let [Block::Code(code)] = blocks.as_slice() else { panic!("{blocks:?}") };
    code.clone()
}

#[test]
fn a_block_with_no_language_or_an_unknown_one_stays_plain() {
    for source in ["```\nfn a() {}\n```", "```nope\nfn a() {}\n```", "    fn a() {}"] {
        let code = only_code(source);
        assert_eq!(code.text(), "fn a() {}", "{source}");
        assert!(code.spans().is_empty(), "{source}");
    }
}

#[test]
fn an_unclosed_fence_takes_the_rest_and_is_coloured() {
    let code = only_code("```rust\nfn a() {}\n\nlet b = 1;");
    assert_eq!(code.text(), "fn a() {}\n\nlet b = 1;");
    let kinds = code_kinds(&code);
    assert!(kinds.contains(&("fn".to_string(), Kind::Keyword)));
    assert!(kinds.contains(&("let".to_string(), Kind::Keyword)));
}

#[test]
fn a_longer_fence_and_tildes_name_a_language() {
    assert_eq!(only_code("````ts\nconst a = 1;\n````").lang(), "ts");
    let tildes = only_code("~~~ sh\necho hi\n~~~");
    assert_eq!(tildes.lang(), "sh");
    assert!(code_kinds(&tildes).contains(&("echo".to_string(), Kind::Function)));
}

#[test]
fn outline_reads_the_same_blocks_without_spans() {
    let painted = parse(SAMPLE);
    let outlined = outline(SAMPLE);
    assert_eq!(painted.len(), outlined.len());
    let Block::Code(code) = &outlined[6] else { panic!("{:?}", outlined[6]) };
    assert_eq!(code.text(), "fn main() {}");
    assert_eq!(code.spans(), []);
    assert_eq!(painted[..6], outlined[..6]);
}

#[test]
fn each_kind_of_block_picks_the_text_it_draws() {
    let blocks = parse(SAMPLE);
    let picked: Vec<Option<String>> =
        blocks.iter().map(|block| block.picked_text().map(Cow::into_owned)).collect();
    assert_eq!(picked[0].as_deref(), Some("Problem"));
    assert_eq!(picked[1].as_deref(), Some("A plugin's popup (#435) runs a program."));
    assert_eq!(picked[2].as_deref(), Some("one more"));
    assert_eq!(picked[5].as_deref(), Some("quoted text"));
    assert_eq!(picked[6].as_deref(), Some("fn main() {}"));
    assert_eq!((picked[7].as_deref(), picked[8].as_deref()), (None, None));
}

#[test]
fn a_code_blocks_picked_text_has_four_spaces_for_a_tab() {
    let blocks = parse("```\nif x {\n\treturn;\n}\n```");
    assert_eq!(blocks[0].picked_text().as_deref(), Some("if x {\n    return;\n}"));
}

#[test]
fn a_paragraph_with_a_link_picks_its_words_and_not_its_address() {
    let blocks = parse("See [the docs](https://example.com/docs) now.");
    assert_eq!(blocks[0].picked_text().as_deref(), Some("See the docs now."));
}
