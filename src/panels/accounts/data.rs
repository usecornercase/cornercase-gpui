//! What `cornercase account list --json` answers (every agent with accounts, its own login and
//! each account, the profiles and the active one), and the commands that change them.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::store::args;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct AccountsInfo {
    pub providers: Vec<Provider>,
    pub profiles: Vec<Profile>,
    /// The active profile, `""` for none.
    pub profile: String,
    /// The login tab `account add --login` opened.
    pub login: Option<Ids>,
    /// Token variables set in the server's environment, which win over every account.
    pub tokens: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Provider {
    pub provider: String,
    pub title: String,
    pub env: String,
    pub accounts: Vec<Account>,
    /// The name the agent's own login shows, `""` for `System`.
    pub system_label: String,
    /// The effective account for new panes, `""` for the own login.
    pub active: String,
    pub system_path: Option<String>,
    pub system_login: Option<Login>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Account {
    pub name: String,
    pub path: String,
    /// Shared settings that stopped being links.
    pub unshared: Vec<String>,
    pub login: Option<Login>,
    pub label: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LoginState {
    SignedIn,
    SignedOut,
    Failed,
    #[default]
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Login {
    pub state: LoginState,
    pub email: Option<String>,
    pub plan: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub name: String,
    /// Agent → account folder; an agent left out uses its own login.
    pub accounts: BTreeMap<String, String>,
    pub active: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Ids {
    pub workspace: Option<u64>,
    pub tab: Option<u64>,
    pub pane: Option<u64>,
}

pub fn parse(text: &str) -> anyhow::Result<AccountsInfo> {
    Ok(serde_json::from_str(text)?)
}

impl Login {
    /// As cornercase words it: `signed in as a@b.c (max)`.
    pub fn text(&self) -> String {
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
    /// What the agent's own login is called.
    pub fn system_name(&self) -> &str {
        if self.system_label.is_empty() { "System" } else { &self.system_label }
    }

    /// An account's name as the window shows it: its label, else its folder.
    pub fn name_of<'a>(&'a self, account: &'a str) -> &'a str {
        if account.is_empty() {
            return self.system_name();
        }
        match self.accounts.iter().find(|a| a.name == account) {
            Some(a) if !a.label.is_empty() => &a.label,
            _ => account,
        }
    }
}

/// Letters, digits, `.`, `_` and `-`, at most 32, never `.` or `..`, as `account add` takes.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && name != "."
        && name != ".."
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// The agents of a profile, each with the account it uses (`""` its own login), in the providers'
/// order.
pub fn profile_accounts<'a>(profile: &'a Profile, providers: &'a [Provider]) -> Vec<(&'a Provider, &'a str)> {
    providers
        .iter()
        .map(|p| (p, profile.accounts.get(&p.provider).map(String::as_str).unwrap_or("")))
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Commands. Names go after `--`, so one that starts with `-` is never read as an option.

pub fn add_args(provider: &str, name: &str, login_in: Option<u64>) -> Vec<String> {
    let mut a = args(&["account", "add", "--json"]);
    if let Some(workspace) = login_in {
        a.extend(args(&["--login", "--workspace", &workspace.to_string()]));
    }
    a.extend(args(&["--", provider, name]));
    a
}

pub fn remove_args(provider: &str, name: &str) -> Vec<String> {
    args(&["account", "remove", "--yes", "--", provider, name])
}

/// `account use`: an account, or the agent's own login for `""`.
pub fn use_args(provider: &str, account: &str) -> Vec<String> {
    if account.is_empty() {
        args(&["account", "use", "--system", "--json", "--", provider])
    } else {
        args(&["account", "use", "--json", "--", provider, account])
    }
}

/// `account rename`: `""` for the own login; an empty label drops the name.
pub fn rename_args(provider: &str, account: &str, label: &str) -> Vec<String> {
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

pub fn profile_new_args(name: &str) -> Vec<String> {
    args(&["profile", "new", "--", name])
}

/// `profile set NAME --provider AGENT [--account ACCOUNT]`: `""` the agent's own login.
pub fn profile_set_args(name: &str, provider: &str, account: &str) -> Vec<String> {
    let mut a = args(&["profile", "set", &format!("--provider={provider}")]);
    if !account.is_empty() {
        a.push(format!("--account={account}"));
    }
    a.extend(args(&["--", name]));
    a
}

pub fn profile_rename_args(name: &str, new: &str) -> Vec<String> {
    args(&["profile", "set", &format!("--rename={new}"), "--", name])
}

/// `profile use NAME`, or `--system` for none.
pub fn profile_use_args(name: &str) -> Vec<String> {
    if name.is_empty() { args(&["profile", "use", "--system"]) } else { args(&["profile", "use", "--", name]) }
}

pub fn profile_delete_args(name: &str) -> Vec<String> {
    args(&["profile", "delete", "--", name])
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "profile": "Work",
      "profiles": [{"name":"Work","accounts":{"claude":"work"},"active":true},{"name":"Home","accounts":{}}],
      "providers": [
        {"accounts":[{"name":"work","path":"/s/accounts/claude/work","unshared":["settings.json"],
                      "login":{"state":"signed-in","email":"a@b.c","plan":"max","checked":"now"},"label":"Work"},
                     {"name":"home","path":"/s/accounts/claude/home","unshared":[],
                      "login":{"state":"signed-out","checked":"now"}}],
         "env":"CLAUDE_CONFIG_DIR","provider":"claude","system_path":"/home/me/.claude","title":"Claude Code",
         "active":"work","system_login":{"state":"weird"}},
        {"accounts":[],"env":"CODEX_HOME","provider":"codex","title":"Codex","system_label":"Mine"}
      ],
      "login": {"project": 1, "workspace": 2, "tab": 3, "pane": 4},
      "tokens": ["ANTHROPIC_API_KEY"]
    }"#;

    #[test]
    fn parses_accounts_and_profiles() {
        let info = parse(SAMPLE).unwrap();
        assert_eq!(info.profile, "Work");
        assert_eq!(info.providers.len(), 2);
        let claude = &info.providers[0];
        assert_eq!(claude.active, "work");
        assert_eq!(claude.accounts[0].unshared, ["settings.json"]);
        assert_eq!(claude.system_login.as_ref().map(|l| l.state), Some(LoginState::Unknown));
        assert_eq!(info.login.and_then(|i| i.tab), Some(3));
        assert_eq!(info.tokens, ["ANTHROPIC_API_KEY"]);
        assert!(parse(r#"{"providers":[],"profiles":[],"profile":""}"#).is_ok());
    }

    #[test]
    fn logins_read_as_cornercase_says_them() {
        let info = parse(SAMPLE).unwrap();
        let claude = &info.providers[0];
        assert_eq!(claude.accounts[0].login.as_ref().map(Login::text).as_deref(), Some("signed in as a@b.c (max)"));
        assert_eq!(claude.accounts[1].login.as_ref().map(Login::text).as_deref(), Some("signed out"));
    }

    #[test]
    fn names_prefer_labels() {
        let info = parse(SAMPLE).unwrap();
        let (claude, codex) = (&info.providers[0], &info.providers[1]);
        assert_eq!(claude.name_of("work"), "Work");
        assert_eq!(claude.name_of("home"), "home");
        assert_eq!(claude.name_of(""), "System");
        assert_eq!(codex.name_of(""), "Mine");
    }

    #[test]
    fn profiles_list_every_agent() {
        let info = parse(SAMPLE).unwrap();
        let pairs: Vec<(&str, &str)> =
            profile_accounts(&info.profiles[0], &info.providers).into_iter().map(|(p, a)| (p.provider.as_str(), a)).collect();
        assert_eq!(pairs, [("claude", "work"), ("codex", "")]);
    }

    #[test]
    fn names_are_checked_as_cornercase_does() {
        assert!(valid_name("work"));
        assert!(valid_name("my.work_2-x"));
        assert!(!valid_name(""));
        assert!(!valid_name(".."));
        assert!(!valid_name("a b"));
        assert!(!valid_name(&"x".repeat(33)));
    }

    #[test]
    fn commands_keep_names_apart_from_options() {
        assert_eq!(add_args("claude", "work", None), ["account", "add", "--json", "--", "claude", "work"]);
        assert_eq!(add_args("claude", "work", Some(9)), [
            "account",
            "add",
            "--json",
            "--login",
            "--workspace",
            "9",
            "--",
            "claude",
            "work"
        ]);
        assert_eq!(use_args("codex", ""), ["account", "use", "--system", "--json", "--", "codex"]);
        assert_eq!(rename_args("claude", "", "Mine"), ["account", "rename", "--json", "--system", "--", "claude", "Mine"]);
        assert_eq!(rename_args("claude", "work", ""), ["account", "rename", "--json", "--clear", "--", "claude", "work"]);
        assert_eq!(profile_set_args("Work", "claude", ""), ["profile", "set", "--provider=claude", "--", "Work"]);
        assert_eq!(profile_set_args("Work", "claude", "home"), [
            "profile",
            "set",
            "--provider=claude",
            "--account=home",
            "--",
            "Work"
        ]);
        assert_eq!(profile_use_args(""), ["profile", "use", "--system"]);
    }
}
