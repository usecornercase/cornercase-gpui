use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::quicklook::{finished_within, scratch};
use crate::desktop;

const PIXELS: u32 = 900;
const WITHIN: Duration = Duration::from_secs(8);
const ENTRY: &str = "[Thumbnailer Entry]";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Thumbnailer {
    try_exec: Option<String>,
    exec: String,
    types: Vec<String>,
}

impl Thumbnailer {
    fn program(&self) -> &str {
        self.try_exec.as_deref().or_else(|| self.exec.split_whitespace().next()).unwrap_or_default()
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

struct Fill {
    path: String,
    uri: String,
    out: String,
    size: String,
}

impl Fill {
    fn expand(&self, word: &str) -> String {
        let mut text = String::new();
        let mut chars = word.chars();
        while let Some(c) = chars.next() {
            let code = if c == '%' { chars.next() } else { None };
            match code {
                Some('i') => text.push_str(&self.path),
                Some('u') => text.push_str(&self.uri),
                Some('o') => text.push_str(&self.out),
                Some('s') => text.push_str(&self.size),
                Some('%') => text.push('%'),
                Some(other) => text.extend(['%', other]),
                None => text.push(c),
            }
        }
        text
    }
}

pub(super) fn argv(exec: &str, path: &Path, out: &Path, size: u32) -> Option<Vec<String>> {
    let uri = url::Url::from_file_path(path).ok()?.to_string();
    let lossy = |p: &Path| p.to_string_lossy().into_owned();
    let fill = Fill { path: lossy(path), uri, out: lossy(out), size: size.to_string() };
    let argv: Vec<String> = exec.split_whitespace().map(|word| fill.expand(word.trim_matches('"'))).collect();
    (!argv.is_empty()).then_some(argv)
}

fn glob_matches(glob: &str, name: &str) -> bool {
    match glob.strip_prefix('*') {
        Some(suffix) if !suffix.contains(['*', '?', '[']) => name.ends_with(suffix),
        _ => glob == name,
    }
}

fn glob_line<'a>(line: &'a str, name: &str, lower: &str) -> Option<(u32, usize, &'a str)> {
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
    let lower = name.to_lowercase();
    let matches = globs.lines().filter(|line| !line.starts_with('#')).filter_map(|line| glob_line(line, name, &lower));
    matches.max_by_key(|(weight, len, _)| (*weight, *len)).map(|(_, _, mime)| mime.to_string())
}

pub(super) fn pick<'a>(all: &'a [Thumbnailer], mime: &str, runs: &dyn Fn(&str) -> bool) -> Option<&'a Thumbnailer> {
    all.iter().find(|thumbnailer| thumbnailer.reads(mime) && runs(thumbnailer.program()))
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

pub(super) fn thumbnail(path: &Path) -> Option<Vec<u8>> {
    let data = desktop::xdg_data_dirs();
    let name = path.file_name()?.to_str()?;
    let mime = data.iter().find_map(|dir| mime_of(name, &std::fs::read_to_string(dir.join("mime/globs2")).ok()?))?;
    let all = installed(&data);
    let chosen = pick(&all, &mime, &runs)?;
    let out = scratch();
    std::fs::create_dir_all(&out).ok()?;
    let png = out.join("thumbnail.png");
    let argv = argv(&chosen.exec, path, &png, PIXELS)?;
    let (program, rest) = argv.split_first()?;
    let finished = finished_within(Command::new(program).args(rest), WITHIN);
    let made = std::fs::read(&png).ok().filter(|_| finished);
    let _ = std::fs::remove_dir_all(&out);
    made
}

#[cfg(test)]
mod tests;
