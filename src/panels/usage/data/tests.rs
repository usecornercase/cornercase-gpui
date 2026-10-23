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
    assert_eq!((claude.heading().as_str(), claude.limits.len()), ("Claude Code · team plan", 3));
    let severities: Vec<Severity> = claude.limits.iter().map(Limit::severity).collect();
    assert_eq!(severities, [Severity::Normal, Severity::Warning, Severity::Critical]);
    assert!((claude.limits[1].fraction() - 0.83).abs() < 1e-6);
    let read = parse_utc("2026-10-23T18:09:59.530Z").expect("stamp");
    assert_eq!(
        (claude.status(read + 30), claude.status(read + 300)),
        ("updated just now".into(), "updated 5m ago".into())
    );
    let codex = &plans.agents[1];
    assert_eq!(
        (codex.heading().as_str(), codex.error.as_deref(), codex.status(read).as_str(), codex.unasked()),
        ("● Codex · Work", Some("Invalid request"), "", false)
    );
}

#[test]
fn parses_partial_usage() {
    let plans: Plans =
        serde_json::from_str(r#"{"agents": [{"agent": "claude", "title": "Claude Code", "loading": true}]}"#)
            .expect("parses");
    assert_eq!(plans.agents[0].status(0), "loading…");
    let plans: Plans = serde_json::from_str(r#"{"agents": [{"agent": "codex", "title": "Codex"}]}"#).expect("parses");
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
    assert_eq!((rows.len(), &rows[0]), (10, &("agents started", ["1".into(), "1".into(), "1".into()])));
    let firsts: Vec<&str> = [3, 4, 6, 7, 8].iter().map(|i| rows[*i].1[0].as_str()).collect();
    assert_eq!(firsts, ["49m", "2", "154k", "720k", "23M"]);
    assert_eq!(rows[9], ("merged PRs", ["0+".into(), "3+".into(), "12.0k+".into()]));
    let since = parse_utc("2026-10-20T17:35:31.453Z").expect("stamp");
    assert_eq!(stats.days(since + 3 * 86_400 + 5), 3);
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
    assert_eq!(&levels[..4], &[1, 1, 2, 3]);
    assert_eq!(levels[9], 3);
    assert_eq!(levels[4], 0);
    assert!(Stats::default().levels().iter().all(|l| *l == 0));
}

#[test]
fn formats_like_the_tui() {
    let formatted = [
        (duration(0.), "0m"),
        (duration(7200.), "2h"),
        (duration(3.0 * 3600. + 5. * 60.), "3h 05m"),
        (count(9999), "9999"),
        (count(12_345), "12.3k"),
        (count(4_500_000), "4.5M"),
        (tokens(999), "999"),
        (tokens(153_674), "154k"),
        (tokens(1_500), "1.5k"),
        (until(0), "resets in 0m"),
        (until(61), "resets in 2m"),
        (until(2 * 3600 + 13 * 60), "resets in 2h 13m"),
        (until(3 * 86_400 + 4 * 3600), "resets in 3d 4h"),
        (updated(7200), "updated 2h ago"),
        (counting(1), "counting for 1 day"),
        (counting(0), "counting for 0 days"),
        (agent_title("claude"), "Claude Code"),
        (agent_title("zed"), "zed"),
    ];
    for (got, want) in formatted {
        assert_eq!(got, want);
    }
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
