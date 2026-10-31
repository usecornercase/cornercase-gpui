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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Target {
    Account(String, String),
    Adding(String),
    Profile(String),
    NewProfile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Status {
    Login { state: LoginState, headline: String, plan: Option<String> },
    Checking,
    NotChecked,
    Unasked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Ask {
    pub title: String,
    pub message: String,
    pub button: &'static str,
    pub command: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Row<'a> {
    pub provider: &'a Provider,
    pub account: Option<&'a Account>,
}

pub(super) fn parse(text: &str) -> anyhow::Result<AccountsInfo> {
    Ok(serde_json::from_str(text)?)
}

impl<'a> Row<'a> {
    pub(super) fn of(provider: &'a Provider) -> impl Iterator<Item = Row<'a>> {
        let accounts = provider.accounts.iter().map(move |account| Row { provider, account: Some(account) });
        std::iter::once(Row { provider, account: None }).chain(accounts)
    }

    pub(super) fn agent(self) -> &'a str {
        &self.provider.id
    }

    pub(super) fn folder(self) -> &'a str {
        self.account.map_or("", |a| a.name.as_str())
    }

    pub(super) fn active(self) -> bool {
        self.provider.active == self.folder()
    }

    pub(super) fn name(self) -> &'a str {
        self.provider.name_of(self.folder())
    }

    pub(super) fn label(self) -> &'a str {
        self.account.map_or(&self.provider.system_label, |a| &a.label)
    }

    pub(super) fn id(self, what: &str) -> String {
        format!("{what}-{}-{}", self.agent(), self.folder())
    }

    pub(super) fn target(self) -> Target {
        Target::Account(self.agent().to_string(), self.folder().to_string())
    }

    pub(super) fn status(self, checking: bool) -> Status {
        let login = match self.account {
            Some(a) => a.login.as_ref(),
            None => self.provider.system_login.as_ref(),
        };
        status(login, self.provider.checks(self.folder()), checking)
    }

    pub(super) fn detail(self, home: &str) -> String {
        let path = match self.account {
            Some(a) => a.path.as_str(),
            None => self.provider.system_path.as_deref().unwrap_or_default(),
        };
        let path = home_relative(path, home);
        match self.account {
            Some(a) if !a.label.is_empty() => format!("folder {} · {path}", a.name),
            _ => path,
        }
    }

    pub(super) fn login_hint(self, state: LoginState) -> Option<String> {
        match state {
            LoginState::SignedOut => Some(format!(
                "To log in, run the agent in a tab of this account. One opens with: cornercase new-tab --account {}={}",
                self.agent(),
                self.folder()
            )),
            LoginState::Failed => {
                Some("The agent did not answer cornercase's check: the refresh button asks again".into())
            }
            LoginState::SignedIn | LoginState::Unknown => None,
        }
    }

    pub(super) fn remove_ask(self) -> Ask {
        Ask {
            title: format!("Remove the account {}?", self.name()),
            message: format!(
                "Its folder is deleted, and panes and profiles that use it go back to {}'s own login. The keychain \
                 item stays: to log out first, run the agent's logout in a tab of this account.",
                self.provider.title
            ),
            button: "Remove",
            command: remove_args(self.agent(), self.folder()),
        }
    }
}

pub(super) fn delete_ask(profile: &str) -> Ask {
    Ask {
        title: format!("Delete the profile {profile}?"),
        message: "Its accounts stay; only the name that picks them goes.".into(),
        button: "Delete",
        command: profile_delete_args(profile),
    }
}

impl Login {
    pub(super) fn headline(&self) -> String {
        match (self.state, &self.email) {
            (LoginState::SignedIn, Some(email)) => format!("Signed in as {email}"),
            (LoginState::SignedIn, None) => "Signed in".into(),
            (LoginState::SignedOut, _) => "Signed out".into(),
            (LoginState::Unknown, _) => "Login unknown".into(),
            (LoginState::Failed, _) => "Login check failed".into(),
        }
    }
}

pub(super) fn status(login: Option<&Login>, checked: bool, checking: bool) -> Status {
    match login {
        Some(login) => Status::Login { state: login.state, headline: login.headline(), plan: login.plan.clone() },
        None if !checked => Status::Unasked,
        None if checking => Status::Checking,
        None => Status::NotChecked,
    }
}

impl Provider {
    pub(super) fn system_name(&self) -> &str {
        if self.system_label.is_empty() { "System" } else { &self.system_label }
    }

    pub(super) fn checks(&self, folder: &str) -> bool {
        !folder.is_empty() || !self.accounts.is_empty()
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

pub(super) fn home_relative(path: &str, home: &str) -> String {
    match path.strip_prefix(home) {
        Some(rest) if !home.is_empty() && (rest.is_empty() || rest.starts_with('/')) => format!("~{rest}"),
        _ => path.to_string(),
    }
}

fn listed(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

pub(super) fn tokens_title(tokens: &[String]) -> String {
    let verb = if tokens.len() == 1 { "is" } else { "are" };
    format!("{} {verb} set where cornercase runs", listed(tokens))
}

pub(super) fn unshared_text(files: &[String]) -> String {
    let own = if files.len() == 1 { "a file" } else { "files" };
    format!("No longer shared with the own login: {} (now {own} of its own)", listed(files))
}

pub(super) fn target(command: &[String]) -> Option<Target> {
    let names: Vec<String> = command.iter().skip_while(|w| w.as_str() != "--").skip(1).cloned().collect();
    let name = |i: usize| names.get(i).cloned().unwrap_or_default();
    let system = command.iter().any(|w| w == "--system");
    let words: Vec<&str> = command.iter().take(2).map(String::as_str).collect();
    match words.as_slice() {
        ["account", "add"] => Some(Target::Adding(name(0))),
        ["account", "remove" | "use" | "rename"] => {
            Some(Target::Account(name(0), if system { String::new() } else { name(1) }))
        }
        ["profile", "new"] => Some(Target::NewProfile),
        ["profile", "use"] if system => Some(Target::Profile(String::new())),
        ["profile", "use" | "set" | "delete"] => Some(Target::Profile(name(0))),
        _ => None,
    }
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

pub(super) fn doing(command: &[String]) -> &'static str {
    let words: Vec<&str> = command.iter().take(2).map(String::as_str).collect();
    match words.as_slice() {
        ["account", "add"] if command.iter().any(|w| w == "--login") => "Adding the account and opening its login…",
        ["account", "add"] => "Adding the account…",
        ["account", "remove"] => "Removing the account and its folder…",
        ["account", "use"] => "Making it the default…",
        ["account", "rename"] => "Renaming…",
        ["profile", "delete"] => "Deleting the profile…",
        ["profile", _] => "Saving the profile…",
        _ => "Saving…",
    }
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
