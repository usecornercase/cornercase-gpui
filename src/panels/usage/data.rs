use std::collections::BTreeMap;

use serde::Deserialize;

use crate::panels::flows::model::days_from_civil;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Plans {
    pub agents: Vec<Plan>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Plan {
    pub agent: String,
    pub title: String,
    pub loading: bool,
    pub read_at: Option<String>,
    #[serde(rename = "plan")]
    pub tier: Option<String>,
    pub limited: Option<bool>,
    pub limits: Vec<Limit>,
    pub extra: Option<String>,
    pub notice: Option<String>,
    pub error: Option<String>,
    pub account: Option<String>,
    pub label: Option<String>,
    pub active: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Limit {
    pub label: String,
    pub kind: String,
    pub scope: Option<String>,
    pub percent: f64,
    pub severity: Option<String>,
    pub resets_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Severity {
    Normal,
    Warning,
    Critical,
}

impl Limit {
    pub(super) fn severity(&self) -> Severity {
        match self.severity.as_deref() {
            Some("normal") => Severity::Normal,
            Some("warning") => Severity::Warning,
            Some("critical") => Severity::Critical,
            _ if self.percent >= 90. => Severity::Critical,
            _ if self.percent >= 75. => Severity::Warning,
            _ => Severity::Normal,
        }
    }

    pub(super) fn fraction(&self) -> f32 {
        ratio(self.percent, 100.).clamp(0., 1.)
    }
}

impl Plan {
    pub(super) fn heading(&self) -> String {
        let title = if self.active { format!("● {}", self.title) } else { self.title.clone() };
        let account = self.label.as_deref().or(self.account.as_deref()).map(str::to_string);
        let tier = self.tier.as_deref().filter(|p| !p.is_empty()).map(|p| format!("{p} plan"));
        std::iter::once(title).chain(account).chain(tier).collect::<Vec<_>>().join(" · ")
    }

    pub(super) fn status(&self, now: i64) -> String {
        if self.loading {
            return "loading…".into();
        }
        match self.read_at.as_deref().and_then(parse_utc) {
            Some(at) => updated((now - at).max(0)),
            None => String::new(),
        }
    }

    pub(super) fn unasked(&self) -> bool {
        !self.loading && self.read_at.is_none() && self.error.is_none() && self.notice.is_none()
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Stats {
    pub since: Option<String>,
    pub windows: BTreeMap<String, Window>,
    pub blocks: Vec<Block>,
    pub merged_pull_requests: Option<Merged>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Window {
    #[serde(flatten)]
    pub counts: Counts,
    pub busy_seconds: f64,
    pub most_at_once: u64,
    pub by_agent: BTreeMap<String, Counts>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(super) struct Counts {
    pub agents: u64,
    pub prompts: u64,
    pub turns: u64,
    pub agent_seconds: f64,
    pub waited_seconds: f64,
    pub tokens: Tokens,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Tokens {
    #[serde(rename = "in")]
    pub input: u64,
    pub out: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl Tokens {
    pub(super) fn sent(&self) -> u64 {
        self.input.saturating_add(self.cache_write)
    }
}

impl Counts {
    pub(super) fn is_empty(&self) -> bool {
        *self == Counts::default()
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Block {
    pub agent_seconds: f64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Merged {
    #[serde(rename = "24h")]
    pub day: u64,
    #[serde(rename = "7d")]
    pub week: u64,
    pub total: u64,
    pub complete: bool,
    pub accounts: Vec<MergedAccount>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct MergedAccount {
    pub provider: String,
    pub host: String,
    pub login: Option<String>,
    pub error: Option<String>,
}

pub(super) const WINDOWS: [(&str, &str); 3] = [("24h", "24 h"), ("7d", "7 days"), ("total", "total")];

pub(super) const WEEKS: usize = 26;

const AGENTS: [(&str, &str); 7] = [
    ("claude", "Claude Code"),
    ("codex", "Codex"),
    ("gemini", "Gemini CLI"),
    ("opencode", "opencode"),
    ("grok", "Grok Build"),
    ("cursor", "Cursor CLI"),
    ("copilot", "GitHub Copilot CLI"),
];

pub(super) fn agent_title(key: &str) -> String {
    AGENTS.iter().find(|(k, _)| *k == key).map_or_else(|| key.to_string(), |(_, title)| (*title).to_string())
}

fn agent_rank(key: &str) -> usize {
    AGENTS.iter().position(|(k, _)| *k == key).unwrap_or(AGENTS.len())
}

impl Stats {
    pub(super) fn window(&self, key: &str) -> Window {
        self.windows.get(key).cloned().unwrap_or_default()
    }

    pub(super) fn empty(&self) -> bool {
        let total = self.window("total");
        total.counts.is_empty() && total.busy_seconds == 0. && total.most_at_once == 0
    }

    pub(super) fn parallel(&self) -> Option<(f64, f64, f64)> {
        let w = self.window("24h");
        (w.busy_seconds >= 60.)
            .then(|| (w.counts.agent_seconds / w.busy_seconds, w.counts.agent_seconds, w.busy_seconds))
    }

    pub(super) fn rows(&self) -> Vec<(&'static str, [String; 3])> {
        let w = WINDOWS.map(|(key, _)| self.window(key));
        let row = |f: &dyn Fn(&Window) -> String| [f(&w[0]), f(&w[1]), f(&w[2])];
        let mut rows = vec![
            ("agents started", row(&|w| count(w.counts.agents))),
            ("prompts", row(&|w| count(w.counts.prompts))),
            ("turns", row(&|w| count(w.counts.turns))),
            ("agent time", row(&|w| duration(w.counts.agent_seconds))),
            ("most at once", row(&|w| count(w.most_at_once))),
            ("waited for you", row(&|w| duration(w.counts.waited_seconds))),
            ("tokens out", row(&|w| tokens(w.counts.tokens.out))),
            ("tokens in", row(&|w| tokens(w.counts.tokens.sent()))),
            ("tokens from cache", row(&|w| tokens(w.counts.tokens.cache_read))),
        ];
        if let Some(m) = &self.merged_pull_requests {
            let plus = if m.complete { "" } else { "+" };
            rows.push(("merged PRs", [m.day, m.week, m.total].map(|n| format!("{}{plus}", count(n)))));
        }
        rows
    }

    pub(super) fn days(&self, now: i64) -> u64 {
        self.since
            .as_deref()
            .and_then(parse_utc)
            .map_or(0, |since| u64::try_from((now - since).max(0) / 86_400).unwrap_or(0))
    }

    pub(super) fn agents(&self, key: &str) -> Vec<(String, Counts)> {
        let mut list: Vec<(String, Counts)> =
            self.window(key).by_agent.into_iter().filter(|(_, c)| !c.is_empty()).collect();
        list.sort_by(|a, b| agent_rank(&a.0).cmp(&agent_rank(&b.0)).then_with(|| a.0.cmp(&b.0)));
        list
    }

    pub(super) fn levels(&self) -> Vec<u8> {
        let shown = WEEKS * 7;
        let seconds = |i: usize| self.blocks.get(i).map_or(0., |b| b.agent_seconds);
        let mut positive: Vec<f64> = (0..shown).map(seconds).filter(|n| *n > 0.).collect();
        positive.sort_by(f64::total_cmp);
        let thresholds =
            [1, 2, 3].map(|q| positive.get((positive.len() * q).div_ceil(4).saturating_sub(1)).copied().unwrap_or(0.));
        (0..shown)
            .map(|i| {
                let n = seconds(i);
                if n <= 0. {
                    return 0;
                }
                1 + u8::try_from(thresholds.iter().filter(|&&t| n > t).count()).unwrap_or(u8::MAX)
            })
            .collect()
    }
}

#[expect(clippy::cast_possible_truncation, reason = "a share drawn on screen needs no f64 precision")]
pub(super) fn ratio(part: f64, whole: f64) -> f32 {
    (part / whole) as f32
}

pub(super) fn counting(days: u64) -> String {
    format!("counting for {days} day{}", if days == 1 { "" } else { "s" })
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "whole minutes of a duration clamped at zero"
)]
pub(super) fn duration(secs: f64) -> String {
    let minutes = (secs.max(0.) / 60.) as u64;
    let (h, m) = (minutes / 60, minutes % 60);
    if h == 0 {
        format!("{m}m")
    } else if m == 0 {
        format!("{h}h")
    } else {
        format!("{h}h {m:02}m")
    }
}

#[expect(clippy::cast_precision_loss, reason = "an abbreviated count keeps at most one decimal")]
fn scaled(n: u64, scale: u64) -> f64 {
    n as f64 / scale as f64
}

fn abbreviated(n: u64, units: &[(u64, &str)]) -> String {
    let Some(&(scale, suffix)) = units.iter().find(|(scale, _)| n >= *scale) else { return n.to_string() };
    let value = scaled(n, scale);
    if value < 10. { format!("{value:.1}{suffix}") } else { format!("{value:.0}{suffix}") }
}

pub(super) fn count(n: u64) -> String {
    if n < 10_000 {
        n.to_string()
    } else if n < 1_000_000 {
        format!("{:.1}k", scaled(n, 1_000))
    } else {
        abbreviated(n, &[(1_000_000_000, "B"), (1_000_000, "M")])
    }
}

pub(super) fn tokens(n: u64) -> String {
    abbreviated(n, &[(1_000_000_000, "B"), (1_000_000, "M"), (1_000, "k")])
}

pub(super) fn until(secs: i64) -> String {
    let minutes = (secs.max(0) + 59) / 60;
    let (days, hours, minutes) = (minutes / 1_440, minutes / 60 % 24, minutes % 60);
    if days > 0 {
        format!("resets in {days}d {hours}h")
    } else if hours > 0 {
        format!("resets in {hours}h {minutes}m")
    } else {
        format!("resets in {minutes}m")
    }
}

pub(super) fn updated(secs: i64) -> String {
    let minutes = secs.max(0) / 60;
    let (hours, days) = (minutes / 60, minutes / 1_440);
    if minutes == 0 {
        "updated just now".into()
    } else if hours == 0 {
        format!("updated {minutes}m ago")
    } else if days == 0 {
        format!("updated {hours}h ago")
    } else {
        format!("updated {days}d ago")
    }
}

pub(super) fn parse_utc(text: &str) -> Option<i64> {
    let text = text.strip_suffix('Z')?;
    let (date, time) = text.split_once('T')?;
    let mut date = date.splitn(3, '-').map(str::parse::<i64>);
    let (y, mo, d) = (date.next()?.ok()?, date.next()?.ok()?, date.next()?.ok()?);
    let time = time.split('.').next()?;
    let mut time = time.splitn(3, ':').map(str::parse::<i64>);
    let (h, mi, s) = (time.next()?.ok()?, time.next()?.ok()?, time.next()?.ok()?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 60 {
        return None;
    }
    Some(days_from_civil(y, mo, d) * 86_400 + h * 3_600 + mi * 60 + s)
}

pub(super) fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests;
