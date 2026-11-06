use serde::Deserialize;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct ContextAnswer {
    pub title: Option<String>,
    pub model: Option<String>,
    pub percent: Option<u32>,
    pub compacted: bool,
    pub age: Option<u64>,
    pub breakdown: Option<Breakdown>,
    pub notice: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Breakdown {
    pub model: Option<String>,
    pub conversation: Option<String>,
    pub total_tokens: u64,
    pub max_tokens: u64,
    pub percent: u32,
    pub auto_compact_at: Option<u64>,
    pub categories: Vec<Category>,
    pub memory_files: Vec<MemoryFile>,
    pub mcp_servers: Vec<McpServer>,
    pub mcp_instructions: Vec<McpInstruction>,
    pub skills: Option<Skills>,
    pub messages: Option<Messages>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Slot {
    Used,
    Free,
    Buffer,
    Deferred,
    Other,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Category {
    pub name: String,
    pub tokens: u64,
    pub kind: String,
}

impl Category {
    pub(crate) fn slot(&self) -> Slot {
        match self.kind.as_str() {
            "used" => Slot::Used,
            "free" => Slot::Free,
            "buffer" => Slot::Buffer,
            "deferred" => Slot::Deferred,
            _ => Slot::Other,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct MemoryFile {
    pub path: String,
    pub kind: String,
    pub tokens: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct McpServer {
    pub name: String,
    pub tools: u64,
    pub loaded: u64,
    pub tokens: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct McpInstruction {
    pub name: String,
    pub tokens: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Skills {
    pub total: u64,
    pub included: u64,
    pub tokens: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Messages {
    pub tool_calls: u64,
    pub tool_results: u64,
    pub attachments: u64,
    pub assistant: u64,
    pub user: u64,
    pub tools: Vec<ToolUse>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct ToolUse {
    pub name: String,
    pub calls: u64,
    pub results: u64,
}

impl Messages {
    pub(crate) fn biggest_tools(&self, listed: usize) -> Vec<(&str, u64)> {
        let mut tools: Vec<(&str, u64)> = self.tools.iter().map(|t| (t.name.as_str(), t.calls + t.results)).collect();
        tools.sort_by_key(|(_, tokens)| std::cmp::Reverse(*tokens));
        tools.truncate(listed);
        tools
    }
}

#[cfg(test)]
mod tests;
