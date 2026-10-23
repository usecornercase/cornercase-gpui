//! What the Usage sheet reads: `cornercase usage --json` (plan limits per agent) and
//! `cornercase stats --json` (counters per window and agent, and the heatmap blocks), parsed
//! leniently, and the small formatting rules the TUI uses for them (durations, counts, tokens,
//! reset times, heatmap levels), so both clients print the same numbers.

use std::collections::BTreeMap;

use serde::Deserialize;

/// `cornercase usage --json`: one entry per agent (and per account, with agent accounts).
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
    /// When the numbers were read, UTC.
    pub read_at: Option<String>,
    pub plan: Option<String>,
    /// `None` before anything was read.
    pub limited: Option<bool>,
    pub limits: Vec<Limit>,
    /// Extra usage or credits, as text.
    pub extra: Option<String>,
    /// Why there are no limits to show (not signed in, no plan limits), in grey.
    pub notice: Option<String>,
    pub error: Option<String>,
    /// With agent accounts: the account's name, its label and whether it is the one in use.
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
    /// The server's severity, or one from the percentage (75 and 90, as cornercase) when it
    /// sends a name this client does not know.
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

    /// The share used, from 0 to 1, for the bar.
    pub(super) fn fraction(&self) -> f32 {
        (self.percent / 100.).clamp(0., 1.) as f32
    }
}

impl Plan {
    /// The section's title as the TUI writes it: `● Claude Code · work · team plan`.
    pub(super) fn heading(&self) -> String {
        let mut text = if self.active { format!("● {}", self.title) } else { self.title.clone() };
        if let Some(account) = self.label.as_deref().or(self.account.as_deref()) {
            text.push_str(" · ");
            text.push_str(account);
        }
        if let Some(plan) = self.plan.as_deref().filter(|p| !p.is_empty()) {
            text.push_str(&format!(" · {plan} plan"));
        }
        text
    }

    /// `loading…`, `updated 2m ago`, or nothing before the first answer.
    pub(super) fn status(&self, now: i64) -> String {
        if self.loading {
            return "loading…".into();
        }
        match self.read_at.as_deref().and_then(parse_utc) {
            Some(at) => updated((now - at).max(0)),
            None => String::new(),
        }
    }

    /// Nothing read, nothing to say: the probe has not run.
    pub(super) fn unasked(&self) -> bool {
        !self.loading && self.read_at.is_none() && self.error.is_none() && self.notice.is_none()
    }
}

/// `cornercase stats --json`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Stats {
    /// When counting began, UTC.
    pub since: Option<String>,
    /// `24h`, `7d` and `total`.
    pub windows: BTreeMap<String, Window>,
    /// The 24-hour blocks, newest first.
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
    /// `tokens in` as the TUI counts them: uncached input plus cache writes.
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
    /// False when an account could not be asked: the counts are a lower bound.
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

/// The three windows, in the order the table shows them, with their headings.
pub(super) const WINDOWS: [(&str, &str); 3] = [("24h", "24 h"), ("7d", "7 days"), ("total", "total")];

/// Weeks of heatmap blocks, seven to a column.
pub(super) const WEEKS: usize = 26;

/// The agents cornercase counts, in its order, with their names.
const AGENTS: [(&str, &str); 7] = [
    ("claude", "Claude Code"),
    ("codex", "Codex"),
    ("gemini", "Gemini CLI"),
    ("opencode", "opencode"),
    ("grok", "Grok Build"),
    ("cursor", "Cursor CLI"),
    ("copilot", "GitHub Copilot CLI"),
];

/// An agent's name for people, from its key in `by_agent`.
pub(super) fn agent_title(key: &str) -> String {
    AGENTS.iter().find(|(k, _)| *k == key).map_or_else(|| key.to_string(), |(_, title)| (*title).to_string())
}

/// Where an agent sits in cornercase's order (unknown ones last, by name).
fn agent_rank(key: &str) -> usize {
    AGENTS.iter().position(|(k, _)| *k == key).unwrap_or(AGENTS.len())
}

impl Stats {
    pub(super) fn window(&self, key: &str) -> Window {
        self.windows.get(key).cloned().unwrap_or_default()
    }

    /// Nothing counted yet, ever.
    pub(super) fn empty(&self) -> bool {
        let total = self.window("total");
        total.counts.is_empty() && total.busy_seconds == 0. && total.most_at_once == 0
    }

    /// `1.0× in parallel in the last 24 h`: the factor, the agent time and the busy time, or
    /// `None` below a minute of busy time.
    pub(super) fn parallel(&self) -> Option<(f64, f64, f64)> {
        let w = self.window("24h");
        (w.busy_seconds >= 60.)
            .then(|| (w.counts.agent_seconds / w.busy_seconds, w.counts.agent_seconds, w.busy_seconds))
    }

    /// The table's rows: a label and its value in each window, as the TUI shows them.
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

    /// Whole days since counting began.
    pub(super) fn days(&self, now: i64) -> u64 {
        self.since
            .as_deref()
            .and_then(parse_utc)
            .map_or(0, |since| u64::try_from((now - since).max(0) / 86_400).unwrap_or(0))
    }

    /// The agents that did something in a window, in cornercase's order.
    pub(super) fn agents(&self, key: &str) -> Vec<(String, Counts)> {
        let mut list: Vec<(String, Counts)> =
            self.window(key).by_agent.into_iter().filter(|(_, c)| !c.is_empty()).collect();
        list.sort_by(|a, b| agent_rank(&a.0).cmp(&agent_rank(&b.0)).then_with(|| a.0.cmp(&b.0)));
        list
    }

    /// Each heatmap block's level, newest first: 0 for no agent time, else 1 to 4 by the
    /// nearest-rank quartiles of the positive blocks shown, ties sharing their level.
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
                1 + thresholds.iter().filter(|&&t| n > t).count() as u8
            })
            .collect()
    }
}

/// `counting for 3 days`.
pub(super) fn counting(days: u64) -> String {
    format!("counting for {days} day{}", if days == 1 { "" } else { "s" })
}

/// Minutes, or hours and minutes: `24m`, `2h`, `3h 05m`.
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

fn abbreviated(n: u64, units: &[(u64, &str)]) -> String {
    for &(scale, suffix) in units {
        if n >= scale {
            let value = n as f64 / scale as f64;
            return if value < 10. { format!("{value:.1}{suffix}") } else { format!("{value:.0}{suffix}") };
        }
    }
    n.to_string()
}

/// Counts stay whole below 10000, then `12.3k`, `4.5M`.
pub(super) fn count(n: u64) -> String {
    if n < 10_000 {
        n.to_string()
    } else if n < 1_000_000 {
        format!("{:.1}k", n as f64 / 1_000.)
    } else {
        abbreviated(n, &[(1_000_000_000, "B"), (1_000_000, "M")])
    }
}

/// Tokens in k, M or B.
pub(super) fn tokens(n: u64) -> String {
    abbreviated(n, &[(1_000_000_000, "B"), (1_000_000, "M"), (1_000, "k")])
}

/// `resets in 2h 13m`, `resets in 3d 4h`.
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

/// `updated just now`, `updated 5m ago`, `updated 2h ago`.
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

/// Unix seconds from cornercase's UTC stamps (`2026-10-23T22:30:00.000Z`).
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

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Now, in Unix seconds.
pub(super) fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    const USAGE: &str = r#"{"agents": [
        {"age": 0, "agent": "claude", "error": null, "extra": null, "limited": true,
         "limits": [
           {"kind": "session", "label": "session (5h)", "percent": 9, "resets_at": "2026-10-23T22:30:00.000Z", "scope": null, "severity": "normal"},
           {"kind": "weekly_all", "label": "week", "percent": 83, "resets_at": "2026-10-25T16:00:00.000Z", "scope": null, "severity": "warning"},
           {"kind": "weekly_scoped", "label": "week · Fable", "percent": 95, "resets_at": null, "scope": "Fable", "severity": "shiny"}
         ],
         "loading": false, "notice": null, "plan": "team", "read_at": "2026-10-23T18:09:59.530Z", "title": "Claude Code"},
        {"age": null, "agent": "codex", "error": "Invalid request", "extra": null, "limited": null, "limits": [],
         "loading": false, "notice": null, "plan": null, "read_at": null, "title": "Codex",
         "account": "work", "label": "Work", "active": true, "future": [1, 2]}
    ]}"#;

    #[test]
    fn parses_usage() {
        let plans: Plans = serde_json::from_str(USAGE).expect("usage parses");
        assert_eq!(plans.agents.len(), 2);
        let claude = &plans.agents[0];
        assert_eq!(claude.heading(), "Claude Code · team plan");
        assert_eq!(claude.limits.len(), 3);
        assert_eq!(claude.limits[0].severity(), Severity::Normal);
        assert_eq!(claude.limits[1].severity(), Severity::Warning);
        // An unknown severity comes from the percentage.
        assert_eq!(claude.limits[2].severity(), Severity::Critical);
        assert!((claude.limits[1].fraction() - 0.83).abs() < 1e-6);
        let read = parse_utc("2026-10-23T18:09:59.530Z").expect("stamp");
        assert_eq!(claude.status(read + 30), "updated just now");
        assert_eq!(claude.status(read + 300), "updated 5m ago");
        let codex = &plans.agents[1];
        assert_eq!(codex.heading(), "● Codex · Work");
        assert_eq!(codex.error.as_deref(), Some("Invalid request"));
        assert_eq!(codex.status(read), "");
        assert!(!codex.unasked());
    }

    #[test]
    fn parses_partial_usage() {
        let plans: Plans =
            serde_json::from_str(r#"{"agents": [{"agent": "claude", "title": "Claude Code", "loading": true}]}"#)
                .expect("parses");
        assert_eq!(plans.agents[0].status(0), "loading…");
        let plans: Plans =
            serde_json::from_str(r#"{"agents": [{"agent": "codex", "title": "Codex"}]}"#).expect("parses");
        assert!(plans.agents[0].unasked());
    }

    fn counts(agents: u64, agent_seconds: f64) -> String {
        format!(
            r#"{{"agents": {agents}, "prompts": 6, "turns": 5, "agent_seconds": {agent_seconds}, "waited_seconds": 0.0,
               "tokens": {{"in": 308, "out": 153674, "cache_read": 23372916, "cache_write": 719741}}}}"#
        )
    }

    fn stats_json(merged: &str) -> String {
        let zero = r#"{"agents": 0, "prompts": 0, "turns": 0, "agent_seconds": 0.0, "waited_seconds": 0.0,
                       "tokens": {"in": 0, "out": 0, "cache_read": 0, "cache_write": 0}}"#;
        let window = |busy: f64| {
            format!(
                r#"{{"agents": 1, "prompts": 6, "turns": 5, "agent_seconds": 2957.0, "waited_seconds": 0.0,
                   "tokens": {{"in": 308, "out": 153674, "cache_read": 23372916, "cache_write": 719741}},
                   "busy_seconds": {busy}, "most_at_once": 2,
                   "by_agent": {{"opencode": {zero}, "codex": {codex}, "claude": {claude}, "zed": {zero}}}}}"#,
                codex = counts(1, 1000.),
                claude = counts(2, 1957.),
            )
        };
        let mut blocks: Vec<String> = vec![r#"{"agent_seconds": 0.0}"#.into(); 182];
        for (i, s) in [(0, 100.), (1, 200.), (2, 300.), (3, 400.), (9, 400.)] {
            blocks[i] = format!(r#"{{"agent_seconds": {s}}}"#);
        }
        format!(
            r#"{{"since": "2026-10-20T17:35:31.453Z", "windows": {{"24h": {}, "7d": {}, "total": {}}},
                "blocks": [{}] {merged}}}"#,
            window(1478.5),
            window(30.),
            window(1478.5),
            blocks.join(",")
        )
    }

    #[test]
    fn parses_stats() {
        let merged = r#", "merged_pull_requests": {"24h": 0, "7d": 3, "total": 12000, "complete": false,
                         "accounts": [{"provider": "GitHub", "host": "github.com", "login": "me", "error": "offline"}]}"#;
        let stats: Stats = serde_json::from_str(&stats_json(merged)).expect("stats parse");
        assert!(!stats.empty());
        let (factor, agent, busy) = stats.parallel().expect("busy for more than a minute");
        assert!((factor - 2957. / 1478.5).abs() < 1e-9);
        assert_eq!((duration(agent), duration(busy)), ("49m".into(), "24m".into()));
        let rows = stats.rows();
        assert_eq!(rows.len(), 10);
        assert_eq!(rows[0], ("agents started", ["1".into(), "1".into(), "1".into()]));
        assert_eq!(rows[3].1[0], "49m");
        assert_eq!(rows[4].1[0], "2");
        assert_eq!(rows[6].1[0], "154k");
        assert_eq!(rows[7].1[0], "720k");
        assert_eq!(rows[8].1[0], "23M");
        assert_eq!(rows[9], ("merged PRs", ["0+".into(), "3+".into(), "12.0k+".into()]));
        let since = parse_utc("2026-10-20T17:35:31.453Z").expect("stamp");
        assert_eq!(stats.days(since + 3 * 86_400 + 5), 3);
        // Idle agents are left out; the rest keep cornercase's order.
        let agents: Vec<String> = stats.agents("24h").into_iter().map(|(k, _)| k).collect();
        assert_eq!(agents, ["claude", "codex"]);
        assert_eq!(stats.merged_pull_requests.as_ref().map(|m| m.accounts.len()), Some(1));
    }

    #[test]
    fn idle_day_has_no_headline() {
        let stats: Stats = serde_json::from_str(&stats_json("")).expect("stats parse");
        assert!(stats.merged_pull_requests.is_none());
        assert_eq!(stats.rows().len(), 9);
        let mut quiet = stats.clone();
        quiet.windows.insert("24h".into(), Window { busy_seconds: 59., ..Window::default() });
        assert!(quiet.parallel().is_none());
        assert!(Stats::default().empty());
        assert!(Stats::default().parallel().is_none());
    }

    #[test]
    fn heatmap_levels_by_quartile() {
        let stats: Stats = serde_json::from_str(&stats_json("")).expect("stats parse");
        let levels = stats.levels();
        assert_eq!(levels.len(), 182);
        // Positive blocks 100, 200, 300, 400, 400: thresholds 200, 300, 400.
        assert_eq!(&levels[..4], &[1, 1, 2, 3]);
        assert_eq!(levels[9], 3);
        assert_eq!(levels[4], 0);
        assert!(Stats::default().levels().iter().all(|l| *l == 0));
    }

    #[test]
    fn formats_like_the_tui() {
        assert_eq!(duration(0.), "0m");
        assert_eq!(duration(7200.), "2h");
        assert_eq!(duration(3.0 * 3600. + 5. * 60.), "3h 05m");
        assert_eq!(count(9999), "9999");
        assert_eq!(count(12_345), "12.3k");
        assert_eq!(count(4_500_000), "4.5M");
        assert_eq!(tokens(999), "999");
        assert_eq!(tokens(153_674), "154k");
        assert_eq!(tokens(1_500), "1.5k");
        assert_eq!(until(0), "resets in 0m");
        assert_eq!(until(61), "resets in 2m");
        assert_eq!(until(2 * 3600 + 13 * 60), "resets in 2h 13m");
        assert_eq!(until(3 * 86_400 + 4 * 3600), "resets in 3d 4h");
        assert_eq!(updated(7200), "updated 2h ago");
        assert_eq!(counting(1), "counting for 1 day");
        assert_eq!(counting(0), "counting for 0 days");
        assert_eq!(agent_title("claude"), "Claude Code");
        assert_eq!(agent_title("zed"), "zed");
    }

    #[test]
    fn parses_utc_stamps() {
        assert_eq!(parse_utc("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_utc("2000-03-01T00:00:00.000Z"), Some(951_868_800));
        assert_eq!(parse_utc("2026-10-23T22:30:00.000Z"), Some(1_792_794_600));
        assert_eq!(parse_utc("2026-10-23 22:30:00"), None);
        assert_eq!(parse_utc("2026-13-23T22:30:00Z"), None);
        assert_eq!(parse_utc("garbage"), None);
    }
}
