//! A small Markdown reader for issue bodies and comments: enough to read them as formatted text
//! (headings, paragraphs, lists, task boxes, quotes, code, tables, rules, emphasis, inline code
//! and links), not a full CommonMark parser. Unknown syntax stays as its text.

use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Bold,
    Italic,
    Code,
    Strike,
    Link,
}

/// A line of text with its marks and the links in it (byte ranges into `text`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Inline {
    pub text: String,
    pub marks: Vec<(Range<usize>, Mark)>,
    pub links: Vec<(Range<usize>, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Heading(u8, Inline),
    Paragraph(Inline),
    /// A list item: its depth (0 for the outer list), its marker (`•`, `3.`, `☐`, `☑`) and text.
    Item(usize, String, Inline),
    Quote(Inline),
    Code(String),
    /// A table's rows, cells already split.
    Table(Vec<Vec<String>>),
    Rule,
}

pub fn parse(source: &str) -> Vec<Block> {
    let source = strip_comments(&source.replace("\r\n", "\n"));
    let mut blocks = Vec::new();
    let mut paragraph: Vec<&str> = Vec::new();
    let mut lines = source.lines().peekable();
    let flush = |paragraph: &mut Vec<&str>, blocks: &mut Vec<Block>| {
        if !paragraph.is_empty() {
            blocks.push(Block::Paragraph(inline(&paragraph.join(" "))));
            paragraph.clear();
        }
    };
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        // Fenced code.
        if let Some(fence) = ["```", "~~~"].into_iter().find(|f| trimmed.starts_with(f)) {
            flush(&mut paragraph, &mut blocks);
            let mut code = Vec::new();
            for next in lines.by_ref() {
                if next.trim_start().starts_with(fence) {
                    break;
                }
                code.push(next.get(indent.min(next.len() - next.trim_start().len())..).unwrap_or(next));
            }
            blocks.push(Block::Code(code.join("\n")));
            continue;
        }
        if trimmed.is_empty() {
            flush(&mut paragraph, &mut blocks);
            continue;
        }
        if let Some((level, text)) = heading(trimmed) {
            flush(&mut paragraph, &mut blocks);
            blocks.push(Block::Heading(level, inline(text)));
            continue;
        }
        if is_rule(trimmed) {
            flush(&mut paragraph, &mut blocks);
            blocks.push(Block::Rule);
            continue;
        }
        // Setext headings: a paragraph line underlined with === or ---.
        if paragraph.len() == 1 && indent < 4 {
            let underline = trimmed.trim_end();
            if !underline.is_empty() && underline.chars().all(|c| c == '=') {
                blocks.push(Block::Heading(1, inline(paragraph[0])));
                paragraph.clear();
                continue;
            }
        }
        if let Some(rest) = trimmed.strip_prefix('>') {
            flush(&mut paragraph, &mut blocks);
            let mut quote = vec![rest.trim()];
            while let Some(next) = lines.peek() {
                match next.trim_start().strip_prefix('>') {
                    Some(more) => {
                        quote.push(more.trim());
                        lines.next();
                    }
                    None => break,
                }
            }
            let text: Vec<&str> = quote.into_iter().filter(|l| !l.is_empty()).collect();
            blocks.push(Block::Quote(inline(&text.join(" "))));
            continue;
        }
        if let Some((marker, text)) = item(trimmed) {
            flush(&mut paragraph, &mut blocks);
            blocks.push(Block::Item(indent / 2, marker, inline(text)));
            continue;
        }
        if trimmed.starts_with('|') {
            flush(&mut paragraph, &mut blocks);
            let mut rows = vec![cells(trimmed)];
            while let Some(next) = lines.peek().map(|l| l.trim()) {
                if !next.starts_with('|') {
                    break;
                }
                rows.push(cells(next));
                lines.next();
            }
            rows.retain(|row| !row.iter().all(|c| !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':' | ' '))));
            blocks.push(Block::Table(rows));
            continue;
        }
        if indent >= 4 && paragraph.is_empty() {
            let mut code = vec![&line[4.min(line.len())..]];
            while let Some(next) = lines.peek() {
                if next.len() - next.trim_start().len() < 4 && !next.trim().is_empty() {
                    break;
                }
                code.push(next.get(4..).unwrap_or(""));
                lines.next();
            }
            while code.last().is_some_and(|l| l.trim().is_empty()) {
                code.pop();
            }
            blocks.push(Block::Code(code.join("\n")));
            continue;
        }
        // A list item's continuation joins the item.
        if indent > 0
            && paragraph.is_empty()
            && let Some(Block::Item(_, _, text)) = blocks.last_mut()
        {
            let more = inline(trimmed);
            let offset = text.text.len() + 1;
            text.text.push(' ');
            text.text.push_str(&more.text);
            text.marks.extend(more.marks.into_iter().map(|(r, m)| (r.start + offset..r.end + offset, m)));
            text.links.extend(more.links.into_iter().map(|(r, u)| (r.start + offset..r.end + offset, u)));
            continue;
        }
        paragraph.push(trimmed.trim_end());
    }
    flush(&mut paragraph, &mut blocks);
    blocks
}

fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + 3..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

fn heading(line: &str) -> Option<(u8, &str)> {
    let level = line.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &line[level..];
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    Some((level as u8, rest.trim().trim_end_matches('#').trim_end()))
}

fn is_rule(line: &str) -> bool {
    let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    compact.len() >= 3
        && ['-', '*', '_'].into_iter().any(|m| compact.chars().all(|c| c == m))
}

fn item(line: &str) -> Option<(String, &str)> {
    let bullet = ["- ", "* ", "+ "].into_iter().find_map(|b| line.strip_prefix(b));
    if let Some(rest) = bullet {
        for (box_, mark) in [("[ ] ", "☐"), ("[x] ", "☑"), ("[X] ", "☑")] {
            if let Some(text) = rest.strip_prefix(box_) {
                return Some((mark.into(), text));
            }
        }
        return Some(("•".into(), rest));
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if (1..=9).contains(&digits) {
        let after = &line[digits..];
        if let Some(text) = after.strip_prefix(". ").or_else(|| after.strip_prefix(") ")) {
            return Some((format!("{}.", &line[..digits]), text));
        }
    }
    None
}

fn cells(row: &str) -> Vec<String> {
    let row = row.trim().trim_start_matches('|').trim_end_matches('|');
    row.split('|').map(|c| inline(c.trim()).text).collect()
}

/// Reads emphasis, inline code, links, images and a few HTML bits out of a line.
pub fn inline(source: &str) -> Inline {
    let mut out = Inline::default();
    let chars: Vec<char> = source.chars().collect();
    let mut i = 0;
    // Open emphasis: (marker, byte offset in out.text).
    let mut open: Vec<(&'static str, usize)> = Vec::new();
    let starts = |i: usize, s: &str| s.chars().enumerate().all(|(k, c)| chars.get(i + k) == Some(&c));
    while i < chars.len() {
        let c = chars[i];
        // Escapes.
        if c == '\\' && chars.get(i + 1).is_some_and(|n| n.is_ascii_punctuation()) {
            out.text.push(chars[i + 1]);
            i += 2;
            continue;
        }
        // Inline code.
        if c == '`' {
            let ticks = chars[i..].iter().take_while(|c| **c == '`').count();
            let fence: String = "`".repeat(ticks);
            let after: String = chars[i + ticks..].iter().collect();
            if let Some(end) = after.find(&fence) {
                let code = after[..end].trim();
                let start = out.text.len();
                out.text.push_str(code);
                out.marks.push((start..out.text.len(), Mark::Code));
                i += ticks + after[..end].chars().count() + ticks;
                continue;
            }
        }
        // Images and links.
        let image = c == '!' && chars.get(i + 1) == Some(&'[');
        if (c == '[' || image)
            && let Some((label, url, used)) = link_at(&chars[i + usize::from(image)..])
        {
            let start = out.text.len();
            if image {
                out.text.push_str(&format!("[image{}{}]", if label.is_empty() { "" } else { ": " }, label));
            } else {
                let label = inline(&label);
                out.text.push_str(&label.text);
                out.marks.extend(label.marks.into_iter().map(|(r, m)| (r.start + start..r.end + start, m)));
            }
            out.marks.push((start..out.text.len(), Mark::Link));
            out.links.push((start..out.text.len(), url));
            i += usize::from(image) + used;
            continue;
        }
        // Autolinks and bare URLs.
        if c == '<'
            && let Some(end) = chars[i..].iter().position(|c| *c == '>')
        {
            let inner: String = chars[i + 1..i + end].iter().collect();
            if inner.starts_with("http://") || inner.starts_with("https://") {
                push_link(&mut out, &inner, &inner);
                i += end + 1;
                continue;
            }
            // Simple HTML: <br> breaks the line, other tags vanish.
            let tag = inner.trim_start_matches('/').split([' ', '/']).next().unwrap_or("").to_lowercase();
            if !tag.is_empty() && tag.chars().all(|c| c.is_ascii_alphanumeric()) {
                if tag == "br" {
                    out.text.push(' ');
                }
                i += end + 1;
                continue;
            }
        }
        if (starts(i, "https://") || starts(i, "http://")) && (i == 0 || !chars[i - 1].is_alphanumeric()) {
            let end = chars[i..].iter().position(|c| c.is_whitespace() || *c == ')' || *c == '>').map_or(chars.len(), |e| i + e);
            let mut url: String = chars[i..end].iter().collect();
            while url.ends_with(['.', ',', ';', ':']) {
                url.pop();
            }
            let used = url.chars().count();
            push_link(&mut out, &url, &url);
            i += used;
            continue;
        }
        // Emphasis: ** __ ~~ * and _ (not inside words, for snake_case).
        let marker = ["**", "__", "~~", "*", "_"].into_iter().find(|m| starts(i, m));
        if let Some(marker) = marker {
            let len = marker.chars().count();
            let before = i.checked_sub(1).map(|b| chars[b]);
            let after = chars.get(i + len).copied();
            let word = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
            let underscore_in_word = marker.starts_with('_') && word(before) && word(after);
            if !underscore_in_word {
                // Closing: the opening marker, kept as text until now, goes.
                if before.is_some_and(|c| !c.is_whitespace())
                    && let Some(index) = open.iter().rposition(|(m, _)| *m == marker)
                {
                    let (_, at) = open.remove(index);
                    out.text.replace_range(at..at + len, "");
                    shift(&mut out, &mut open, at, len);
                    if at < out.text.len() {
                        let mark = match marker {
                            "**" | "__" => Mark::Bold,
                            "~~" => Mark::Strike,
                            _ => Mark::Italic,
                        };
                        out.marks.push((at..out.text.len(), mark));
                    }
                    i += len;
                    continue;
                }
                // Opening: kept as text in case it is never closed.
                if after.is_some_and(|c| !c.is_whitespace()) {
                    open.push((marker, out.text.len()));
                    out.text.push_str(marker);
                    i += len;
                    continue;
                }
            }
        }
        out.text.push(c);
        i += 1;
    }
    out.marks.sort_by_key(|(r, _)| r.start);
    out
}

impl Inline {
    /// The marks as sorted, non-overlapping ranges, each with every mark that covers it (what
    /// a text's runs need: nested emphasis and a bold link overlap).
    pub fn runs(&self) -> Vec<(Range<usize>, Vec<Mark>)> {
        let mut cuts: Vec<usize> = self.marks.iter().flat_map(|(r, _)| [r.start, r.end]).collect();
        cuts.sort_unstable();
        cuts.dedup();
        cuts.windows(2)
            .filter_map(|w| {
                let range = w[0]..w[1];
                let marks: Vec<Mark> = self
                    .marks
                    .iter()
                    .filter(|(r, _)| r.start <= range.start && range.end <= r.end)
                    .map(|(_, m)| *m)
                    .collect();
                (!marks.is_empty() && range.end <= self.text.len()).then_some((range, marks))
            })
            .collect()
    }
}

/// Moves what follows a removed marker (`len` bytes at `at`) back by its length.
fn shift(out: &mut Inline, open: &mut [(&'static str, usize)], at: usize, len: usize) {
    let fix = |x: &mut usize| {
        if *x >= at + len {
            *x -= len;
        }
    };
    for (range, _) in &mut out.marks {
        fix(&mut range.start);
        fix(&mut range.end);
    }
    for (range, _) in &mut out.links {
        fix(&mut range.start);
        fix(&mut range.end);
    }
    for (_, pos) in open.iter_mut() {
        fix(pos);
    }
}

fn push_link(out: &mut Inline, text: &str, url: &str) {
    let start = out.text.len();
    out.text.push_str(text);
    out.marks.push((start..out.text.len(), Mark::Link));
    out.links.push((start..out.text.len(), url.to_string()));
}

/// `[label](url)` at the start of `chars`: the label, the URL and how many chars it took.
fn link_at(chars: &[char]) -> Option<(String, String, usize)> {
    if chars.first() != Some(&'[') {
        return None;
    }
    let mut depth = 0;
    let mut close = None;
    for (k, c) in chars.iter().enumerate() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(k);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close?;
    if chars.get(close + 1) != Some(&'(') {
        return None;
    }
    let end = chars[close + 2..].iter().position(|c| *c == ')')? + close + 2;
    let label: String = chars[1..close].iter().collect();
    let target: String = chars[close + 2..end].iter().collect();
    let url = target.split_whitespace().next().unwrap_or("").to_string();
    Some((label, url, end + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks() {
        let md = "## Problem\n\nA plugin's popup (#435) runs\na program.\n\n- one\n  more\n- [x] done\n1. first\n\n> quoted\n> text\n\n```rust\nfn main() {}\n```\n---\n| a | b |\n|---|---|\n| 1 | 2 |\n<!-- hidden -->";
        let blocks = parse(md);
        assert_eq!(blocks[0], Block::Heading(2, inline("Problem")));
        assert_eq!(blocks[1], Block::Paragraph(inline("A plugin's popup (#435) runs a program.")));
        let Block::Item(0, marker, text) = &blocks[2] else { panic!("{:?}", blocks[2]) };
        assert_eq!((marker.as_str(), text.text.as_str()), ("•", "one more"));
        assert!(matches!(&blocks[3], Block::Item(0, m, t) if m == "☑" && t.text == "done"));
        assert!(matches!(&blocks[4], Block::Item(0, m, t) if m == "1." && t.text == "first"));
        assert!(matches!(&blocks[5], Block::Quote(t) if t.text == "quoted text"));
        assert_eq!(blocks[6], Block::Code("fn main() {}".into()));
        assert_eq!(blocks[7], Block::Rule);
        assert_eq!(blocks[8], Block::Table(vec![vec!["a".into(), "b".into()], vec!["1".into(), "2".into()]]));
        assert_eq!(blocks.len(), 9);
    }

    #[test]
    fn setext_heading() {
        assert_eq!(parse("Title\n=====\nbody"), vec![Block::Heading(1, inline("Title")), Block::Paragraph(inline("body"))]);
    }

    #[test]
    fn emphasis_code_and_links() {
        let line = inline("Use **bold**, *it*, `surface --view` and [the doc](https://x.dev/a) for snake_case_names");
        assert_eq!(line.text, "Use bold, it, surface --view and the doc for snake_case_names");
        let at = |s: &str| line.text.find(s).map(|i| i..i + s.len());
        assert!(line.marks.contains(&(at("bold").unwrap(), Mark::Bold)));
        assert!(line.marks.contains(&(at("it,").map(|r| r.start..r.end - 1).unwrap(), Mark::Italic)));
        assert!(line.marks.contains(&(at("surface --view").unwrap(), Mark::Code)));
        assert_eq!(line.links, vec![(at("the doc").unwrap(), "https://x.dev/a".to_string())]);
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
        let start = line.text.find("wörld").unwrap();
        assert_eq!(line.marks, vec![(start..start + "wörld".len(), Mark::Bold)]);
    }
}
