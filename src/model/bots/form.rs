use anyhow::bail;
use serde::Deserialize;
use serde_json::Value;

pub(crate) const CORNERCASE: &str = "cornercase";
pub(crate) const FULL: &str = "full";

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct AgentInfo {
    pub agent: String,
    pub title: String,
    pub bot: bool,
    pub why: Option<String>,
    pub accesses: Vec<String>,
    pub efforts: Vec<String>,
    pub models: Vec<String>,
}

pub(crate) fn agents_of(text: &str) -> anyhow::Result<Vec<AgentInfo>> {
    let value: Value = serde_json::from_str(text)?;
    let Some(Value::Array(entries)) = value.get("agents") else { bail!("cornercase listed no agents") };
    Ok(entries.iter().filter_map(|entry| AgentInfo::deserialize(entry).ok()).filter(|a| !a.agent.is_empty()).collect())
}

pub(crate) fn chosen_at_open(agents: &[AgentInfo], default: Option<&str>) -> Option<usize> {
    let default = default.and_then(|name| agents.iter().position(|a| a.agent == name && a.bot));
    default.or_else(|| agents.iter().position(|a| a.bot))
}

impl AgentInfo {
    pub(crate) fn offers(&self, access: &str) -> bool {
        self.accesses.iter().any(|a| a == access)
    }

    pub(crate) fn takes_effort(&self) -> bool {
        !self.efforts.is_empty()
    }

    pub(crate) fn first_access(&self) -> Option<String> {
        self.offers(CORNERCASE).then(|| CORNERCASE.to_string())
    }

    pub(crate) fn name(&self) -> &str {
        if self.title.is_empty() { &self.agent } else { &self.title }
    }

    pub(crate) fn label(&self) -> String {
        match (&self.why, self.bot) {
            (Some(why), false) => format!("{} · {why}", self.name()),
            _ => self.name().to_string(),
        }
    }

    pub(crate) fn model_hint(&self) -> String {
        match self.models.first() {
            Some(model) => format!("the agent's own, such as {model}"),
            None => "the agent's own".to_string(),
        }
    }

    pub(crate) fn full_only_line(&self) -> Option<String> {
        if self.offers(CORNERCASE) || !self.offers(FULL) {
            return None;
        }
        let why = self.why.as_deref().map(|why| format!(": {why}")).unwrap_or_default();
        Some(format!("{} can be a bot with full access only{why}.", self.name()))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Typed {
    pub name: String,
    pub model: String,
    pub effort: Option<String>,
    pub account: Option<String>,
    pub access: Option<String>,
    pub about: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NotSent {
    Blank,
    Access,
    NotABot(String),
}

fn flag(name: &str, value: &str) -> String {
    format!("--{name}={value}")
}

pub(crate) fn new_args(agent: &AgentInfo, typed: &Typed) -> Result<Vec<String>, NotSent> {
    let name = typed.name.trim();
    if name.is_empty() {
        return Err(NotSent::Blank);
    }
    if !agent.bot {
        let why = agent.why.as_deref().unwrap_or("it cannot");
        return Err(NotSent::NotABot(format!("{} cannot be a bot: {why}", agent.name())));
    }
    let access = typed.access.as_deref().ok_or(NotSent::Access)?;
    let mut args: Vec<String> = ["bot", "new", "--json"].iter().map(ToString::to_string).collect();
    args.push(flag("agent", &agent.agent));
    let model = typed.model.trim();
    if !model.is_empty() {
        args.push(flag("model", model));
    }
    if let Some(effort) = typed.effort.as_deref().filter(|_| agent.takes_effort()) {
        args.push(flag("effort", effort));
    }
    if let Some(account) = typed.account.as_deref().filter(|account| !account.is_empty()) {
        args.push(flag("account", account));
    }
    args.push(flag("access", access));
    if !typed.about.trim().is_empty() {
        args.push(flag("about", &typed.about));
    }
    args.extend(["--".to_string(), name.to_string()]);
    Ok(args)
}

#[cfg(test)]
mod tests;
