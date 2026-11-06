use super::*;
use crate::model::{McpServer, Skills};
use crate::test_util::fixture;

fn answer(text: &str) -> ContextAnswer {
    serde_json::from_str(text).expect("a context answer")
}

fn read() -> Breakdown {
    answer(fixture!("context/read.json")).breakdown.expect("a breakdown")
}

fn rows_of(breakdown: &Breakdown, title: &str) -> Vec<Row> {
    sections(breakdown, "/home/ana").into_iter().find(|s| s.title == title).map(|s| s.rows).unwrap_or_default()
}

fn category(name: &str, tokens: u64, kind: &str) -> Category {
    Category { name: name.into(), tokens, kind: kind.into() }
}

#[test]
fn tokens_read_as_short_figures() {
    let said = [418, 2_681, 6_000, 251_853, 1_000_000, 1_250_000].map(tokens);
    assert_eq!(said, ["418", "2.7k", "6k", "251.9k", "1M", "1.2M"]);
}

#[test]
fn a_read_names_its_agent_and_model() {
    assert_eq!(agent_line(&answer(fixture!("context/read.json"))), "Claude Code · Opus 5.5");
}

#[test]
fn a_breakdown_says_its_tokens_and_percent() {
    let read = answer(fixture!("context/read.json"));
    assert_eq!(fill_line(&read).as_deref(), Some("251.9k of 1M tokens · 25%"));
}

#[test]
fn a_breakdown_says_where_it_compacts_by_itself() {
    assert_eq!(compacts_line(&read()).as_deref(), Some("Compacts by itself at 967k"));
}

#[test]
fn another_agent_says_only_its_model_and_percent() {
    let other = answer(fixture!("context/other-agent.json"));
    assert_eq!(agent_line(&other), "Codex · gpt-5.4");
    assert_eq!(fill_line(&other).as_deref(), Some("20% of its window used"));
}

#[test]
fn the_breakdowns_model_stands_in_when_the_tab_knows_none_yet() {
    let fresh = answer(fixture!("context/fresh.json"));
    assert_eq!(agent_line(&fresh), "Claude Code · claude-opus-5-5");
}

#[test]
fn an_agent_with_nothing_to_say_is_named_alone() {
    let not_yet = answer(fixture!("context/not-yet.json"));
    assert_eq!(agent_line(&not_yet), "Claude Code");
    assert_eq!(fill_line(&not_yet), None);
    assert_eq!(agent_line(&ContextAnswer::default()), "Agent");
}

#[test]
fn a_compacted_agent_says_so_when_it_has_no_percent() {
    let compacted = ContextAnswer { compacted: true, ..ContextAnswer::default() };
    assert_eq!(fill_line(&compacted).as_deref(), Some("Compacted"));
}

#[test]
fn auto_compact_turned_off_says_nothing() {
    assert_eq!(compacts_line(&Breakdown::default()), None);
}

#[test]
fn only_a_breakdown_without_a_conversation_says_it_is_new() {
    let fresh = answer(fixture!("context/fresh.json")).breakdown.expect("a breakdown");
    assert_eq!(no_conversation(&fresh), Some(NO_CONVERSATION));
    assert_eq!(no_conversation(&read()), None);
}

#[test]
fn the_bar_holds_used_then_free_then_the_buffer_at_the_far_end() {
    let breakdown = Breakdown {
        max_tokens: 100,
        categories: vec![
            category("Buffer", 10, "buffer"),
            category("Free", 50, "free"),
            category("Prompt", 30, "used"),
            category("Messages", 10, "used"),
        ],
        ..Breakdown::default()
    };
    let fills: Vec<Fill> = slices(&breakdown).iter().map(|s| s.fill).collect();
    assert_eq!(fills, [Fill::Used(0), Fill::Used(1), Fill::Free, Fill::Buffer]);
}

#[test]
fn the_slices_of_a_read_fill_the_whole_window() {
    let sum: f32 = slices(&read()).iter().map(|s| s.fraction).sum();
    assert!((sum - 1.).abs() < 0.001, "{sum}");
}

#[test]
fn a_slice_says_its_tokens_and_share() {
    let all = slices(&read());
    let prompt = all.first().expect("a slice");
    assert_eq!(
        (prompt.label.as_str(), prompt.tokens.as_str(), prompt.share.as_str()),
        ("System prompt", "2.7k", "0.3%")
    );
    let free = all.iter().find(|s| s.fill == Fill::Free).expect("free space");
    assert_eq!((free.tokens.as_str(), free.share.as_str()), ("715.1k", "71.5%"));
}

#[test]
fn a_kind_never_seen_stays_out_of_the_bar() {
    let breakdown = Breakdown {
        max_tokens: 100,
        categories: vec![category("New", 10, "never seen before"), category("Prompt", 10, "used")],
        ..Breakdown::default()
    };
    let labels: Vec<String> = slices(&breakdown).into_iter().map(|s| s.label).collect();
    assert_eq!(labels, ["Prompt"]);
}

#[test]
fn deferred_categories_stay_out_of_the_bar() {
    assert!(slices(&read()).iter().all(|s| !s.label.contains("deferred")));
}

#[test]
fn deferred_categories_fold_into_one_line() {
    assert_eq!(not_loaded(&read()).as_deref(), Some("Not loaded: MCP tools 199.9k · System tools 19k"));
}

#[test]
fn no_deferred_category_gives_no_line() {
    assert_eq!(not_loaded(&Breakdown::default()), None);
}

#[test]
fn an_empty_window_divides_by_nothing() {
    assert!(fraction(5, 0) <= 1.);
    assert_eq!(share(0, 0), "0.0%");
}

#[test]
fn a_memory_file_shows_relative_to_home_with_its_name_apart() {
    let rows = rows_of(&read(), "Memory files");
    assert_eq!(rows.len(), 4);
    let agents = &rows[1];
    assert_eq!(
        (agents.folder.as_str(), agents.left.as_str(), agents.middle.as_str(), agents.right.as_str()),
        ("~/code/web-shop/", "AGENTS.md", "Project", "8.5k")
    );
}

#[test]
fn a_memory_file_outside_home_keeps_its_whole_path() {
    let row = path_row("/etc/claude/CLAUDE.md", "Managed", 12, "/home/ana");
    assert_eq!((row.folder.as_str(), row.left.as_str()), ("/etc/claude/", "CLAUDE.md"));
}

#[test]
fn a_memory_file_on_windows_splits_at_its_backslash() {
    let row = path_row(r"C:\Users\ana\code\AGENTS.md", "Project", 12, r"C:\Users\ana");
    assert_eq!((row.folder.as_str(), row.left.as_str()), (r"~\code\", "AGENTS.md"));
}

#[test]
fn a_server_says_how_many_of_its_tools_loaded() {
    let rows = rows_of(&read(), "MCP servers");
    let docs = &rows[0];
    assert_eq!(
        (docs.left.as_str(), docs.middle.as_str(), docs.right.as_str()),
        ("claude_ai_Docs", "3 of 8 tools loaded", "623")
    );
}

#[test]
fn a_server_with_none_loaded_is_muted() {
    let rows = rows_of(&read(), "MCP servers");
    assert!(!rows[0].muted);
    assert_eq!((rows[2].middle.as_str(), rows[2].muted), ("0 of 3 tools loaded", true));
}

#[test]
fn server_instructions_are_their_own_list() {
    let rows = rows_of(&read(), "MCP server instructions");
    assert_eq!((rows[0].left.as_str(), rows[0].right.as_str()), ("claude.ai Docs", "658"));
}

#[test]
fn skills_say_how_many_are_included() {
    assert_eq!(rows_of(&read(), "Skills")[0].left, "34 skills");
    let some = Breakdown { skills: Some(Skills { total: 34, included: 20, tokens: 9 }), ..Breakdown::default() };
    assert_eq!(rows_of(&some, "Skills")[0].left, "20 of 34 skills");
}

#[test]
fn no_skills_give_no_section() {
    let none = Breakdown { skills: Some(Skills::default()), ..Breakdown::default() };
    assert_eq!(rows_of(&none, "Skills"), Vec::<Row>::new());
}

#[test]
fn messages_say_what_the_conversation_is_made_of() {
    let rows = rows_of(&read(), "Messages");
    let said: Vec<(&str, &str)> = rows.iter().map(|r| (r.left.as_str(), r.right.as_str())).collect();
    assert_eq!(
        said,
        [("Tool calls and results", "97.9k"), ("Attachments", "137.1k"), ("Assistant", "20.7k"), ("You", "418")]
    );
}

#[test]
fn the_tools_that_took_the_most_come_first() {
    let rows = rows_of(&read(), "Tools that took the most");
    let said: Vec<(&str, &str)> = rows.iter().map(|r| (r.left.as_str(), r.right.as_str())).collect();
    assert_eq!(said, [("Bash", "86.8k"), ("Read", "9k"), ("Write", "2.2k")]);
}

#[test]
fn only_the_lists_with_something_in_them_show() {
    let server = McpServer { name: "github".into(), ..McpServer::default() };
    let breakdown = Breakdown { mcp_servers: vec![server], ..Breakdown::default() };
    let titles: Vec<&str> = sections(&breakdown, "").iter().map(|s| s.title).collect();
    assert_eq!(titles, ["MCP servers"]);
}
