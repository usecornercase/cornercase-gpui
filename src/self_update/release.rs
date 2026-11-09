use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::install::Way;
use crate::cli::Version;
use crate::notify::Notes;
use crate::platform::Os;

const FALLBACK_PAGE: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/releases");

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct Asset {
    pub name: String,
    #[serde(rename = "browser_download_url")]
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Latest {
    pub version: String,
    pub page: String,
    pub body: String,
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Found {
    pub latest: Latest,
    pub notes: Vec<Notes>,
    pub way: Way,
    pub on_disk: bool,
    pub changelog: bool,
}

#[derive(Deserialize)]
struct Raw {
    tag_name: Option<String>,
    html_url: Option<String>,
    body: Option<String>,
    #[serde(default)]
    assets: Vec<Asset>,
}

pub(crate) fn parse(answer: &Value) -> Option<Latest> {
    let raw = Raw::deserialize(answer).ok()?;
    let tag = raw.tag_name?;
    let version = tag.trim().trim_start_matches('v').to_string();
    (!version.is_empty()).then(|| Latest {
        version,
        page: raw.html_url.unwrap_or_else(|| FALLBACK_PAGE.to_string()),
        body: raw.body.unwrap_or_default(),
        assets: raw.assets,
    })
}

impl Latest {
    pub(crate) fn asset(&self, name: &str) -> Option<&Asset> {
        self.assets.iter().find(|asset| asset.name == name)
    }
}

fn rank(text: &str) -> Option<(Version, bool)> {
    Some((Version::parse(text)?, !text.contains('-')))
}

pub(crate) fn newer(latest: &str, current: &str) -> bool {
    match (rank(latest), rank(current)) {
        (Some(latest), Some(current)) => latest > current,
        _ => false,
    }
}

pub(crate) fn notes_since(changelog: &str, current: &str, latest: &str) -> Vec<Notes> {
    let (Some(current), Some(latest)) = (Version::parse(current), Version::parse(latest)) else { return Vec::new() };
    sections(changelog)
        .into_iter()
        .filter(|(heading, text)| {
            !text.is_empty() && Version::parse(heading).is_some_and(|version| current < version && version <= latest)
        })
        .map(|(version, text)| Notes { version, text })
        .collect()
}

fn sections(changelog: &str) -> Vec<(String, String)> {
    let mut found: Vec<(String, Vec<&str>)> = Vec::new();
    for line in changelog.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            let version = heading.split_whitespace().next().unwrap_or_default();
            found.push((version.to_string(), Vec::new()));
        } else if let Some((_, lines)) = found.last_mut() {
            lines.push(line);
        }
    }
    found.into_iter().map(|(version, lines)| (version, lines.join("\n").trim().to_string())).collect()
}

pub(crate) fn body_notes(version: &str, body: &str) -> Vec<Notes> {
    let text = body.trim();
    if text.is_empty() { Vec::new() } else { vec![Notes { version: version.to_string(), text: text.to_string() }] }
}

pub(crate) fn asset_name(os: Os, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        (Os::Mac, _) => Some("cornercase-gpui-universal-apple-darwin.dmg"),
        (Os::Windows, "x86_64") => Some("cornercase-gpui-x86_64-pc-windows-msvc.zip"),
        (Os::Linux, "x86_64") => Some("cornercase-gpui-x86_64-unknown-linux-gnu.tar.gz"),
        _ => None,
    }
}

pub(crate) fn sums_name(asset: &str) -> String {
    format!("{asset}.sha256")
}

pub(crate) fn matches(bytes: &[u8], sums: &str) -> bool {
    let expected = sums.split_whitespace().next().unwrap_or_default().to_lowercase();
    !expected.is_empty() && expected == format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn version_line() -> String {
    format!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
}

pub(crate) fn printed_version(line: &str) -> Option<&str> {
    line.trim().strip_prefix(concat!(env!("CARGO_PKG_NAME"), " "))
}

pub(crate) fn source_command(version: &str) -> String {
    format!("cargo install --locked --git {} --tag v{version}", env!("CARGO_PKG_REPOSITORY"))
}

#[cfg(test)]
mod tests;
