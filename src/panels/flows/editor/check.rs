use crate::panels::flows::draft::Saved;
use crate::panels::flows::model::plural;

pub(in crate::panels::flows) enum Verdict {
    Unknown,
    Checked(Saved),
    Invalid(String),
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::panels::flows) enum Pill {
    Checking,
    Unchecked,
    Ready,
    Problems(usize),
}

impl Pill {
    pub(in crate::panels::flows) fn words(self) -> String {
        match self {
            Self::Checking => "Checking…".into(),
            Self::Unchecked => "Not checked".into(),
            Self::Ready => "Ready to save".into(),
            Self::Problems(count) => plural(count, "problem"),
        }
    }
}

pub(in crate::panels::flows) struct Check {
    verdict: Verdict,
    pending: bool,
    generation: u64,
}

impl Default for Check {
    fn default() -> Self {
        Self { verdict: Verdict::Unknown, pending: true, generation: 0 }
    }
}

impl Check {
    pub(in crate::panels::flows) fn changed(&mut self) -> u64 {
        self.generation += 1;
        self.pending = true;
        self.generation
    }

    pub(in crate::panels::flows) fn is_current(&self, generation: u64) -> bool {
        generation == self.generation
    }

    pub(in crate::panels::flows) fn answer(&mut self, generation: u64, verdict: Verdict) {
        if self.is_current(generation) {
            self.verdict = verdict;
            self.pending = false;
        }
    }

    pub(in crate::panels::flows) fn refuse(&mut self, error: String) {
        self.answer(self.generation, Verdict::Invalid(error));
    }

    pub(in crate::panels::flows) fn pill(&self) -> Pill {
        if self.pending {
            return Pill::Checking;
        }
        match &self.verdict {
            Verdict::Unknown => Pill::Checking,
            Verdict::Checked(saved) if saved.problems.is_empty() => Pill::Ready,
            Verdict::Checked(saved) => Pill::Problems(saved.problems.len()),
            Verdict::Invalid(_) => Pill::Problems(1),
            Verdict::Failed(_) => Pill::Unchecked,
        }
    }

    pub(in crate::panels::flows) fn rests(&self) -> bool {
        matches!(self.pill(), Pill::Problems(_))
    }

    pub(in crate::panels::flows) fn problems(&self) -> Vec<String> {
        match &self.verdict {
            Verdict::Unknown => Vec::new(),
            Verdict::Checked(saved) => saved.problems.clone(),
            Verdict::Invalid(error) | Verdict::Failed(error) => vec![error.clone()],
        }
    }

    pub(in crate::panels::flows) fn risk(&self) -> Option<&str> {
        match &self.verdict {
            Verdict::Checked(saved) => saved.check.as_deref(),
            Verdict::Unknown | Verdict::Invalid(_) | Verdict::Failed(_) => None,
        }
    }
}

#[cfg(test)]
mod tests;
