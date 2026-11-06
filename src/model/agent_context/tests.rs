use super::*;
use crate::test_util::fixture;

fn answer(text: &str) -> ContextAnswer {
    serde_json::from_str(text).expect("a context answer")
}

#[test]
fn a_read_carries_the_lists_of_what_fills_the_window() {
    let answer = answer(fixture!("context/read.json"));
    assert_eq!(answer.title.as_deref(), Some("Claude Code"));
    let breakdown = answer.breakdown.expect("a breakdown");
    assert_eq!((breakdown.total_tokens, breakdown.max_tokens, breakdown.percent), (251_853, 1_000_000, 25));
    assert_eq!(breakdown.auto_compact_at, Some(967_000));
    assert_eq!(breakdown.categories.len(), 11);
    assert_eq!(breakdown.memory_files[1].path, "/home/ana/code/web-shop/AGENTS.md");
    assert_eq!(breakdown.mcp_servers[0], McpServer { name: "claude_ai_Docs".into(), tools: 8, loaded: 3, tokens: 623 });
    assert_eq!(breakdown.skills, Some(Skills { total: 34, included: 34, tokens: 6_002 }));
}

#[test]
fn another_agent_answers_its_model_and_percent_with_a_notice() {
    let answer = answer(fixture!("context/other-agent.json"));
    assert_eq!(answer.model.as_deref(), Some("gpt-5.4"));
    assert_eq!(answer.percent, Some(20));
    assert_eq!(answer.notice.as_deref(), Some("Codex does not say what fills its context"));
    assert!(answer.breakdown.is_none());
}

#[test]
fn an_answer_without_the_newer_keys_is_still_read() {
    assert_eq!(answer("{}"), ContextAnswer::default());
}

#[test]
fn a_category_is_placed_by_its_kind_never_its_name() {
    let of = |kind: &str| Category { name: "Free space".into(), tokens: 1, kind: kind.into() }.slot();
    assert_eq!(of("used"), Slot::Used);
    assert_eq!(of("free"), Slot::Free);
    assert_eq!(of("buffer"), Slot::Buffer);
    assert_eq!(of("deferred"), Slot::Deferred);
    assert_eq!(of("never seen before"), Slot::Other);
}

#[test]
fn only_as_many_of_the_biggest_tools_as_asked_come_first() {
    let tool = |name: &str, calls, results| ToolUse { name: name.into(), calls, results };
    let messages = Messages {
        tools: vec![tool("Read", 80, 8_881), tool("Bash", 4_841, 81_916), tool("Write", 2_037, 177)],
        ..Messages::default()
    };
    assert_eq!(messages.biggest_tools(2), [("Bash", 86_757), ("Read", 8_961)]);
}
