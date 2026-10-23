use std::iter::Peekable;
use std::ops::Range;
use std::str::Lines;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mark {
    Bold,
    Italic,
    Code,
    Strike,
    Link,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Inline {
    pub text: String,
    pub marks: Vec<(Range<usize>, Mark)>,
    pub links: Vec<(Range<usize>, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Block {
    Heading(u8, Inline),
    Paragraph(Inline),
    Item(usize, String, Inline),
    Quote(Inline),
    Code(String),
    Table(Vec<Vec<String>>),
    Rule,
}

pub(super) fn parse(source: &str) -> Vec<Block> {
    let source = strip_comments(&source.replace("\r\n", "\n"));
    let mut parser = Parser { lines: source.lines().peekable(), blocks: Vec::new(), paragraph: Vec::new() };
    while let Some(line) = parser.lines.next() {
        parser.line(line);
    }
    parser.flush();
    parser.blocks
}

struct Parser<'a> {
    lines: Peekable<Lines<'a>>,
    blocks: Vec<Block>,
    paragraph: Vec<&'a str>,
}

impl<'a> Parser<'a> {
    fn line(&mut self, line: &'a str) {
        let trimmed = line.trim_start();
        let indent = indent_of(line);
        let taken = self.fenced(trimmed, indent)
            || self.blank(trimmed)
            || self.single(trimmed, indent)
            || self.setext(trimmed, indent)
            || self.quote(trimmed)
            || self.table(trimmed)
            || self.indented(line, indent)
            || self.continuation(trimmed, indent);
        if !taken {
            self.paragraph.push(trimmed.trim_end());
        }
    }

    fn flush(&mut self) {
        if !self.paragraph.is_empty() {
            self.blocks.push(Block::Paragraph(inline(&self.paragraph.join(" "))));
            self.paragraph.clear();
        }
    }

    fn push(&mut self, block: Block) -> bool {
        self.flush();
        self.blocks.push(block);
        true
    }

    fn fenced(&mut self, trimmed: &str, indent: usize) -> bool {
        let Some(fence) = ["```", "~~~"].into_iter().find(|f| trimmed.starts_with(f)) else { return false };
        let mut code = Vec::new();
        for next in self.lines.by_ref() {
            if next.trim_start().starts_with(fence) {
                break;
            }
            code.push(next.get(indent.min(indent_of(next))..).unwrap_or(next));
        }
        self.push(Block::Code(code.join("\n")))
    }

    fn blank(&mut self, trimmed: &str) -> bool {
        if !trimmed.is_empty() {
            return false;
        }
        self.flush();
        true
    }

    fn single(&mut self, trimmed: &str, indent: usize) -> bool {
        if let Some((level, text)) = heading(trimmed) {
            return self.push(Block::Heading(level, inline(text)));
        }
        if is_rule(trimmed) {
            return self.push(Block::Rule);
        }
        match item(trimmed) {
            Some((marker, text)) => self.push(Block::Item(indent / 2, marker, inline(text))),
            None => false,
        }
    }

    fn setext(&mut self, trimmed: &str, indent: usize) -> bool {
        let underline = trimmed.trim_end();
        let [title] = self.paragraph[..] else { return false };
        if indent >= 4 || underline.is_empty() || !underline.chars().all(|c| c == '=') {
            return false;
        }
        self.paragraph.clear();
        self.push(Block::Heading(1, inline(title)))
    }

    fn quote(&mut self, trimmed: &'a str) -> bool {
        let Some(rest) = trimmed.strip_prefix('>') else { return false };
        let mut quote = vec![rest.trim()];
        while let Some(more) = self.lines.peek().copied().and_then(|next| next.trim_start().strip_prefix('>')) {
            quote.push(more.trim());
            self.lines.next();
        }
        let text: Vec<&str> = quote.into_iter().filter(|l| !l.is_empty()).collect();
        self.push(Block::Quote(inline(&text.join(" "))))
    }

    fn table(&mut self, trimmed: &str) -> bool {
        if !trimmed.starts_with('|') {
            return false;
        }
        let mut rows = vec![cells(trimmed)];
        while let Some(next) = self.lines.peek().copied().map(str::trim).filter(|l| l.starts_with('|')) {
            rows.push(cells(next));
            self.lines.next();
        }
        rows.retain(|row| !is_separator(row));
        self.push(Block::Table(rows))
    }

    fn indented(&mut self, line: &'a str, indent: usize) -> bool {
        if indent < 4 || !self.paragraph.is_empty() {
            return false;
        }
        let mut code = vec![&line[4.min(line.len())..]];
        while let Some(next) = self.lines.peek().copied().filter(|next| indent_of(next) >= 4 || next.trim().is_empty())
        {
            code.push(next.get(4..).unwrap_or(""));
            self.lines.next();
        }
        while code.last().is_some_and(|l| l.trim().is_empty()) {
            code.pop();
        }
        self.blocks.push(Block::Code(code.join("\n")));
        true
    }

    fn continuation(&mut self, trimmed: &str, indent: usize) -> bool {
        if indent == 0 || !self.paragraph.is_empty() {
            return false;
        }
        let Some(Block::Item(_, _, text)) = self.blocks.last_mut() else { return false };
        text.text.push(' ');
        text.append(inline(trimmed), true);
        true
    }
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find("-->") else {
            rest = "";
            break;
        };
        rest = &rest[start + end + 3..];
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
    Some((u8::try_from(level).ok()?, rest.trim().trim_end_matches('#').trim_end()))
}

fn is_rule(line: &str) -> bool {
    let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    compact.len() >= 3 && ['-', '*', '_'].into_iter().any(|m| compact.chars().all(|c| c == m))
}

fn item(line: &str) -> Option<(String, &str)> {
    if let Some(rest) = ["- ", "* ", "+ "].into_iter().find_map(|b| line.strip_prefix(b)) {
        return Some(task_box(rest).unwrap_or_else(|| ("•".into(), rest)));
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if !(1..=9).contains(&digits) {
        return None;
    }
    let after = &line[digits..];
    let text = after.strip_prefix(". ").or_else(|| after.strip_prefix(") "))?;
    Some((format!("{}.", &line[..digits]), text))
}

fn task_box(rest: &str) -> Option<(String, &str)> {
    [("[ ] ", "☐"), ("[x] ", "☑"), ("[X] ", "☑")]
        .into_iter()
        .find_map(|(box_, mark)| rest.strip_prefix(box_).map(|text| (mark.to_string(), text)))
}

fn cells(row: &str) -> Vec<String> {
    let row = row.trim().trim_start_matches('|').trim_end_matches('|');
    row.split('|').map(|c| inline(c.trim()).text).collect()
}

fn is_separator(row: &[String]) -> bool {
    row.iter().all(|c| !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':' | ' ')))
}

pub(super) fn inline(source: &str) -> Inline {
    let mut reader = Reader { chars: source.chars().collect(), at: 0, out: Inline::default(), opened: Vec::new() };
    while reader.at < reader.chars.len() {
        reader.step();
    }
    reader.out.marks.sort_by_key(|(r, _)| r.start);
    reader.out
}

struct Reader {
    chars: Vec<char>,
    at: usize,
    out: Inline,
    opened: Vec<(&'static str, usize)>,
}

impl Reader {
    fn step(&mut self) {
        let taken = self.escape() || self.code() || self.link() || self.angle() || self.bare_url() || self.emphasis();
        if !taken {
            self.out.text.push(self.chars[self.at]);
            self.at += 1;
        }
    }

    fn current(&self) -> char {
        self.chars[self.at]
    }

    fn starts(&self, text: &str) -> bool {
        text.chars().enumerate().all(|(k, c)| self.chars.get(self.at + k) == Some(&c))
    }

    fn escape(&mut self) -> bool {
        if self.current() != '\\' {
            return false;
        }
        let Some(&next) = self.chars.get(self.at + 1).filter(|c| c.is_ascii_punctuation()) else { return false };
        self.out.text.push(next);
        self.at += 2;
        true
    }

    fn code(&mut self) -> bool {
        if self.current() != '`' {
            return false;
        }
        let ticks = self.chars[self.at..].iter().take_while(|c| **c == '`').count();
        let after: String = self.chars[self.at + ticks..].iter().collect();
        let Some(end) = after.find(&"`".repeat(ticks)) else { return false };
        let start = self.out.text.len();
        self.out.text.push_str(after[..end].trim());
        self.out.marks.push((start..self.out.text.len(), Mark::Code));
        self.at += ticks + after[..end].chars().count() + ticks;
        true
    }

    fn link(&mut self) -> bool {
        let image = self.current() == '!' && self.chars.get(self.at + 1) == Some(&'[');
        if self.current() != '[' && !image {
            return false;
        }
        let Some((label, url, used)) = link_at(&self.chars[self.at + usize::from(image)..]) else { return false };
        let start = self.out.text.len();
        if image {
            let alt = if label.is_empty() { "[image]".to_string() } else { format!("[image: {label}]") };
            self.out.text.push_str(&alt);
        } else {
            self.out.append(inline(&label), false);
        }
        self.out.marks.push((start..self.out.text.len(), Mark::Link));
        self.out.links.push((start..self.out.text.len(), url));
        self.at += usize::from(image) + used;
        true
    }

    fn angle(&mut self) -> bool {
        if self.current() != '<' {
            return false;
        }
        let Some(end) = self.chars[self.at..].iter().position(|c| *c == '>') else { return false };
        let inner: String = self.chars[self.at + 1..self.at + end].iter().collect();
        if inner.starts_with("http://") || inner.starts_with("https://") {
            push_link(&mut self.out, &inner);
        } else if let Some(tag) = html_tag(&inner) {
            if tag == "br" {
                self.out.text.push(' ');
            }
        } else {
            return false;
        }
        self.at += end + 1;
        true
    }

    fn bare_url(&mut self) -> bool {
        let word_start = self.at == 0 || !self.chars[self.at - 1].is_alphanumeric();
        if !word_start || !(self.starts("https://") || self.starts("http://")) {
            return false;
        }
        let end = self.chars[self.at..]
            .iter()
            .position(|c| c.is_whitespace() || *c == ')' || *c == '>')
            .map_or(self.chars.len(), |e| self.at + e);
        let mut url: String = self.chars[self.at..end].iter().collect();
        while url.ends_with(['.', ',', ';', ':']) {
            url.pop();
        }
        self.at += url.chars().count();
        push_link(&mut self.out, &url);
        true
    }

    fn emphasis(&mut self) -> bool {
        let Some(marker) = ["**", "__", "~~", "*", "_"].into_iter().find(|m| self.starts(m)) else { return false };
        let before = self.at.checked_sub(1).map(|b| self.chars[b]);
        let after = self.chars.get(self.at + marker.len()).copied();
        let word = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
        if marker.starts_with('_') && word(before) && word(after) {
            return false;
        }
        self.close_marker(marker, before) || self.open_marker(marker, after)
    }

    fn close_marker(&mut self, marker: &'static str, before: Option<char>) -> bool {
        if before.is_none_or(char::is_whitespace) {
            return false;
        }
        let Some(index) = self.opened.iter().rposition(|(m, _)| *m == marker) else { return false };
        let len = marker.len();
        let (_, at) = self.opened.remove(index);
        self.out.text.replace_range(at..at + len, "");
        self.shift(at, len);
        if at < self.out.text.len() {
            self.out.marks.push((at..self.out.text.len(), mark_of(marker)));
        }
        self.at += len;
        true
    }

    fn open_marker(&mut self, marker: &'static str, after: Option<char>) -> bool {
        if after.is_none_or(char::is_whitespace) {
            return false;
        }
        self.opened.push((marker, self.out.text.len()));
        self.out.text.push_str(marker);
        self.at += marker.len();
        true
    }

    fn shift(&mut self, at: usize, len: usize) {
        let fix = |x: &mut usize| {
            if *x >= at + len {
                *x -= len;
            }
        };
        let marks = self.out.marks.iter_mut().map(|(range, _)| range);
        let links = self.out.links.iter_mut().map(|(range, _)| range);
        for range in marks.chain(links) {
            fix(&mut range.start);
            fix(&mut range.end);
        }
        self.opened.iter_mut().for_each(|(_, pos)| fix(pos));
    }
}

fn mark_of(marker: &str) -> Mark {
    match marker {
        "**" | "__" => Mark::Bold,
        "~~" => Mark::Strike,
        _ => Mark::Italic,
    }
}

fn html_tag(inner: &str) -> Option<String> {
    let tag = inner.trim_start_matches('/').split([' ', '/']).next().unwrap_or("").to_lowercase();
    (!tag.is_empty() && tag.chars().all(|c| c.is_ascii_alphanumeric())).then_some(tag)
}

impl Inline {
    pub(super) fn runs(&self) -> Vec<(Range<usize>, Vec<Mark>)> {
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

    fn append(&mut self, more: Inline, with_links: bool) {
        let offset = self.text.len();
        self.text.push_str(&more.text);
        self.marks.extend(more.marks.into_iter().map(|(r, m)| (r.start + offset..r.end + offset, m)));
        if with_links {
            self.links.extend(more.links.into_iter().map(|(r, u)| (r.start + offset..r.end + offset, u)));
        }
    }
}

fn push_link(out: &mut Inline, url: &str) {
    let start = out.text.len();
    out.text.push_str(url);
    out.marks.push((start..out.text.len(), Mark::Link));
    out.links.push((start..out.text.len(), url.to_string()));
}

fn link_at(chars: &[char]) -> Option<(String, String, usize)> {
    if chars.first() != Some(&'[') {
        return None;
    }
    let close = closing_bracket(chars)?;
    if chars.get(close + 1) != Some(&'(') {
        return None;
    }
    let end = chars[close + 2..].iter().position(|c| *c == ')')? + close + 2;
    let label: String = chars[1..close].iter().collect();
    let target: String = chars[close + 2..end].iter().collect();
    let url = target.split_whitespace().next().unwrap_or("").to_string();
    Some((label, url, end + 1))
}

fn closing_bracket(chars: &[char]) -> Option<usize> {
    let mut depth = 0;
    for (k, c) in chars.iter().enumerate() {
        match c {
            '[' => depth += 1,
            ']' if depth == 1 => return Some(k),
            ']' => depth -= 1,
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests;
