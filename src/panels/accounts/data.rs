use std::collections::BTreeMap;

use serde::Deserialize;

use crate::store::args;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct AccountsInfo {
    pub providers: Vec<Provider>,
    pub profiles: Vec<Profile>,
    pub profile: String,
    pub login: Option<Ids>,
    pub tokens: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Provider {
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
pub(super) struct Account {
    pub name: String,
    pub path: String,
    pub unshared: Vec<String>,
    pub login: Option<Login>,
    pub label: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum LoginState {
    SignedIn,
    SignedOut,
    Failed,
    #[default]
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Login {
    pub state: LoginState,
    pub email: Option<String>,
    pub plan: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Profile {
    pub name: String,
    pub accounts: BTreeMap<String, String>,
    pub active: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Ids {
    pub workspace: Option<u64>,
    pub tab: Option<u64>,
    pub pane: Option<u64>,
}

pub(super) fn parse(text: &str) -> anyhow::Result<AccountsInfo> {
    Ok(serde_json::from_str(text)?)
}

impl Login {
    pub(super) fn text(&self) -> String {
        match (self.state, &self.email, &self.plan) {
            (LoginState::SignedIn, Some(email), Some(plan)) => format!("signed in as {email} ({plan})"),
            (LoginState::SignedIn, Some(email), None) => format!("signed in as {email}"),
            (LoginState::SignedIn, None, Some(plan)) => format!("signed in ({plan})"),
            (LoginState::SignedIn, None, None) => "signed in".into(),
            (LoginState::SignedOut, ..) => "signed out".into(),
            (LoginState::Unknown, ..) => "login unknown".into(),
            (LoginState::Failed, ..) => "login check failed".into(),
        }
    }
}

impl Provider {
    pub(super) fn system_name(&self) -> &str {
        if self.system_label.is_empty() { "System" } else { &self.system_label }
    }

    pub(super) fn name_of<'a>(&'a self, account: &'a str) -> &'a str {
        if account.is_empty() {
            return self.system_name();
        }
        match self.accounts.iter().find(|a| a.name == account) {
            Some(a) if !a.label.is_empty() => &a.label,
            _ => account,
        }
    }
}

pub(super) fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && name != "."
        && name != ".."
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

pub(super) fn profile_accounts<'a>(profile: &'a Profile, providers: &'a [Provider]) -> Vec<(&'a Provider, &'a str)> {
    providers.iter().map(|p| (p, profile.accounts.get(&p.id).map_or("", String::as_str))).collect()
}

pub(super) fn add_args(provider: &str, name: &str, login_in: Option<u64>) -> Vec<String> {
    let mut a = args(&["account", "add", "--json"]);
    if let Some(workspace) = login_in {
        a.extend(args(&["--login", "--workspace", &workspace.to_string()]));
    }
    a.extend(args(&["--", provider, name]));
    a
}

pub(super) fn remove_args(provider: &str, name: &str) -> Vec<String> {
    args(&["account", "remove", "--yes", "--", provider, name])
}

pub(super) fn use_args(provider: &str, account: &str) -> Vec<String> {
    if account.is_empty() {
        args(&["account", "use", "--system", "--json", "--", provider])
    } else {
        args(&["account", "use", "--json", "--", provider, account])
    }
}

pub(super) fn rename_args(provider: &str, account: &str, label: &str) -> Vec<String> {
    let mut a = args(&["account", "rename", "--json"]);
    if account.is_empty() {
        a.push("--system".into());
    }
    if label.is_empty() {
        a.push("--clear".into());
    }
    a.push("--".into());
    a.push(provider.into());
    if !account.is_empty() {
        a.push(account.into());
    }
    if !label.is_empty() {
        a.push(label.into());
    }
    a
}

pub(super) fn profile_new_args(name: &str) -> Vec<String> {
    args(&["profile", "new", "--", name])
}

pub(super) fn profile_set_args(name: &str, provider: &str, account: &str) -> Vec<String> {
    let mut a = args(&["profile", "set", &format!("--provider={provider}")]);
    if !account.is_empty() {
        a.push(format!("--account={account}"));
    }
    a.extend(args(&["--", name]));
    a
}

pub(super) fn profile_rename_args(name: &str, new: &str) -> Vec<String> {
    args(&["profile", "set", &format!("--rename={new}"), "--", name])
}

pub(super) fn profile_use_args(name: &str) -> Vec<String> {
    if name.is_empty() { args(&["profile", "use", "--system"]) } else { args(&["profile", "use", "--", name]) }
}

pub(super) fn profile_delete_args(name: &str) -> Vec<String> {
    args(&["profile", "delete", "--", name])
}

#[cfg(test)]
mod tests;
