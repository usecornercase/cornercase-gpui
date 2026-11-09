use serde::{Deserialize, Serialize};

use crate::theme::resolve::ROOT;

pub(crate) const LIGHT: &str = "cornercase-light";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Choice {
    pub theme: String,
    pub follow_system: bool,
    pub light: String,
    pub dark: String,
}

impl Default for Choice {
    fn default() -> Self {
        Choice { theme: ROOT.to_string(), follow_system: false, light: LIGHT.to_string(), dark: ROOT.to_string() }
    }
}

impl Choice {
    pub(crate) fn parse(text: &str) -> Choice {
        serde_json::from_str(text).unwrap_or_default()
    }

    pub(crate) fn text(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default() + "\n"
    }

    pub(crate) fn in_use(&self, system_dark: bool) -> &str {
        match (self.follow_system, system_dark) {
            (false, _) => &self.theme,
            (true, true) => &self.dark,
            (true, false) => &self.light,
        }
    }

    pub(crate) fn choose(&mut self, id: &str, light: bool) {
        if !self.follow_system {
            self.theme = id.to_string();
        } else if light {
            self.light = id.to_string();
        } else {
            self.dark = id.to_string();
        }
    }

    pub(crate) fn follow(&mut self, on: bool, system_dark: bool, in_use_is_light: bool) {
        if on && !self.follow_system {
            let kept = self.in_use(system_dark).to_string();
            let slot = if in_use_is_light { &mut self.light } else { &mut self.dark };
            *slot = kept;
        }
        if !on && self.follow_system {
            self.theme = self.in_use(system_dark).to_string();
        }
        self.follow_system = on;
    }
}

#[cfg(test)]
mod tests;
