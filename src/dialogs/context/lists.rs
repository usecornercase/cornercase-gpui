use crate::model::{Breakdown, Category, ContextAnswer, Messages, Slot};
use crate::platform::paths::{self, Style};

const BIGGEST_TOOLS: usize = 5;
const NO_CONVERSATION: &str = "No conversation yet: this is what a new conversation starts with.";

#[expect(clippy::cast_precision_loss, reason = "a shown count keeps at most one decimal")]
fn tokens(n: u64) -> String {
    let (value, suffix) = match n {
        1_000_000.. => (n as f64 / 1_000_000.0, "M"),
        1_000.. => (n as f64 / 1_000.0, "k"),
        _ => return n.to_string(),
    };
    let text = format!("{value:.1}");
    format!("{}{suffix}", text.strip_suffix(".0").unwrap_or(&text))
}

#[expect(clippy::cast_precision_loss, reason = "a shown share keeps one decimal")]
fn share(n: u64, max: u64) -> String {
    format!("{:.1}%", n as f64 * 100.0 / max.max(1) as f64)
}

#[expect(clippy::cast_precision_loss, reason = "a slice's width needs no exact large integers")]
fn fraction(n: u64, max: u64) -> f32 {
    (n as f32 / max.max(1) as f32).clamp(0., 1.)
}

pub(super) fn agent_line(answer: &ContextAnswer) -> String {
    let title = answer.title.as_deref().unwrap_or("Agent");
    let model = answer.model.as_deref().or_else(|| answer.breakdown.as_ref()?.model.as_deref());
    model.map_or_else(|| title.to_string(), |model| format!("{title} · {model}"))
}

pub(super) fn fill_line(answer: &ContextAnswer) -> Option<String> {
    if let Some(b) = &answer.breakdown {
        return Some(format!("{} of {} tokens · {}%", tokens(b.total_tokens), tokens(b.max_tokens), b.percent));
    }
    match (answer.percent, answer.compacted) {
        (Some(percent), _) => Some(format!("{percent}% of its window used")),
        (None, true) => Some("Compacted".into()),
        (None, false) => None,
    }
}

pub(super) fn compacts_line(breakdown: &Breakdown) -> Option<String> {
    breakdown.auto_compact_at.map(|at| format!("Compacts by itself at {}", tokens(at)))
}

pub(super) fn no_conversation(breakdown: &Breakdown) -> Option<&'static str> {
    breakdown.conversation.is_none().then_some(NO_CONVERSATION)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Fill {
    Used(usize),
    Free,
    Buffer,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Slice {
    pub label: String,
    pub fraction: f32,
    pub tokens: String,
    pub share: String,
    pub fill: Fill,
}

fn slice(category: &Category, max: u64, fill: Fill) -> Slice {
    Slice {
        label: category.name.clone(),
        fraction: fraction(category.tokens, max),
        tokens: tokens(category.tokens),
        share: share(category.tokens, max),
        fill,
    }
}

pub(super) fn slices(breakdown: &Breakdown) -> Vec<Slice> {
    let max = breakdown.max_tokens;
    let of = |slot: Slot| breakdown.categories.iter().filter(move |c| c.slot() == slot);
    let used = of(Slot::Used).enumerate().map(|(at, c)| slice(c, max, Fill::Used(at)));
    let free = of(Slot::Free).map(|c| slice(c, max, Fill::Free));
    let buffer = of(Slot::Buffer).map(|c| slice(c, max, Fill::Buffer));
    used.chain(free).chain(buffer).collect()
}

pub(super) fn not_loaded(breakdown: &Breakdown) -> Option<String> {
    let names: Vec<String> = breakdown
        .categories
        .iter()
        .filter(|c| c.slot() == Slot::Deferred)
        .map(|c| format!("{} {}", c.name.replace(" (deferred)", ""), tokens(c.tokens)))
        .collect();
    (!names.is_empty()).then(|| format!("Not loaded: {}", names.join(" · ")))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Row {
    pub folder: String,
    pub left: String,
    pub middle: String,
    pub right: String,
    pub muted: bool,
}

impl Row {
    fn new(left: impl Into<String>, right: u64) -> Self {
        Self { left: left.into(), right: tokens(right), ..Self::default() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Section {
    pub title: &'static str,
    pub rows: Vec<Row>,
}

fn path_row(path: &str, kind: &str, right: u64, home: &str) -> Row {
    let shown = paths::home_relative(path, home);
    let (folder, name) = shown.split_at(Style::of_path(path).name_start(&shown));
    Row { folder: folder.to_string(), middle: kind.to_string(), ..Row::new(name, right) }
}

fn memory_rows(breakdown: &Breakdown, home: &str) -> Vec<Row> {
    breakdown.memory_files.iter().map(|file| path_row(&file.path, &file.kind, file.tokens, home)).collect()
}

fn server_rows(breakdown: &Breakdown) -> Vec<Row> {
    breakdown
        .mcp_servers
        .iter()
        .map(|server| Row {
            middle: format!("{} of {} tools loaded", server.loaded, server.tools),
            muted: server.loaded == 0,
            ..Row::new(server.name.as_str(), server.tokens)
        })
        .collect()
}

fn instruction_rows(breakdown: &Breakdown) -> Vec<Row> {
    breakdown.mcp_instructions.iter().map(|item| Row::new(item.name.as_str(), item.tokens)).collect()
}

fn skill_rows(breakdown: &Breakdown) -> Vec<Row> {
    let Some(skills) = breakdown.skills.as_ref().filter(|s| s.total > 0) else { return Vec::new() };
    let count = if skills.included == skills.total {
        format!("{} skills", skills.total)
    } else {
        format!("{} of {} skills", skills.included, skills.total)
    };
    vec![Row::new(count, skills.tokens)]
}

fn message_rows(messages: &Messages) -> Vec<Row> {
    vec![
        Row::new("Tool calls and results", messages.tool_calls + messages.tool_results),
        Row::new("Attachments", messages.attachments),
        Row::new("Assistant", messages.assistant),
        Row::new("You", messages.user),
    ]
}

fn tool_rows(messages: &Messages) -> Vec<Row> {
    messages.biggest_tools(BIGGEST_TOOLS).into_iter().map(|(name, used)| Row::new(name, used)).collect()
}

pub(super) fn sections(breakdown: &Breakdown, home: &str) -> Vec<Section> {
    let messages = breakdown.messages.as_ref();
    let all = [
        ("Memory files", memory_rows(breakdown, home)),
        ("MCP servers", server_rows(breakdown)),
        ("MCP server instructions", instruction_rows(breakdown)),
        ("Skills", skill_rows(breakdown)),
        ("Messages", messages.map(message_rows).unwrap_or_default()),
        ("Tools that took the most", messages.map(tool_rows).unwrap_or_default()),
    ];
    all.into_iter().filter(|(_, rows)| !rows.is_empty()).map(|(title, rows)| Section { title, rows }).collect()
}

#[cfg(test)]
mod tests;
