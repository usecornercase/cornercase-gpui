use crate::store::args;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum When {
    Keep,
    At,
    Tomorrow,
    On,
    In,
    ClaudeReset,
    CodexReset,
}

impl When {
    pub(super) const NEW: [When; 6] =
        [When::In, When::At, When::Tomorrow, When::On, When::ClaudeReset, When::CodexReset];

    pub(super) fn label(self) -> &'static str {
        match self {
            When::Keep => "unchanged",
            When::At => "at",
            When::Tomorrow => "tomorrow at",
            When::On => "on",
            When::In => "in",
            When::ClaudeReset => "Claude reset",
            When::CodexReset => "Codex reset",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    Keep,
    Workspace(u64),
    Worktree { project: u64, branch: String },
}

impl Target {
    fn push_args(&self, a: &mut Vec<String>) -> Result<(), String> {
        match self {
            Target::Keep => {}
            Target::Workspace(id) => push(a, "--workspace", &id.to_string()),
            Target::Worktree { project, branch } => {
                if branch.trim().is_empty() {
                    return Err("name the worktree's branch".into());
                }
                push(a, "--project", &project.to_string());
                push(a, "--worktree", branch.trim());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Draft {
    pub edit: Option<u64>,
    pub issue: bool,
    pub text: String,
    pub source: Option<String>,
    pub when: When,
    pub clock: String,
    pub date: String,
    pub amount: String,
    pub unit: char,
    pub target: Target,
    pub agent: Option<String>,
    pub mode: Option<String>,
    pub name: Option<String>,
}

fn push(a: &mut Vec<String>, flag: &str, value: &str) {
    a.push(flag.to_string());
    a.push(value.to_string());
}

impl Draft {
    pub(crate) fn at(&self) -> Result<Option<String>, String> {
        let clock = self.clock.trim();
        let need_clock = || if clock.is_empty() { Err("choose a time, such as 14:30".to_string()) } else { Ok(()) };
        Ok(Some(match self.when {
            When::Keep => return Ok(None),
            When::At => {
                need_clock()?;
                clock.to_string()
            }
            When::Tomorrow => {
                need_clock()?;
                format!("tomorrow {clock}")
            }
            When::On => {
                need_clock()?;
                format!("{} {clock}", self.date()?)
            }
            When::In => self.delay()?,
            When::ClaudeReset => "claude reset".into(),
            When::CodexReset => "codex reset".into(),
        }))
    }

    fn date(&self) -> Result<&str, String> {
        let date = self.date.trim();
        if date.is_empty() { Err("choose a date, such as 2026-10-24".into()) } else { Ok(date) }
    }

    fn delay(&self) -> Result<String, String> {
        let amount = self.amount.trim();
        if !amount.parse::<u32>().is_ok_and(|n| n > 0) {
            return Err("the delay must be a whole number above 0".into());
        }
        Ok(format!("in {amount}{}", self.unit))
    }

    pub(crate) fn args(&self) -> Result<Vec<String>, String> {
        let mut a = match self.edit {
            Some(id) => args(&["queue", "edit", &id.to_string()]),
            None => args(&["queue", "add"]),
        };
        match self.at()? {
            Some(at) => push(&mut a, "--at", &at),
            None if self.edit.is_none() => return Err("choose when it starts".into()),
            None => {}
        }
        self.push_what(&mut a)?;
        self.target.push_args(&mut a)?;
        self.push_choices(&mut a);
        a.push("--json".into());
        Ok(a)
    }

    fn push_what(&self, a: &mut Vec<String>) -> Result<(), String> {
        let text = self.text.trim();
        match (self.issue, text.is_empty()) {
            (true, false) => {
                push(a, "--issue", text);
                if let Some(source) = &self.source {
                    push(a, "--source", source);
                }
            }
            (false, false) => push(a, "--prompt", text),
            (true, true) if self.edit.is_none() => return Err("paste an issue URL or key".into()),
            (false, true) if self.edit.is_none() => return Err("write the prompt it starts with".into()),
            _ => {}
        }
        Ok(())
    }

    fn push_choices(&self, a: &mut Vec<String>) {
        if let Some(agent) = &self.agent {
            push(a, "--agent", agent);
        }
        if let Some(mode) = &self.mode {
            push(a, "--mode", mode);
        }
        if let Some(name) = self.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
            push(a, "--name", name);
        }
    }
}
