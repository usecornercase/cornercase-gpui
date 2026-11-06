use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::Value;

use super::{PaneInfo, Session};

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct AccountsInfo {
    pub providers: Vec<Provider>,
    pub profiles: Vec<Profile>,
    pub profile: String,
    pub login: Option<Ids>,
    pub tokens: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Provider {
    #[serde(rename = "provider")]
    pub id: String,
    pub title: String,
    pub env: String,
    pub accounts: Vec<Account>,
    pub system_label: String,
    pub active: String,
    pub system_path: Option<String>,
    pub system_login: Option<Login>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Account {
    pub name: String,
    pub path: String,
    pub unshared: Vec<String>,
    pub login: Option<Login>,
    pub label: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum LoginState {
    SignedIn,
    SignedOut,
    Failed,
    #[default]
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Login {
    pub state: LoginState,
    pub email: Option<String>,
    pub plan: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Profile {
    pub name: String,
    pub accounts: BTreeMap<String, String>,
    pub active: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Ids {
    pub workspace: Option<u64>,
    pub tab: Option<u64>,
    pub pane: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Selection {
    pub accounts: BTreeMap<String, String>,
    pub profile: Option<String>,
}

impl Selection {
    pub(crate) fn of(app: &Value) -> Self {
        Self::deserialize(app).unwrap_or_default()
    }
}

pub(crate) type Stamp = (u64, BTreeSet<(String, String)>);

impl Session {
    pub(crate) fn accounts_stamp(&self) -> Stamp {
        let panes = self.tabs().flat_map(|t| &t.panes);
        let in_use = panes.flat_map(|pane| {
            let running = pane.agent.iter().zip(&pane.account).map(|(agent, account)| (agent.clone(), account.clone()));
            running.chain(pane.accounts.iter().map(|(agent, account)| (agent.clone(), account.clone())))
        });
        (self.config.revision, in_use.collect())
    }
}

impl Provider {
    pub(crate) fn system_name(&self) -> &str {
        if self.system_label.is_empty() { "System" } else { &self.system_label }
    }

    pub(crate) fn name_of<'a>(&'a self, account: &'a str) -> &'a str {
        if account.is_empty() {
            return self.system_name();
        }
        match self.accounts.iter().find(|a| a.name == account) {
            Some(a) if !a.label.is_empty() => &a.label,
            _ => account,
        }
    }

    fn folders(&self) -> impl Iterator<Item = &str> {
        std::iter::once("").chain(self.accounts.iter().map(|a| a.name.as_str()))
    }

    fn switcher(&self, pane: &PaneInfo) -> Switcher {
        let runs = pane.agent.is_some();
        let folder = if runs { pane.account.as_deref() } else { pane.accounts.get(&self.id).map(String::as_str) };
        let folder = folder.unwrap_or_default();
        Switcher {
            pane: pane.id,
            agent: self.id.clone(),
            title: self.title.clone(),
            folder: folder.to_string(),
            name: self.name_of(folder).to_string(),
            runs,
            waiting: pane.account_restart.is_some(),
        }
    }
}

impl AccountsInfo {
    pub(crate) fn any(&self) -> bool {
        self.switchable().next().is_some()
    }

    fn switchable(&self) -> impl Iterator<Item = &Provider> {
        self.providers.iter().filter(|p| !p.accounts.is_empty())
    }

    fn provider(&self, id: &str) -> Option<&Provider> {
        self.providers.iter().find(|p| p.id == id)
    }

    pub(crate) fn mark(&self, pane: &PaneInfo) -> Option<String> {
        if !self.any() {
            return None;
        }
        let provider = self.provider(pane.agent.as_deref()?);
        let name = match (pane.account.as_deref(), provider) {
            (Some(account), Some(provider)) => provider.name_of(account),
            (Some(account), None) => account,
            (None, provider) => provider.filter(|p| !p.accounts.is_empty())?.system_name(),
        };
        Some(name.to_string())
    }

    pub(crate) fn switchers(&self, pane: &PaneInfo) -> Vec<Switcher> {
        match pane.agent.as_deref() {
            Some(agent) => self.switchable().filter(|p| p.id == agent).map(|p| p.switcher(pane)).collect(),
            None => self.switchable().map(|p| p.switcher(pane)).collect(),
        }
    }

    pub(crate) fn switches(&self, switcher: &Switcher) -> Vec<Switch> {
        let Some(provider) = self.provider(&switcher.agent) else { return Vec::new() };
        let unlisted = Some(switcher.folder.as_str()).filter(|folder| provider.folders().all(|f| f != *folder));
        unlisted
            .into_iter()
            .chain(provider.folders())
            .map(|folder| switcher.to(folder, provider.name_of(folder)))
            .collect()
    }

    pub(crate) fn switcher_tip(&self, switcher: &Switcher, selection: &Selection) -> String {
        let here = if switcher.runs {
            format!("{} runs on {} here", switcher.title, switcher.name)
        } else {
            format!("A {} typed here starts on {}", switcher.agent, switcher.name)
        };
        let default = selection.accounts.get(&switcher.agent).map_or("", String::as_str);
        let default = self.provider(&switcher.agent).map_or(default, |p| p.name_of(default));
        match &selection.profile {
            Some(profile) => format!("{here}\nNew workspaces start on {default}, from the profile {profile}"),
            None => format!("{here}\nNew workspaces start on {default}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Switcher {
    pub pane: u64,
    pub agent: String,
    pub title: String,
    pub folder: String,
    pub name: String,
    pub runs: bool,
    pub waiting: bool,
}

impl Switcher {
    fn to(&self, folder: &str, name: &str) -> Switch {
        let chosen = folder == self.folder;
        Switch {
            pane: self.pane,
            agent: self.agent.clone(),
            folder: folder.to_string(),
            name: name.to_string(),
            runs: self.runs,
            chosen,
            cancels: chosen && self.waiting,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Switch {
    pub pane: u64,
    pub agent: String,
    pub folder: String,
    pub name: String,
    pub runs: bool,
    pub chosen: bool,
    pub cancels: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct RestartAnswer {
    pub restarted: Vec<u64>,
    pub waiting: Vec<u64>,
    pub skipped: Vec<Skipped>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Skipped {
    pub pane: u64,
    pub why: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Said {
    pub text: String,
    pub refused: bool,
}

pub(crate) fn switching_key(pane: u64) -> String {
    format!("account-restart:{pane}")
}

impl Switch {
    pub(crate) fn label(&self) -> String {
        if self.cancels { format!("Stay on {}", self.name) } else { self.name.clone() }
    }

    pub(crate) fn does_anything(&self) -> bool {
        !self.chosen || self.cancels
    }

    pub(crate) fn args(&self) -> Vec<String> {
        let mut args: Vec<String> = ["account", "restart", "--json", "--pane"].map(String::from).into();
        args.push(self.pane.to_string());
        if self.folder.is_empty() {
            args.push("--system".into());
        }
        args.push("--".into());
        args.push(self.agent.clone());
        if !self.folder.is_empty() {
            args.push(self.folder.clone());
        }
        args
    }

    pub(crate) fn doing(&self) -> String {
        let (agent, name) = (&self.agent, &self.name);
        match (self.cancels, self.runs) {
            (true, _) => format!("Keeping {agent} on {name}…"),
            (false, true) => format!("Restarting {agent} on {name}…"),
            (false, false) => format!("Moving this pane's {agent} to {name}…"),
        }
    }

    pub(crate) fn said(&self, answer: &RestartAnswer) -> Said {
        let (agent, name) = (&self.agent, &self.name);
        let done = |text: String| Said { text, refused: false };
        if answer.restarted.contains(&self.pane) {
            return done(if self.runs {
                format!("{agent} restarted on {name}")
            } else {
                format!("A {agent} typed in this pane now starts on {name}")
            });
        }
        if answer.waiting.contains(&self.pane) {
            return done(format!("{agent} restarts on {name} after this turn"));
        }
        match answer.skipped.iter().find(|skipped| skipped.pane == self.pane) {
            Some(_) if self.cancels => done(format!("{agent} stays on {name}")),
            Some(skipped) => Said { text: format!("{agent} did not move to {name}: {}", skipped.why), refused: true },
            None => Said { text: format!("{agent} did not move to {name}"), refused: true },
        }
    }
}

#[cfg(test)]
mod tests;
