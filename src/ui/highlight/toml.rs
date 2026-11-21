use std::ops::Range;

use super::{Kind, Span};

const STOPS: &[u8] = b"=,{}[]#\"'";
const WORDS: [&str; 4] = ["true", "false", "inf", "nan"];

#[derive(Default)]
struct State {
    open: Option<&'static str>,
    nests: Vec<u8>,
}

struct Reader<'a> {
    text: &'a str,
    at: usize,
    key: bool,
    pending: Vec<Range<usize>>,
    spans: Vec<Span>,
    state: &'a mut State,
}

pub(super) fn spans(lines: &[&str]) -> Vec<Vec<Span>> {
    let mut state = State::default();
    lines.iter().map(|line| line_spans(line, &mut state)).collect()
}

fn line_spans(text: &str, state: &mut State) -> Vec<Span> {
    let key = state.nests.is_empty();
    let mut reader = Reader { text, at: 0, key, pending: Vec::new(), spans: Vec::new(), state };
    if !reader.carry() {
        reader.header();
    }
    while reader.at < text.len() {
        reader.step();
    }
    reader.spans
}

impl Reader<'_> {
    fn bytes(&self) -> &[u8] {
        self.text.as_bytes()
    }

    fn carry(&mut self) -> bool {
        let Some(quote) = self.state.open else { return false };
        let end = match self.text.find(quote) {
            Some(found) => {
                self.state.open = None;
                found + quote.len()
            }
            None => self.text.len(),
        };
        self.spans.push((0..end, Kind::String));
        self.at = end;
        true
    }

    fn header(&mut self) {
        let start = self.text.len() - self.text.trim_start().len();
        if !self.state.nests.is_empty() || !self.text[start..].starts_with('[') {
            return;
        }
        let before_comment = self.text[start..].find('#').map_or(self.text.len(), |cut| start + cut);
        let Some(close) = self.text[start..before_comment].rfind(']') else { return };
        let end = start + close + 1;
        self.spans.push((start..end, Kind::Function));
        self.at = end;
        self.key = false;
    }

    fn step(&mut self) {
        match self.bytes()[self.at] {
            b'#' => self.comment(),
            b'"' | b'\'' => self.string(),
            b'=' => self.equals(),
            b',' => self.comma(),
            b'{' | b'[' => self.open(),
            b'}' | b']' => self.close(),
            byte if byte.is_ascii_whitespace() => self.at += 1,
            _ => self.word(),
        }
    }

    fn comment(&mut self) {
        self.spans.push((self.at..self.text.len(), Kind::Comment));
        self.at = self.text.len();
    }

    fn string(&mut self) {
        let quote = self.bytes()[self.at];
        let triple = if quote == b'"' { "\"\"\"" } else { "'''" };
        let end = if self.text[self.at..].starts_with(triple) {
            self.long_string_end(triple)
        } else {
            self.string_end(quote)
        };
        self.token(self.at..end, Kind::String);
        self.at = end;
    }

    fn long_string_end(&mut self, triple: &'static str) -> usize {
        let from = self.at + triple.len();
        if let Some(found) = self.text[from..].find(triple) {
            return from + found + triple.len();
        }
        self.state.open = Some(triple);
        self.text.len()
    }

    fn string_end(&self, quote: u8) -> usize {
        let mut at = self.at + 1;
        while at < self.text.len() {
            match self.bytes()[at] {
                b'\\' if quote == b'"' => at += 2,
                byte if byte == quote => return at + 1,
                _ => at += 1,
            }
        }
        self.text.len()
    }

    fn word(&mut self) {
        let rest = &self.bytes()[self.at..];
        let len = rest.iter().position(|b| b.is_ascii_whitespace() || STOPS.contains(b)).unwrap_or(rest.len());
        let range = self.at..self.at + len;
        self.at = range.end;
        if self.key {
            self.pending.push(range);
        } else if is_number(&self.text[range.clone()]) {
            self.spans.push((range, Kind::Number));
        }
    }

    fn token(&mut self, range: Range<usize>, kind: Kind) {
        if self.key {
            self.pending.push(range);
        } else {
            self.spans.push((range, kind));
        }
    }

    fn equals(&mut self) {
        self.spans.extend(self.pending.drain(..).map(|range| (range, Kind::Tag)));
        self.key = false;
        self.at += 1;
    }

    fn comma(&mut self) {
        self.pending.clear();
        self.key = self.state.nests.last() == Some(&b'{');
        self.at += 1;
    }

    fn open(&mut self) {
        let byte = self.bytes()[self.at];
        self.pending.clear();
        self.at += 1;
        if byte == b'[' && self.key {
            return;
        }
        self.state.nests.push(byte);
        self.key = byte == b'{';
    }

    fn close(&mut self) {
        self.state.nests.pop();
        self.pending.clear();
        self.key = false;
        self.at += 1;
    }
}

fn is_number(word: &str) -> bool {
    word.starts_with(|c: char| c.is_ascii_digit() || c == '+' || c == '-') || WORDS.contains(&word)
}

#[cfg(test)]
mod tests;
