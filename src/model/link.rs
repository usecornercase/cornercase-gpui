use serde::Deserialize;
use url::{Host, Url};

use super::{Grid, PaneGrid};
use crate::platform::paths;

const SCHEMES: [&str; 2] = ["https", "http"];
const TRAILING: [&str; 3] = [".", ":", "?"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub(crate) struct Lines {
    pub first: usize,
    pub last: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct Link {
    pub path: String,
    #[serde(default)]
    pub inside: bool,
    pub workspace: Option<u64>,
    pub relative: Option<String>,
    pub lines: Option<Lines>,
    pub row: u16,
    pub start: u16,
    pub end: u16,
}

#[derive(Deserialize)]
struct Answer {
    link: Option<Link>,
}

impl Link {
    pub(crate) fn parse(json: &str) -> Option<Self> {
        serde_json::from_str::<Answer>(json).ok()?.link.filter(|link| link.start < link.end)
    }

    pub(crate) fn span(&self) -> Span {
        Span { row: self.row, start: self.start, end: self.end }
    }

    pub(crate) fn shown_path(&self) -> &str {
        self.relative.as_deref().filter(|_| self.inside).unwrap_or(&self.path)
    }

    pub(crate) fn style(&self) -> paths::Style {
        paths::Style::of_path(&self.path)
    }

    pub(crate) fn name(&self) -> &str {
        self.style().name(&self.path)
    }

    pub(crate) fn first_line(&self) -> Option<usize> {
        self.lines.map(|lines| lines.first.saturating_sub(1))
    }

    pub(crate) fn last_line(&self) -> Option<usize> {
        self.lines.map(|lines| lines.last.max(lines.first).saturating_sub(1))
    }

    pub(crate) fn same_file(&self, other: &Link) -> bool {
        self.path == other.path && self.lines == other.lines
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Span {
    pub row: u16,
    pub start: u16,
    pub end: u16,
}

impl Span {
    pub(crate) fn covers(self, (x, y): (u16, u16)) -> bool {
        y == self.row && (self.start..self.end).contains(&x)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    File(Link),
    Web(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hovered {
    pub pane: u64,
    pub span: Span,
    pub target: Target,
    pub text: String,
}

impl Hovered {
    pub(crate) fn new(pane: &PaneGrid, span: Span, target: Target) -> Self {
        let text = text_between(&row_cells(pane, span.row), usize::from(span.start), usize::from(span.end));
        Self { pane: pane.pane, span, target, text }
    }

    pub(crate) fn file(pane: &PaneGrid, link: Link) -> Self {
        Self::new(pane, link.span(), Target::File(link))
    }

    pub(crate) fn shown_in(&self, grid: &Grid) -> bool {
        let at = (self.span.row, self.span.start, self.span.end);
        grid.pane(self.pane).is_some_and(|pane| shows(pane, at, &self.text))
    }

    pub(crate) fn at(&self, pane: u64, cell: (u16, u16)) -> bool {
        self.pane == pane && self.span.covers(cell)
    }
}

pub(crate) fn is_web(url: &str) -> bool {
    let Some((scheme, rest)) = url.split_once("://") else { return false };
    let clean = !url.chars().any(|c| c.is_whitespace() || c.is_control());
    let named = SCHEMES.iter().any(|known| known.eq_ignore_ascii_case(scheme));
    named && clean && !written_host(rest).is_empty() && web_host(url).is_some()
}

fn written_host(rest: &str) -> &str {
    let authority = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
    let authority = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    match authority.rsplit_once(':') {
        Some((host, port)) if !port.contains(']') => host,
        _ => authority,
    }
}

fn web_host(url: &str) -> Option<(Host<String>, u16)> {
    let parsed = Url::parse(url).ok()?;
    let port = parsed.port_or_known_default()?;
    let host = parsed.host()?.to_owned();
    SCHEMES.contains(&parsed.scheme()).then_some((host, port))
}

fn loopback_host(host: &Host<String>) -> bool {
    match host {
        Host::Domain(name) => {
            let name = name.trim_end_matches('.');
            name == "localhost" || name.ends_with(".localhost")
        }
        Host::Ipv4(ip) => ip.is_loopback() || ip.is_unspecified(),
        Host::Ipv6(ip) => {
            let mapped = ip.to_ipv4_mapped().is_some_and(|v4| v4.is_loopback() || v4.is_unspecified());
            ip.is_loopback() || ip.is_unspecified() || mapped
        }
    }
}

pub(crate) fn loopback_port(url: &str) -> Option<u16> {
    let (host, port) = web_host(url)?;
    loopback_host(&host).then_some(port)
}

pub(crate) fn rewrite(url: &str, local: u16) -> String {
    let Some((scheme, rest)) = url.split_once("://") else { return url.to_string() };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = rest[..end].rsplit_once('@').map_or(&rest[..end], |(_, host)| host);
    let name = authority.rsplit_once(':').map_or(authority, |(host, _)| host);
    let host = if name.to_ascii_lowercase().ends_with(".localhost") { name } else { "localhost" };
    format!("{scheme}://{host}:{local}{}", &rest[end..])
}

pub(crate) fn is_loopback(url: &str) -> bool {
    loopback_port(url).is_some()
}

pub(crate) fn url_in(cells: &[&str], start: usize, end: usize) -> Option<(usize, usize)> {
    let scheme = |at: &usize| cells.get(*at).is_some_and(|cell| cell.eq_ignore_ascii_case("h"));
    let starts = |at: &usize| scheme(at) && is_web(&text_between(cells, *at, end));
    let from = (start..end).find(starts)?;
    let to = (from..end).rev().find(|at| cells.get(*at).is_some_and(|cell| !TRAILING.contains(cell)))? + 1;
    is_web(&text_between(cells, from, to)).then_some((from, to))
}

pub(crate) fn web_hyperlink(pane: &PaneGrid, (x, y): (u16, u16)) -> Option<(Span, String)> {
    let runs = pane.lines.get(usize::from(y))?;
    let links: Vec<Option<&str>> = runs.iter().flat_map(|run| run.cells.iter().map(|_| run.link.as_deref())).collect();
    let at = usize::from(x);
    let url = links.get(at).copied().flatten().filter(|url| is_web(url))?;
    let same = |i: &usize| links.get(*i).copied().flatten() == Some(url);
    let start = (0..at).rev().take_while(same).last().unwrap_or(at);
    let end = (at + 1..links.len()).take_while(same).last().unwrap_or(at) + 1;
    let span = Span { row: y, start: u16::try_from(start).ok()?, end: u16::try_from(end).ok()? };
    Some((span, url.to_string()))
}

pub(crate) fn row_cells(pane: &PaneGrid, row: u16) -> Vec<&str> {
    pane.lines
        .get(usize::from(row))
        .map(|line| line.iter().flat_map(|run| run.cells.iter().map(String::as_str)).collect())
        .unwrap_or_default()
}

pub(crate) fn text_between(cells: &[&str], start: usize, end: usize) -> String {
    cells.get(start..end.min(cells.len())).map(<[&str]>::concat).unwrap_or_default()
}

pub(crate) fn shows(pane: &PaneGrid, (row, start, end): (u16, u16, u16), text: &str) -> bool {
    text_between(&row_cells(pane, row), usize::from(start), usize::from(end)) == text
}

#[cfg(test)]
mod tests;
