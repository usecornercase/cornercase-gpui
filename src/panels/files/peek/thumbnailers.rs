use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::super::disk;
use super::quicklook::{SCRATCH, finished_within, picture};
use crate::desktop::{self, Scratch};

const PIXELS: u32 = 900;
const WITHIN: Duration = Duration::from_secs(8);
const ENTRY: &str = "[Thumbnailer Entry]";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Thumbnailer {
    try_exec: Option<String>,
    exec: String,
    types: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Missing {
    NoReader,
    Failed(String),
}

impl Thumbnailer {
    fn program(&self) -> String {
        self.try_exec.clone().or_else(|| words(&self.exec).into_iter().next()).unwrap_or_default()
    }

    fn name(&self) -> String {
        let program = self.program();
        Path::new(&program).file_name().map_or(program.clone(), |name| name.to_string_lossy().into_owned())
    }

    fn reads(&self, mime: &str) -> bool {
        self.types.iter().any(|kind| kind == mime)
    }
}

pub(super) fn parse(text: &str) -> Option<Thumbnailer> {
    let mut entry = false;
    let (mut try_exec, mut exec, mut types) = (None, None, Vec::new());
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            entry = line == ENTRY;
        }
        let Some((key, value)) = line.split_once('=').filter(|_| entry) else { continue };
        let value = value.trim().to_string();
        match key.trim() {
            "TryExec" => try_exec = Some(value),
            "Exec" => exec = Some(value),
            "MimeType" => types = value.split(';').filter(|kind| !kind.is_empty()).map(str::to_string).collect(),
            _ => {}
        }
    }
    Some(Thumbnailer { try_exec, exec: exec.filter(|exec| !exec.is_empty())?, types })
}

#[derive(Default)]
struct Words {
    done: Vec<String>,
    word: Option<String>,
    quote: Option<char>,
}

impl Words {
    fn take(&mut self, c: char, rest: &mut impl Iterator<Item = char>) {
        match (self.quote, c) {
            (None, c) if c.is_whitespace() => self.done.extend(self.word.take()),
            (None, '"' | '\'') => {
                self.quote = Some(c);
                self.word.get_or_insert_default();
            }
            (Some(quote), c) if c == quote => self.quote = None,
            (None, '\\') => self.word.get_or_insert_default().extend(rest.next()),
            (Some('"'), '\\') => escaped(self.word.get_or_insert_default(), rest.next()),
            (_, c) => self.word.get_or_insert_default().push(c),
        }
    }
}

fn escaped(word: &mut String, next: Option<char>) {
    match next {
        Some(c @ ('"' | '`' | '$' | '\\')) => word.push(c),
        Some(c) => word.extend(['\\', c]),
        None => word.push('\\'),
    }
}

pub(super) fn words(exec: &str) -> Vec<String> {
    let mut words = Words::default();
    let mut chars = exec.chars();
    while let Some(c) = chars.next() {
        words.take(c, &mut chars);
    }
    words.done.extend(words.word);
    words.done
}

struct Fill {
    path: OsString,
    uri: String,
    out: OsString,
    size: String,
}

impl Fill {
    fn expand(&self, word: &str) -> OsString {
        let mut text = OsString::new();
        let mut chars = word.chars();
        while let Some(c) = chars.next() {
            let code = if c == '%' { chars.next() } else { None };
            match code {
                Some('i') => text.push(&self.path),
                Some('u') => text.push(&self.uri),
                Some('o') => text.push(&self.out),
                Some('s') => text.push(&self.size),
                Some('%') => text.push("%"),
                Some(other) => text.push(format!("%{other}")),
                None => text.push(c.encode_utf8(&mut [0; 4])),
            }
        }
        text
    }
}

pub(super) fn argv(exec: &str, path: &Path, out: &Path, size: u32) -> Option<Vec<OsString>> {
    let uri = url::Url::from_file_path(path).ok()?.to_string();
    let fill = Fill { path: path.into(), uri, out: out.into(), size: size.to_string() };
    let argv: Vec<OsString> = words(exec).iter().map(|word| fill.expand(word)).collect();
    (!argv.is_empty()).then_some(argv)
}

enum Token {
    Star,
    One(usize),
    Miss,
}

fn token(glob: &[char], c: char) -> Token {
    let literal = |len: usize, wanted: char| if wanted == c { Token::One(len) } else { Token::Miss };
    match glob {
        [] => Token::Miss,
        ['*', ..] => Token::Star,
        ['?', ..] => Token::One(1),
        ['[', ..] => bracket(glob, c).unwrap_or_else(|| literal(1, '[')),
        ['\\', escaped, ..] => literal(2, *escaped),
        [wanted, ..] => literal(1, *wanted),
    }
}

fn bracket(glob: &[char], c: char) -> Option<Token> {
    let negated = matches!(glob.get(1), Some('!' | '^'));
    let start = if negated { 2 } else { 1 };
    let close = start + 1 + glob.get(start + 1..)?.iter().position(|&ch| ch == ']')?;
    let hit = in_set(glob.get(start..close)?, c) != negated;
    Some(if hit { Token::One(close + 1) } else { Token::Miss })
}

fn in_set(set: &[char], c: char) -> bool {
    match set {
        [] => false,
        [low, '-', high, rest @ ..] => (*low..=*high).contains(&c) || in_set(rest, c),
        [one, rest @ ..] => *one == c || in_set(rest, c),
    }
}

fn glob_matches(glob: &str, name: &[char]) -> bool {
    let glob: Vec<char> = glob.chars().collect();
    let (mut at, mut n, mut star) = (0, 0, None);
    while let Some(&c) = name.get(n) {
        match token(glob.get(at..).unwrap_or_default(), c) {
            Token::Star => (star, at) = (Some((at + 1, n)), at + 1),
            Token::One(len) => (at, n) = (at + len, n + 1),
            Token::Miss => {
                let Some((after, from)) = star else { return false };
                (at, n, star) = (after, from + 1, Some((after, from + 1)));
            }
        }
    }
    glob.get(at..).unwrap_or_default().iter().all(|&c| c == '*')
}

fn glob_line<'a>(line: &'a str, name: &[char], lower: &[char]) -> Option<(u32, usize, &'a str)> {
    let mut parts = line.split(':');
    let weight: u32 = parts.next()?.parse().ok()?;
    let (mime, glob) = (parts.next()?, parts.next()?);
    let matched = if parts.any(|flag| flag == "cs") {
        glob_matches(glob, name)
    } else {
        glob_matches(&glob.to_lowercase(), lower)
    };
    matched.then_some((weight, glob.len(), mime))
}

pub(super) fn mime_of(name: &str, globs: &str) -> Option<String> {
    let (exact, lower): (Vec<char>, Vec<char>) = (name.chars().collect(), name.to_lowercase().chars().collect());
    let lines = globs.lines().filter(|line| !line.starts_with('#'));
    let matches = lines.filter_map(|line| glob_line(line, &exact, &lower));
    matches.max_by_key(|(weight, len, _)| (*weight, *len)).map(|(_, _, mime)| mime.to_string())
}

pub(super) fn mime_in(name: &str, files: &[String]) -> Option<String> {
    let merged: Vec<&str> = files.iter().rev().map(String::as_str).collect();
    mime_of(name, &merged.join("\n"))
}

pub(super) fn pick<'a>(all: &'a [Thumbnailer], mime: &str, runs: &dyn Fn(&str) -> bool) -> Option<&'a Thumbnailer> {
    all.iter().find(|thumbnailer| thumbnailer.reads(mime) && runs(&thumbnailer.program()))
}

fn runs(program: &str) -> bool {
    if program.starts_with('/') { desktop::executable(Path::new(program)) } else { desktop::on_path(program).is_some() }
}

fn installed(data: &[PathBuf]) -> Vec<Thumbnailer> {
    let mut found = Vec::new();
    for dir in data {
        let Ok(entries) = std::fs::read_dir(dir.join("thumbnailers")) else { continue };
        let mut files: Vec<PathBuf> = entries.filter_map(|entry| entry.ok().map(|entry| entry.path())).collect();
        files.retain(|file| file.extension().is_some_and(|e| e == "thumbnailer"));
        files.sort();
        found.extend(files.iter().filter_map(|file| parse(&std::fs::read_to_string(file).ok()?)));
    }
    found
}

pub(super) fn thumbnail(path: &Path) -> Result<Vec<u8>, Missing> {
    let data = desktop::xdg_data_dirs();
    let name = path.file_name().and_then(|name| name.to_str()).ok_or(Missing::NoReader)?;
    let globs: Vec<String> =
        data.iter().filter_map(|dir| std::fs::read_to_string(dir.join("mime/globs2")).ok()).collect();
    let mime = mime_in(name, &globs).ok_or(Missing::NoReader)?;
    let all = installed(&data);
    let chosen = pick(&all, &mime, &runs).ok_or(Missing::NoReader)?;
    made_by(chosen, path)
}

fn made_by(chosen: &Thumbnailer, path: &Path) -> Result<Vec<u8>, Missing> {
    make(chosen, path).map_err(|why| Missing::Failed(format!("{} {why}", chosen.name())))
}

fn make(chosen: &Thumbnailer, path: &Path) -> Result<Vec<u8>, String> {
    let out = Scratch::folder(SCRATCH).map_err(|e| format!("got no temporary folder: {e}"))?;
    let png = out.path().join("thumbnail.png");
    let argv = argv(&chosen.exec, path, &png, PIXELS).ok_or("cannot be given this file")?;
    let (program, rest) = argv.split_first().ok_or("has no command")?;
    finished_within(Command::new(program).args(rest), WITHIN)?;
    picture(&png, disk::MAX_IMAGE_BYTES)
}

#[cfg(test)]
mod tests;
