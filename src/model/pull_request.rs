use serde::Deserialize;

use super::{Session, Workspace};

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct PullRequest {
    pub number: u64,
    pub title: String,
    pub url: String,
    pub state: String,
    pub checks: Option<String>,
    pub failed: Vec<String>,
    pub conflicts: bool,
    pub base: Option<String>,
    pub provider: String,
    pub stack: Option<Stack>,
    pub all_checks: Option<Vec<Check>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Check {
    pub name: String,
    pub workflow: String,
    pub outcome: String,
    pub url: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct PullRequestAnswer {
    pub workspace: u64,
    pub branch: Option<String>,
    pub pull_request: Option<PullRequest>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Stack {
    pub number: u64,
    pub base: String,
    pub position: usize,
    pub size: usize,
    pub layers: Vec<Layer>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Layer {
    pub number: u64,
    pub title: String,
    pub url: String,
    pub state: String,
    pub checks: Option<String>,
    pub conflicts: bool,
    pub review: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Block {
    Conflicts,
    Checks,
    Review,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Rung<'a> {
    pub position: usize,
    pub layer: &'a Layer,
    pub own: bool,
}

const OUTCOMES: [&str; 4] = ["failed", "pending", "passed", "skipped"];

const ROLLUPS: [(&str, &str); 3] = [("failed", "failing"), ("pending", "pending"), ("passed", "passing")];

fn settled(state: &str) -> bool {
    matches!(state, "merged" | "closed")
}

impl Check {
    fn named(&self) -> String {
        if self.url.is_empty() { self.name.clone() } else { format!("{} {}", self.name, self.url) }
    }
}

impl PullRequest {
    pub(crate) fn on_gitlab(&self) -> bool {
        self.provider == "gitlab"
    }

    pub(crate) fn noun(&self) -> &'static str {
        if self.on_gitlab() { "merge request" } else { "pull request" }
    }

    pub(crate) fn reference(&self) -> String {
        format!("{}{}", if self.on_gitlab() { "!" } else { "#" }, self.number)
    }

    pub(crate) fn label(&self) -> String {
        format!("{} {}", self.noun(), self.reference())
    }

    pub(crate) fn heading(&self) -> String {
        let noun = if self.on_gitlab() { "Merge request" } else { "Pull request" };
        format!("{noun} {}", self.reference())
    }

    pub(crate) fn site(&self) -> &'static str {
        if self.on_gitlab() { "GitLab" } else { "GitHub" }
    }

    pub(crate) fn checks_noun(&self) -> &'static str {
        if self.on_gitlab() { "pipeline" } else { "checks" }
    }

    pub(crate) fn settled(&self) -> bool {
        settled(&self.state)
    }

    pub(crate) fn base_name(&self) -> &str {
        self.base.as_deref().filter(|b| !b.is_empty()).unwrap_or("its base")
    }

    pub(crate) fn conflict_said(&self) -> Option<String> {
        self.conflicts.then(|| format!("conflicts with {}", self.base_name()))
    }

    pub(crate) fn rollup(&self) -> Option<&str> {
        let Some(all) = &self.all_checks else { return self.checks.as_deref() };
        ROLLUPS.into_iter().find(|(outcome, _)| all.iter().any(|c| c.outcome == *outcome)).map(|(_, rollup)| rollup)
    }

    fn counts(&self, all: &[Check]) -> String {
        let counted =
            OUTCOMES.into_iter().map(|outcome| (all.iter().filter(|c| c.outcome == outcome).count(), outcome));
        let counts: Vec<String> =
            counted.filter(|(n, _)| *n > 0).map(|(n, outcome)| format!("{n} {outcome}")).collect();
        if counts.is_empty() { format!("no {}", self.checks_noun()) } else { counts.join(", ") }
    }

    pub(crate) fn checks_said(&self) -> String {
        if let Some(all) = &self.all_checks {
            return self.counts(all);
        }
        match &self.checks {
            Some(checks) => format!("{} {checks}", self.checks_noun()),
            None => format!("no {}", self.checks_noun()),
        }
    }

    pub(crate) fn summary(&self) -> String {
        let mut parts = vec![self.state.clone()];
        parts.extend(self.conflict_said());
        parts.push(self.checks_said());
        parts.extend(self.stack.as_ref().map(|s| format!("layer {} of {} of stack #{}", s.position, s.size, s.number)));
        parts.join(" · ")
    }

    fn failed_said(&self) -> Option<String> {
        let failed: Vec<String> = match &self.all_checks {
            Some(all) => all.iter().filter(|c| c.outcome == "failed").map(Check::named).collect(),
            None => self.failed.clone(),
        };
        (!failed.is_empty()).then(|| failed.join(", "))
    }

    fn link_said(&self) -> String {
        if self.all_checks.is_some() || self.url.is_empty() { String::new() } else { format!(" ({})", self.url) }
    }

    pub(crate) fn problems(&self) -> Option<String> {
        if self.settled() {
            return None;
        }
        let mut parts = Vec::new();
        if self.conflicts {
            parts.push(format!("has merge conflicts with {}", self.base_name()));
        }
        if let Some(failed) = self.failed_said() {
            parts.push(format!("failed its {}: {failed}", self.checks_noun()));
        }
        if parts.is_empty() {
            return None;
        }
        Some(format!("{} {}{} ", self.label(), parts.join(" and "), self.link_said()))
    }

    pub(crate) fn blocked(&self) -> Option<(&Layer, Block)> {
        if self.settled() {
            return None;
        }
        self.stack.as_ref()?.blocker(self.number)
    }
}

impl Layer {
    pub(crate) fn settled(&self) -> bool {
        settled(&self.state)
    }

    fn block(&self) -> Option<Block> {
        if self.settled() {
            None
        } else if self.conflicts {
            Some(Block::Conflicts)
        } else if self.checks.as_deref() == Some("failing") {
            Some(Block::Checks)
        } else if self.review.as_deref() == Some("changes_requested") {
            Some(Block::Review)
        } else {
            None
        }
    }
}

impl Stack {
    fn below(&self, number: u64) -> Option<&[Layer]> {
        let own = self.layers.iter().position(|l| l.number == number)?;
        Some(&self.layers[..own])
    }

    pub(crate) fn rungs(&self, number: u64) -> Vec<Rung<'_>> {
        let own = self.layers.iter().position(|l| l.number == number);
        let first = own.map_or(1, |at| self.position.saturating_sub(at).max(1));
        let rung = |(at, layer)| Rung { position: first + at, layer, own: Some(at) == own };
        self.layers.iter().enumerate().rev().map(rung).collect()
    }

    pub(crate) fn blocker(&self, number: u64) -> Option<(&Layer, Block)> {
        let below = self.below(number)?;
        let lowest = |wanted: fn(Block) -> bool| {
            below.iter().find_map(|layer| layer.block().filter(|b| wanted(*b)).map(|b| (layer, b)))
        };
        lowest(|b| b != Block::Review).or_else(|| lowest(|b| b == Block::Review))
    }

    pub(crate) fn merged_below(&self, number: u64) -> Option<&Layer> {
        self.below(number)?.iter().rev().find(|l| l.state == "merged")
    }
}

impl Session {
    pub(crate) fn pull_request_holder(&self, workspace: u64, url: &str) -> Option<&Workspace> {
        let (project, own) = self.workspace(workspace)?;
        let holds = |w: &&Workspace| w.pull_request.as_ref().is_some_and(|p| !url.is_empty() && p.url == url);
        Some(own).filter(holds).or_else(|| project.workspaces.iter().find(holds))
    }
}

#[cfg(test)]
mod tests;
