use super::Question;
use crate::model::{PaneInfo, Tab};
use crate::store::Busy;

pub(crate) struct CloseWords {
    pub(crate) title: String,
    pub(crate) message: String,
    pub(crate) button: &'static str,
    pub(crate) doing: String,
}

fn on_machine(on: Option<&str>) -> String {
    on.map(|machine| format!(" on {machine}")).unwrap_or_default()
}

impl CloseWords {
    pub(crate) fn pane(info: Option<&PaneInfo>, on: Option<&str>) -> Self {
        let running = info.and_then(PaneInfo::running);
        let place = on_machine(on);
        let what = running.clone().unwrap_or_else(|| "its shell".into());
        let named = running.unwrap_or_else(|| "a shell".into());
        Self {
            title: format!("Close this pane{place}?"),
            message: format!("It runs {what}: closing the pane stops it."),
            button: "Close pane",
            doing: format!("Closing {named}{place}…"),
        }
    }

    pub(crate) fn tab(tab: &Tab, agent: Option<&PaneInfo>, on: Option<&str>) -> Self {
        let what = agent.and_then(|a| a.agent.clone()).unwrap_or_else(|| "an agent".into());
        let state = agent.and_then(|a| a.status.as_deref()).map(|s| format!(" ({s})")).unwrap_or_default();
        let (place, there) = (on_machine(on), if on.is_some() { " there" } else { "" });
        Self {
            title: format!("Close the tab {}{place}?", tab.name),
            message: format!("It runs {what}{state}: closing the tab stops it. Its worktree and branch stay{there}."),
            button: "Close tab",
            doing: format!("Closing {}{place}…", tab.name),
        }
    }

    pub(crate) fn question(&self, command: Vec<String>) -> Question {
        Question::new(self.title.clone(), self.message.clone(), self.button, command)
    }

    pub(crate) fn busy(&self, key: String) -> Busy {
        Busy::new(key, self.doing.clone())
    }
}

#[cfg(test)]
mod tests;
