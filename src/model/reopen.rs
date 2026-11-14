use super::Workspace;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reopen {
    Closed(usize),
    Exited(usize),
}

impl Workspace {
    pub(crate) fn exited_roles(&self) -> &[String] {
        self.run.as_ref().map_or(&[], |run| run.exited.as_slice())
    }

    pub(crate) fn reopen(&self) -> Option<Reopen> {
        if let Some(n) = self.closed_agents.filter(|n| *n > 0) {
            return Some(Reopen::Closed(n));
        }
        Some(self.exited_roles().len()).filter(|n| *n > 0).map(Reopen::Exited)
    }

    pub(crate) fn reopen_for_run(&self, run_closed: bool) -> Option<Reopen> {
        self.reopen().filter(|_| run_closed || !self.exited_roles().is_empty())
    }
}

fn agents(n: usize) -> &'static str {
    if n == 1 { "agent" } else { "agents" }
}

impl Reopen {
    pub(crate) fn label(self) -> String {
        match self {
            Self::Closed(n) => format!("Reopen {n} closed {}", agents(n)),
            Self::Exited(n) => format!("Reopen {n} exited {}", agents(n)),
        }
    }

    pub(crate) fn closed_first(self) -> usize {
        match self {
            Self::Closed(n) => n,
            Self::Exited(_) => 0,
        }
    }
}

#[cfg(test)]
mod tests;
