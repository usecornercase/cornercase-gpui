use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Result, bail};
use base64::Engine as _;
use serde::Deserialize;

use crate::cli;

use super::disk::{self, Content, Stamp};
use super::search::{self, FileHits, LineHit, NameHit, TextResults};
use super::{Mode, Results, tree};

pub(super) type Listed = Vec<(PathBuf, std::io::Result<tree::Listing>)>;

#[derive(Deserialize)]
struct Listing {
    folders: Vec<Folder>,
}

#[derive(Deserialize)]
struct Folder {
    entries: Vec<Entry>,
    error: Option<String>,
    more: bool,
}

#[derive(Deserialize)]
struct Entry {
    name: String,
    kind: String,
    bytes: Option<u64>,
    modified: Option<String>,
}

#[derive(Deserialize)]
struct Read {
    kind: String,
    bytes: Option<u64>,
    modified: Option<String>,
    text: Option<String>,
    image: Option<Image>,
}

#[derive(Deserialize)]
struct Image {
    data: String,
}

#[derive(Deserialize)]
struct Found {
    capped: bool,
    names: Vec<Name>,
    files: Vec<File>,
}

#[derive(Deserialize)]
struct Name {
    path: String,
    indices: Vec<u32>,
}

#[derive(Deserialize)]
struct File {
    path: String,
    lines: Vec<Line>,
}

#[derive(Deserialize)]
struct Line {
    number: u64,
    text: String,
    ranges: Vec<(u32, u32)>,
}

fn arg(rel: &Path) -> String {
    if rel.as_os_str().is_empty() {
        ".".to_string()
    } else if rel.is_absolute() {
        rel.display().to_string()
    } else {
        format!("./{}", rel.display())
    }
}

fn files_args(command: &str, workspace: u64, flags: &[&str], rest: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut args = vec!["files".to_string(), command.to_string(), "--workspace".to_string(), workspace.to_string()];
    args.extend(flags.iter().map(|f| (*f).to_string()));
    args.push("--json".to_string());
    args.push("--".to_string());
    args.extend(rest);
    args
}

fn answer(args: &[String]) -> Result<String> {
    let out = cli::command(args).stdin(Stdio::null()).output()?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    if stdout.trim_start().starts_with('{') {
        return Ok(stdout);
    }
    let err = String::from_utf8_lossy(&out.stderr);
    let err = err.trim().trim_start_matches("Error: ");
    bail!("{}", if err.is_empty() { "cornercase files failed" } else { err })
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let of_era = year - era * 400;
    let of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    era * 146_097 + of_era * 365 + of_era / 4 - of_era / 100 + of_year - 719_468
}

fn numbers<const N: usize>(text: &str, by: char) -> Option<[u64; N]> {
    let parts: Vec<u64> = text.split(by).map(str::parse).collect::<Result<_, _>>().ok()?;
    parts.try_into().ok()
}

pub(super) fn parse_utc(text: &str) -> Option<SystemTime> {
    let (date, time) = text.strip_suffix('Z')?.split_once('T')?;
    let (clock, fraction) = time.split_once('.').unwrap_or((time, "0"));
    let [year, month, day] = numbers::<3>(date, '-')?;
    let [hours, minutes, seconds] = numbers::<3>(clock, ':')?;
    let nanos: u32 = format!("{fraction:0<9}").get(..9)?.parse().ok()?;
    let days = days_from_civil(i64::try_from(year).ok()?, i64::try_from(month).ok()?, i64::try_from(day).ok()?);
    let secs = u64::try_from(days).ok()? * 86_400 + hours * 3_600 + minutes * 60 + seconds;
    Some(UNIX_EPOCH + Duration::new(secs, nanos))
}

fn stamp_of(bytes: Option<u64>, modified: Option<&str>) -> Stamp {
    Stamp { len: bytes.unwrap_or(0), modified: modified.and_then(parse_utc) }
}

fn folder_listing(folder: Folder) -> std::io::Result<tree::Listing> {
    if let Some(error) = folder.error {
        return Err(std::io::Error::other(error));
    }
    let mut entries: Vec<tree::Entry> =
        folder.entries.into_iter().map(|e| tree::Entry { dir: e.kind == "folder", name: e.name }).collect();
    tree::sort(&mut entries);
    Ok(tree::Listing { entries, truncated: folder.more })
}

pub(super) fn listings(folders: Vec<PathBuf>, json: Result<String>) -> Listed {
    let parsed = json.and_then(|json| Ok(serde_json::from_str::<Listing>(&json)?));
    match parsed {
        Ok(listing) => folders.into_iter().zip(listing.folders.into_iter().map(folder_listing)).collect(),
        Err(e) => {
            let text = e.to_string();
            folders.into_iter().map(|rel| (rel, Err(std::io::Error::other(text.clone())))).collect()
        }
    }
}

pub(super) fn list(workspace: u64, folders: Vec<PathBuf>) -> Listed {
    let json = answer(&files_args("list", workspace, &[], folders.iter().map(|rel| arg(rel))));
    listings(folders, json)
}

pub(super) fn stamp_in(json: &str, name: &str) -> Option<Stamp> {
    let listing = serde_json::from_str::<Listing>(json).ok()?;
    let entry = listing.folders.into_iter().next()?.entries.into_iter().find(|e| e.name == name)?;
    Some(stamp_of(entry.bytes, entry.modified.as_deref()))
}

pub(super) fn stamp(workspace: u64, rel: &Path) -> Option<Stamp> {
    let name = rel.file_name()?.to_string_lossy().into_owned();
    let parent = rel.parent().unwrap_or(Path::new(""));
    let json = answer(&files_args("list", workspace, &[], [arg(parent)])).ok()?;
    stamp_in(&json, &name)
}

fn bytes_of(read: Read) -> std::result::Result<Vec<u8>, String> {
    if let Some(image) = read.image {
        return base64::engine::general_purpose::STANDARD.decode(image.data).map_err(|e| e.to_string());
    }
    Ok(read.text.unwrap_or_default().into_bytes())
}

fn content_of(path: &Path, read: Read) -> Content {
    match read.kind.as_str() {
        "text" | "image" => match bytes_of(read) {
            Ok(bytes) => disk::classify(path, bytes),
            Err(e) => Content::Gone(e),
        },
        "binary" => Content::Binary,
        "too-large" => Content::TooLarge,
        "folder" => Content::NotRegular,
        "missing" => Content::Gone("No such file".to_string()),
        other => Content::Gone(format!("cornercase answered a file of kind {other}")),
    }
}

pub(super) fn content(path: &Path, json: Result<String>) -> (Content, Option<Stamp>) {
    match json.and_then(|json| Ok(serde_json::from_str::<Read>(&json)?)) {
        Ok(read) => {
            let stamp = (read.kind != "missing").then(|| stamp_of(read.bytes, read.modified.as_deref()));
            (content_of(path, read), stamp)
        }
        Err(e) => (Content::Gone(e.to_string()), None),
    }
}

pub(super) fn read(workspace: u64, rel: &Path) -> (Content, Option<Stamp>) {
    content(rel, answer(&files_args("read", workspace, &[], [arg(rel)])))
}

fn line_hit(line: Line) -> LineHit {
    let text = line.text.replace('\t', " ");
    let indices: Vec<u32> = line.ranges.into_iter().flat_map(|(start, end)| start..end).collect();
    let lit = search::char_ranges(&text, &indices);
    LineHit { line: line.number, text, lit }
}

fn results_of(found: Found, mode: Mode) -> Results {
    match mode {
        Mode::Names => Results::Names(
            found
                .names
                .into_iter()
                .map(|n| NameHit { lit: search::char_ranges(&n.path, &n.indices), path: n.path })
                .collect(),
        ),
        Mode::Text => Results::Text(TextResults {
            files: found
                .files
                .into_iter()
                .map(|f| FileHits { path: f.path, lines: f.lines.into_iter().map(line_hit).collect() })
                .collect(),
            capped: found.capped,
        }),
    }
}

pub(super) fn results(mode: Mode, json: Result<String>) -> Results {
    match json.and_then(|json| Ok(serde_json::from_str::<Found>(&json)?)) {
        Ok(found) => results_of(found, mode),
        Err(e) => Results::Failed(e.to_string()),
    }
}

pub(super) fn search(workspace: u64, query: &str, mode: Mode) -> Results {
    let flag = match mode {
        Mode::Names => "--names",
        Mode::Text => "--text",
    };
    results(mode, answer(&files_args("search", workspace, &[flag], [query.to_string()])))
}

#[cfg(test)]
mod tests;
