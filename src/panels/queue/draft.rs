use crate::model::accounts::pick::{AccountPick, account_args};
use crate::store::args;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum When {
    Keep,
    At,
    Tomorrow,
    On,
    In,
    Every,
    Daily,
    Weekdays,
    Weekly,
    ClaudeReset,
    CodexReset,
}

impl When {
    const ONCE: [When; 6] = [When::In, When::At, When::Tomorrow, When::On, When::ClaudeReset, When::CodexReset];
    const REPEATING: [When; 4] = [When::Every, When::Daily, When::Weekdays, When::Weekly];

    pub(super) fn offered(editing: bool, routine: bool) -> Vec<Vec<When>> {
        let kept = |kinds: &[When]| std::iter::once(When::Keep).chain(kinds.iter().copied()).collect();
        match (editing, routine) {
            (false, _) => vec![Self::ONCE.to_vec(), Self::REPEATING.to_vec()],
            (true, false) => vec![kept(&Self::ONCE)],
            (true, true) => vec![kept(&Self::REPEATING)],
        }
    }

    pub(super) fn repeats(self) -> bool {
        Self::REPEATING.contains(&self)
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            When::Keep => "unchanged",
            When::At => "at",
            When::Tomorrow => "tomorrow at",
            When::On => "on",
            When::In => "in",
            When::Every => "every",
            When::Daily => "every day at",
            When::Weekdays => "every weekday at",
            When::Weekly => "every week on",
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
    Repository(u64),
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
            Target::Repository(project) => push(a, "--project", &project.to_string()),
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
    pub weekday: String,
    pub target: Target,
    pub agent: Option<String>,
    pub mode: Option<String>,
    pub flow: Option<String>,
    pub cleanup: Option<bool>,
    pub name: Option<String>,
    pub accounts: Vec<AccountPick>,
}

fn push(a: &mut Vec<String>, flag: &str, value: &str) {
    a.push(flag.to_string());
    a.push(value.to_string());
}

pub(super) fn cleanup_flag(repeats: bool, cleanup: bool, was: Option<bool>) -> Option<bool> {
    (repeats && was != Some(cleanup)).then_some(cleanup)
}

impl Draft {
    fn clock(&self) -> Result<&str, String> {
        let clock = self.clock.trim();
        if clock.is_empty() { Err("choose a time, such as 14:30".into()) } else { Ok(clock) }
    }

    pub(crate) fn at(&self) -> Result<Option<String>, String> {
        Ok(Some(match self.when {
            When::Keep | When::Every | When::Daily | When::Weekdays | When::Weekly => return Ok(None),
            When::At => self.clock()?.to_string(),
            When::Tomorrow => format!("tomorrow {}", self.clock()?),
            When::On => format!("{} {}", self.date()?, self.clock()?),
            When::In => self.delay()?,
            When::ClaudeReset => "claude reset".into(),
            When::CodexReset => "codex reset".into(),
        }))
    }

    pub(crate) fn every(&self) -> Result<Option<String>, String> {
        Ok(Some(match self.when {
            When::Every => self.interval()?,
            When::Daily => format!("day {}", self.clock()?),
            When::Weekdays => format!("weekday {}", self.clock()?),
            When::Weekly => format!("{} {}", self.weekday, self.clock()?),
            _ => return Ok(None),
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

    fn interval(&self) -> Result<String, String> {
        let amount = self.amount.trim();
        if amount.is_empty() {
            return Err("choose how often it repeats, such as every 2 hours".into());
        }
        Ok(format!("{amount}{}", self.unit))
    }

    pub(crate) fn args(&self) -> Result<Vec<String>, String> {
        let mut a = match self.edit {
            Some(id) => args(&["queue", "edit", &id.to_string()]),
            None => args(&["queue", "add"]),
        };
        self.push_when(&mut a)?;
        self.push_what(&mut a)?;
        self.push_target(&mut a)?;
        self.push_choices(&mut a);
        a.push("--json".into());
        Ok(a)
    }

    fn push_when(&self, a: &mut Vec<String>) -> Result<(), String> {
        if let Some(every) = self.every()? {
            push(a, "--every", &every);
            return Ok(());
        }
        match self.at()? {
            Some(at) => push(a, "--at", &at),
            None if self.edit.is_none() => return Err("choose when it starts".into()),
            None => {}
        }
        Ok(())
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
            (false, true) if self.edit.is_none() => return Err(self.no_prompt().into()),
            _ => {}
        }
        Ok(())
    }

    fn no_prompt(&self) -> &'static str {
        if self.flow.is_some() { "write the team's task" } else { "write the prompt it starts with" }
    }

    fn push_target(&self, a: &mut Vec<String>) -> Result<(), String> {
        if self.when.repeats() && self.edit.is_none() && self.target == Target::Keep {
            return Err("choose the repository it runs in".into());
        }
        self.target.push_args(a)
    }

    fn push_choices(&self, a: &mut Vec<String>) {
        if let Some(flow) = &self.flow {
            push(a, "--flow", flow);
        }
        if let Some(agent) = &self.agent {
            push(a, "--agent", agent);
        }
        if let Some(mode) = &self.mode {
            push(a, "--mode", mode);
        }
        if let Some(name) = self.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
            push(a, "--name", name);
        }
        if let Some(cleanup) = self.cleanup {
            a.push(if cleanup { "--cleanup" } else { "--no-cleanup" }.to_string());
        }
        a.extend(account_args(&self.accounts));
    }
}
