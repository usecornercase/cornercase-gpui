use serde::Deserialize;

use crate::panels::queue::{days, now};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Found {
    pub(super) matches: Vec<Hit>,
    pub(super) capped: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Hit {
    pub(super) pane: u64,
    pub(super) place: String,
    pub(super) who: String,
    pub(super) source: String,
    pub(super) line: Option<u64>,
    pub(super) text: String,
    pub(super) written: Option<String>,
}

impl Hit {
    pub(super) fn when(&self) -> String {
        match (&self.written, self.line) {
            (Some(written), _) => parse_time(written).map(ago).unwrap_or_default(),
            (None, Some(line)) => format!("line {line}"),
            (None, None) => String::new(),
        }
    }
}

pub(super) struct PaneHits<'a> {
    pub(super) pane: u64,
    pub(super) place: &'a str,
    pub(super) who: &'a str,
    pub(super) hits: Vec<&'a Hit>,
}

pub(super) fn by_pane(matches: &[Hit]) -> Vec<PaneHits<'_>> {
    let mut out: Vec<PaneHits> = Vec::new();
    for hit in matches {
        match out.iter_mut().find(|g| g.pane == hit.pane) {
            Some(group) => group.hits.push(hit),
            None => out.push(PaneHits { pane: hit.pane, place: &hit.place, who: &hit.who, hits: vec![hit] }),
        }
    }
    out
}

pub(super) fn row_of_match(groups: &[PaneHits], n: usize) -> usize {
    let mut row = 0;
    let mut seen = 0;
    for group in groups {
        row += 1;
        if n < seen + group.hits.len() {
            return row + n - seen;
        }
        row += group.hits.len();
        seen += group.hits.len();
    }
    row.saturating_sub(1)
}

pub(super) fn parse_time(text: &str) -> Option<i64> {
    let (date, time) = text.split_once(['T', ' '])?;
    let (clock, offset) = match time.find(['Z', 'z', '+', '-']) {
        Some(at) => time.split_at(at),
        None => (time, "Z"),
    };
    Some(date_days(date)? * 86_400 + clock_seconds(clock)? - offset_seconds(offset)?)
}

fn date_days(date: &str) -> Option<i64> {
    let mut parts = date.splitn(3, '-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: u32 = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    Some(days(year, month, day))
}

fn clock_seconds(clock: &str) -> Option<i64> {
    let mut parts = clock.split(':');
    let hours: i64 = parts.next()?.parse().ok()?;
    let minutes: i64 = parts.next()?.parse().ok()?;
    let seconds: i64 = match parts.next() {
        Some(seconds) => seconds.split('.').next()?.parse().ok()?,
        None => 0,
    };
    Some(hours * 3600 + minutes * 60 + seconds)
}

fn offset_seconds(offset: &str) -> Option<i64> {
    let (sign, rest) = if let Some(rest) = offset.strip_prefix('+') {
        (1, rest)
    } else if let Some(rest) = offset.strip_prefix('-') {
        (-1, rest)
    } else {
        return Some(0);
    };
    let (hours, minutes) = rest.split_once(':').unwrap_or((rest, "0"));
    Some(sign * (hours.parse::<i64>().ok()? * 3600 + minutes.parse::<i64>().ok()? * 60))
}

fn ago(at: i64) -> String {
    ago_from(at, now())
}

pub(super) fn ago_from(at: i64, now: i64) -> String {
    let secs = (now - at).max(0);
    match secs {
        0..60 => "just now".into(),
        60..3600 => format!("{}m ago", secs / 60),
        3600..86_400 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86_400),
    }
}
