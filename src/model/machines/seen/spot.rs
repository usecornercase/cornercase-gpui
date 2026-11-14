use super::{ACTS_BOOT_SINCE, MachineSeen, ReadReleases, ThereId, ThereProject};
use crate::cli::{Version, with_boot};
use crate::model::Project;
use crate::store::args;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Spot {
    pub machine: String,
    pub boot: String,
    pub project: ThereId,
    pub name: String,
    pub path: String,
    pub root: Option<ThereId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LoginTab<'a> {
    pub agent: &'a str,
    pub account: Option<&'a str>,
    pub command: Option<&'a str>,
}

fn typed(word: &str) -> String {
    let plain = |c: char| c.is_ascii_alphanumeric() || "-_./:@%+=,".contains(c);
    if !word.is_empty() && word.chars().all(plain) {
        return word.to_string();
    }
    format!("'{}'", word.replace('\'', r"'\''"))
}

fn same_repo(there: Option<&str>, here: Option<&str>) -> bool {
    there.zip(here).is_some_and(|(there, here)| there.eq_ignore_ascii_case(here))
}

impl Spot {
    fn id(&self) -> String {
        self.project.0.to_string()
    }

    pub(crate) fn asking_args(&self, releases: ReadReleases) -> Vec<String> {
        let command = args(&["--project", &self.id()]);
        if releases.carry_boot() { with_boot(command, &self.boot) } else { command }
    }

    pub(crate) fn starting_args(&self) -> Vec<String> {
        args(&["--machine", &self.machine, "--project", &self.id()])
    }

    pub(crate) fn login_tab_args(&self, tab: LoginTab<'_>, release: Option<Version>) -> Option<Vec<String>> {
        let workspace = self.root?.0.to_string();
        let name = format!("{} login", tab.agent);
        let mut command = args(&["new-tab", "--workspace", &workspace, "--name", &name, "--json"]);
        if let Some(account) = tab.account {
            command.extend(args(&["--account", &format!("{}={account}", tab.agent)]));
        }
        if let Some(line) = tab.command {
            command.extend(["--".to_string(), typed(line)]);
        }
        Some(if ACTS_BOOT_SINCE.reached_by(release) { with_boot(command, &self.boot) } else { command })
    }
}

impl MachineSeen {
    pub(crate) fn pair_of(&self, ours: &Project) -> Option<&ThereProject> {
        let same = |there: &&ThereProject| {
            there.paired == Some(ours.id) || same_repo(there.repo.as_deref(), ours.repo.as_deref())
        };
        self.projects.iter().find(same)
    }

    pub(crate) fn project(&self, id: ThereId) -> Option<&ThereProject> {
        self.projects.iter().find(|there| there.id == id)
    }

    pub(crate) fn spot(&self, there: &ThereProject) -> Spot {
        let root = there.workspaces.iter().find(|ws| !ws.worktree).or_else(|| there.workspaces.first());
        Spot {
            machine: self.name.clone(),
            boot: self.boot.clone(),
            project: there.id,
            name: there.name.clone(),
            path: there.path.clone(),
            root: root.map(|ws| ws.id),
        }
    }
}

#[cfg(test)]
mod tests;
