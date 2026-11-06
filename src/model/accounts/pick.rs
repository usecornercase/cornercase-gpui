use super::{AccountsInfo, Provider};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountChoice {
    pub folder: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentChoices {
    pub agent: String,
    pub title: String,
    pub in_use: String,
    pub accounts: Vec<AccountChoice>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AccountSettings {
    pub mark: bool,
    pub picker: bool,
    pub default_agent: Option<String>,
}

impl AccountSettings {
    pub(crate) fn offered(&self, accounts: &AccountsInfo) -> Vec<AgentChoices> {
        if self.picker { accounts.choices() } else { Vec::new() }
    }
}

impl Provider {
    fn choices(&self) -> AgentChoices {
        let account =
            |folder: &str| AccountChoice { folder: folder.to_string(), name: self.name_of(folder).to_string() };
        let in_use = self.folders().find(|folder| *folder == self.active).unwrap_or_default();
        AgentChoices {
            agent: self.id.clone(),
            title: self.title.clone(),
            in_use: in_use.to_string(),
            accounts: self.folders().map(account).collect(),
        }
    }
}

impl AccountsInfo {
    pub(crate) fn choices(&self) -> Vec<AgentChoices> {
        self.switchable().map(Provider::choices).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountPick {
    pub agent: String,
    pub folder: String,
}

pub(crate) fn account_args(picks: &[AccountPick]) -> Vec<String> {
    picks.iter().flat_map(|pick| ["--account".to_string(), format!("{}={}", pick.agent, pick.folder)]).collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PickRow {
    pub folder: String,
    pub name: String,
    pub note: String,
    pub chosen: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Picking {
    choices: AgentChoices,
    saved: Option<String>,
    chosen: Option<String>,
}

impl Picking {
    pub(crate) fn new(choices: AgentChoices, saved: Option<&str>) -> Self {
        Self { choices, saved: saved.map(str::to_string), chosen: None }
    }

    pub(crate) fn agent(&self) -> &str {
        &self.choices.agent
    }

    pub(crate) fn title(&self) -> &str {
        &self.choices.title
    }

    pub(crate) fn follow(&mut self, choices: AgentChoices) -> bool {
        let changed = self.choices != choices;
        self.choices = choices;
        changed
    }

    pub(crate) fn choose(&mut self, folder: &str) {
        self.chosen = Some(folder.to_string());
    }

    fn untouched(&self) -> &str {
        self.saved.as_deref().unwrap_or(&self.choices.in_use)
    }

    fn current(&self) -> &str {
        self.chosen.as_deref().unwrap_or_else(|| self.untouched())
    }

    pub(crate) fn pick(&self) -> Option<AccountPick> {
        let folder = self.chosen.as_deref().filter(|folder| *folder != self.untouched())?;
        Some(AccountPick { agent: self.choices.agent.clone(), folder: folder.to_string() })
    }

    pub(crate) fn follows_in_use(&self) -> bool {
        self.saved.is_none() && self.current() == self.choices.in_use
    }

    pub(crate) fn gone(&self) -> Option<String> {
        let current = self.current();
        let listed = self.choices.accounts.iter().any(|account| account.folder == current);
        (!listed).then(|| format!("The {} account {current} is gone: pick another account", self.choices.agent))
    }

    fn note(&self, folder: &str) -> String {
        let own = folder.is_empty().then_some("own login");
        let in_use = (folder == self.choices.in_use).then_some("in use");
        own.into_iter().chain(in_use).collect::<Vec<_>>().join(" · ")
    }

    pub(crate) fn rows(&self) -> Vec<PickRow> {
        let row = |account: &AccountChoice| PickRow {
            folder: account.folder.clone(),
            name: account.name.clone(),
            note: self.note(&account.folder),
            chosen: account.folder == self.current(),
        };
        self.choices.accounts.iter().map(row).collect()
    }

    pub(crate) fn shown(&self) -> PickRow {
        self.rows().into_iter().find(|row| row.chosen).unwrap_or_else(|| PickRow {
            folder: self.current().to_string(),
            name: format!("{} (gone)", self.current()),
            note: String::new(),
            chosen: true,
        })
    }

    pub(crate) fn chosen_at(&self) -> Option<usize> {
        self.choices.accounts.iter().position(|account| account.folder == self.current())
    }
}

#[cfg(test)]
mod tests;
