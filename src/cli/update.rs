use serde::Deserialize;

use super::partial::said;
use super::{Answer, Older, Said, Version, reason, update_args};

pub(crate) const UPDATE_JSON_SINCE: Version = Version::new(0, 13, 53);

const REFUSED: i32 = 2;
const BY_HAND: i32 = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ByHand {
    pub(crate) version: String,
    pub(crate) latest: String,
    pub(crate) command: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UpdateAnswer {
    Updated { from: String, version: String },
    UpToDate { version: String },
    Manual(ByHand),
    Refused,
    Printed(Said),
}

#[derive(Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
enum Line {
    Updated { from: String, version: String },
    UpToDate { version: String },
    Manual { version: String, latest: String, command: String },
}

impl From<Line> for UpdateAnswer {
    fn from(line: Line) -> Self {
        match line {
            Line::Updated { from, version } => Self::Updated { from, version },
            Line::UpToDate { version } => Self::UpToDate { version },
            Line::Manual { version, latest, command } => Self::Manual(ByHand { version, latest, command }),
        }
    }
}

impl UpdateAnswer {
    pub(crate) fn ended(&self) -> bool {
        matches!(self, Self::Updated { .. } | Self::UpToDate { .. } | Self::Printed(Said::Done(_)))
    }
}

pub(crate) fn refused_json() -> String {
    format!("cornercase refused update --json, which needs cornercase {UPDATE_JSON_SINCE} or newer")
}

fn line_of(answer: &Answer) -> Option<UpdateAnswer> {
    if !matches!(answer.code, Some(0 | BY_HAND)) {
        return None;
    }
    answer.stdout.lines().find_map(|line| serde_json::from_str::<Line>(line).ok()).map(UpdateAnswer::from)
}

pub(crate) fn update_answer(result: &anyhow::Result<Answer>) -> UpdateAnswer {
    let answer = match result {
        Ok(answer) => answer,
        Err(e) => return UpdateAnswer::Printed(Said::Failed(reason(e))),
    };
    if answer.code == Some(REFUSED) {
        return UpdateAnswer::Refused;
    }
    line_of(answer).unwrap_or_else(|| UpdateAnswer::Printed(said(answer)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UpdateGate {
    Here(Option<Version>),
    Through { here: Option<Version>, there: Option<Version> },
}

impl UpdateGate {
    pub(crate) fn of(older: &Older) -> Self {
        Self::Through { here: Version::parse(&older.here), there: older.there.as_deref().and_then(Version::parse) }
    }

    pub(crate) fn here(self) -> Option<Version> {
        match self {
            Self::Here(here) | Self::Through { here, .. } => here,
        }
    }

    pub(crate) fn lacks(self) -> bool {
        let lacks = |release| !UPDATE_JSON_SINCE.reached_by(release);
        match self {
            Self::Here(here) => lacks(here),
            Self::Through { here, there } => lacks(here) || lacks(there),
        }
    }

    pub(crate) fn args(self, mut words: Vec<String>) -> Vec<String> {
        if !self.lacks() {
            words.push("--json".to_string());
        }
        words
    }
}

pub(crate) fn update_command(older: &Older) -> Option<Vec<String>> {
    update_args(older).map(|words| UpdateGate::of(older).args(words))
}

#[cfg(test)]
mod tests;
