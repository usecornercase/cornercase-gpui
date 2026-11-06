use serde::Deserialize;
use serde_json::Value;

use crate::panels::flows::model::epoch;
use crate::ui::Icon;

pub(crate) mod run_on;
pub(crate) mod seen;

pub(crate) const THIS_COMPUTER: &str = "this computer";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(from = "String")]
pub(crate) enum MachineIcon {
    #[default]
    Server,
    Laptop,
    Desktop,
    Mini,
    HomeServer,
    Other,
}

impl MachineIcon {
    pub(crate) const ALL: [Self; 6] =
        [Self::Server, Self::Laptop, Self::Desktop, Self::Mini, Self::HomeServer, Self::Other];

    pub(crate) fn glyph(self) -> &'static str {
        match self {
            Self::Server => "☁",
            Self::Laptop => "▭",
            Self::Desktop => "▣",
            Self::Mini => "◫",
            Self::HomeServer => "⌂",
            Self::Other => "◆",
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Server => "server",
            Self::Laptop => "laptop",
            Self::Desktop => "desktop",
            Self::Mini => "mini",
            Self::HomeServer => "home server",
            Self::Other => "other",
        }
    }

    pub(crate) fn icon(self) -> Icon {
        match self {
            Self::Server => Icon::Cloud,
            Self::Laptop => Icon::Laptop,
            Self::Desktop => Icon::Monitor,
            Self::Mini => Icon::Worktrees,
            Self::HomeServer => Icon::House,
            Self::Other => Icon::Diamond,
        }
    }
}

impl From<String> for MachineIcon {
    fn from(glyph: String) -> Self {
        Self::ALL.into_iter().find(|icon| icon.glyph() == glyph).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum LinkState {
    Linked,
    Unreachable,
    Older,
    #[default]
    #[serde(other)]
    Connecting,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Machine {
    pub name: String,
    pub icon: MachineIcon,
    pub destination: String,
    pub folder: String,
    pub state: LinkState,
    pub version: Option<String>,
    pub server_version: Option<String>,
    pub seen: Option<String>,
    pub error: Option<String>,
    pub updating: bool,
    pub problem: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Here {
    pub icon: MachineIcon,
    pub version: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct MachineList {
    pub here: Here,
    pub machines: Vec<Machine>,
}

pub(crate) fn parse_list(text: &str) -> anyhow::Result<MachineList> {
    Ok(serde_json::from_str(text)?)
}

pub(crate) fn in_app(app: &Value) -> Vec<Machine> {
    app.get("machines").and_then(|machines| Vec::deserialize(machines).ok()).unwrap_or_default()
}

fn ago(seconds: i64) -> String {
    match seconds.max(0) {
        seconds @ 0..60 => format!("{seconds} s"),
        seconds @ 60..3_600 => format!("{} min", seconds / 60),
        seconds @ 3_600..86_400 => format!("{} h", seconds / 3_600),
        86_400..172_800 => "1 day".to_string(),
        seconds => format!("{} days", seconds / 86_400),
    }
}

fn seen_ago(seen: Option<&str>, now: i64) -> Option<String> {
    let seen = epoch(seen?)?;
    Some(format!("seen {} ago", ago(now - seen)))
}

impl Machine {
    pub(crate) fn linked(&self) -> bool {
        self.problem.is_none() && self.state == LinkState::Linked
    }

    pub(crate) fn can_retry(&self) -> bool {
        self.problem.is_none() && matches!(self.state, LinkState::Unreachable | LinkState::Older)
    }

    pub(crate) fn can_update(&self) -> bool {
        self.problem.is_none() && self.state == LinkState::Older && !self.updating
    }

    pub(crate) fn in_progress(&self) -> bool {
        self.problem.is_none() && (self.updating || self.state == LinkState::Connecting)
    }

    pub(crate) fn counts_up(&self) -> bool {
        self.problem.is_none() && self.state == LinkState::Unreachable && self.seen.is_some()
    }

    pub(crate) fn seen_ago(&self, now: i64) -> Option<String> {
        seen_ago(self.seen.as_deref(), now)
    }

    pub(crate) fn words(&self, now: i64) -> String {
        if self.problem.is_some() {
            return "problem · no link".to_string();
        }
        if self.updating {
            return "updating…".to_string();
        }
        match (self.state, self.version.as_deref()) {
            (LinkState::Connecting, _) => "connecting…".to_string(),
            (LinkState::Linked, Some(version)) => format!("linked · {version}"),
            (LinkState::Linked, None) => "linked".to_string(),
            (LinkState::Older, Some(version)) => format!("older · {version}"),
            (LinkState::Older, None) => "older".to_string(),
            (LinkState::Unreachable, _) => {
                format!("unreachable · {}", self.seen_ago(now).unwrap_or_else(|| "not seen yet".to_string()))
            }
        }
    }

    pub(crate) fn server_behind(&self) -> Option<String> {
        let running = self.server_version.as_deref().filter(|running| self.version.as_deref() != Some(*running))?;
        self.linked().then(|| format!("Its server runs {running} until it restarts there"))
    }
}

#[cfg(test)]
mod tests;
