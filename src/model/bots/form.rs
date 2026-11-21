use std::collections::BTreeMap;

use anyhow::bail;
use serde::Deserialize;
use serde_json::Value;

use super::chat_apps::known;
use super::{Bot, set, text_or_empty};
use crate::cli::{Has, flag};

pub(crate) const CORNERCASE: &str = "cornercase";
pub(crate) const FULL: &str = "full";
const FOLDER: &str = "folder";
const READS_ANYTHING: &str = "Through cornercase, it reads anything and writes only in its own folder.";
const READS_FOLDER: &str = "Through cornercase, it reads and writes only in its own folder.";
pub(crate) const SETS_ITSELF_UP: &str = "Once made, the bot sets itself up from it at once: it schedules what this asks for and says in its chat what it will do. Left empty, it starts nothing and spends no tokens until you write to it.";

pub(crate) fn about_note(setup: Has) -> Option<&'static str> {
    (setup == Has::Yes).then_some(SETS_ITSELF_UP)
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct AgentInfo {
    pub agent: String,
    pub title: String,
    pub bot: bool,
    pub why: Option<String>,
    pub accesses: Vec<String>,
    #[serde(deserialize_with = "text_or_empty")]
    pub reads: String,
    pub efforts: Vec<String>,
    pub models: Vec<String>,
}

pub(crate) fn agents_of(text: &str) -> anyhow::Result<Vec<AgentInfo>> {
    let value: Value = serde_json::from_str(text)?;
    let Some(Value::Array(entries)) = value.get("agents") else { bail!("cornercase listed no agents") };
    Ok(entries.iter().filter_map(|entry| AgentInfo::deserialize(entry).ok()).filter(|a| !a.agent.is_empty()).collect())
}

pub(crate) fn chosen_at_open(agents: &[AgentInfo], default: Option<&str>) -> Option<usize> {
    let through = |a: &AgentInfo| a.bot && a.offers(CORNERCASE);
    let default = default.and_then(|name| agents.iter().position(|a| a.agent == name && through(a)));
    default.or_else(|| agents.iter().position(through)).or_else(|| agents.iter().position(|a| a.bot))
}

pub(crate) fn id_of(answer: &str) -> Option<String> {
    let value: Value = serde_json::from_str(answer).ok()?;
    value.get("id")?.as_str().filter(|id| !id.is_empty()).map(str::to_string)
}

impl AgentInfo {
    pub(crate) fn offers(&self, access: &str) -> bool {
        self.accesses.iter().any(|a| a == access)
    }

    pub(crate) fn through_note(&self) -> &'static str {
        if self.reads == FOLDER { READS_FOLDER } else { READS_ANYTHING }
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
    pub chat_apps: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NotSent {
    Blank,
    Access,
    NotABot(String),
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
    if let Some(chat) = typed.chat_apps.as_deref() {
        args.push(flag("chat-apps", chat));
    }
    if !typed.about.trim().is_empty() {
        args.push(flag("about", &typed.about));
    }
    args.extend(["--".to_string(), name.to_string()]);
    Ok(args)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Editing {
    pub id: String,
    pub agent: String,
    pub before: Typed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Save {
    Sends(Vec<String>),
    Closes,
    Waits,
}

fn edit_head(id: &str) -> Vec<String> {
    ["bot", "edit", id, "--json"].iter().map(ToString::to_string).collect()
}

impl Editing {
    pub(crate) fn of(bot: &Bot) -> Self {
        let access = [CORNERCASE, FULL].contains(&bot.access.as_str()).then(|| bot.access.clone());
        let before = Typed {
            name: bot.name.clone(),
            model: bot.model.clone().unwrap_or_default(),
            effort: set(bot.effort.as_ref()),
            account: set(bot.account.as_ref()),
            access,
            about: String::new(),
            chat_apps: known(&bot.chat_apps),
        };
        Self { id: bot.id.clone(), agent: bot.agent.clone(), before }
    }

    pub(crate) fn args(&self, agent: &AgentInfo, now: &Typed) -> Option<Vec<String>> {
        let before = &self.before;
        let mut changes = Vec::new();
        let name = now.name.trim();
        if !name.is_empty() && name != before.name.trim() {
            changes.push(flag("name", name));
        }
        let model = now.model.trim();
        if model != before.model.trim() {
            changes.push(flag("model", model));
        }
        if agent.takes_effort() && now.effort != before.effort {
            changes.push(flag("effort", now.effort.as_deref().unwrap_or("")));
        }
        if let Some(account) = now.account.as_deref().filter(|a| *a != before.account.as_deref().unwrap_or("")) {
            changes.push(flag("account", account));
        }
        if let Some(access) = now.access.as_deref().filter(|a| Some(*a) != before.access.as_deref()) {
            changes.push(flag("access", access));
        }
        if let Some(chat) = now.chat_apps.as_deref().filter(|c| Some(*c) != before.chat_apps.as_deref()) {
            changes.push(flag("chat-apps", chat));
        }
        (!changes.is_empty()).then(|| [edit_head(&self.id), changes].concat())
    }

    pub(crate) fn save(&self, starting: bool, agent: &AgentInfo, now: &Typed) -> Save {
        if starting {
            return Save::Waits;
        }
        self.args(agent, now).map_or(Save::Closes, Save::Sends)
    }
}

pub(crate) fn kept_accounts(agents: &[AgentInfo], editing: Option<&Editing>) -> BTreeMap<String, String> {
    let mut kept: BTreeMap<String, String> = agents.iter().map(|a| (a.agent.clone(), String::new())).collect();
    if let Some(editing) = editing {
        kept.insert(editing.agent.clone(), editing.before.account.clone().unwrap_or_default());
    }
    kept
}

pub(crate) fn rename_args(id: &str, name: &str) -> Vec<String> {
    let mut args = edit_head(id);
    args.push(flag("name", name));
    args
}

#[cfg(test)]
mod tests;
