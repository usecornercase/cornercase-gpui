use std::collections::HashSet;
use std::time::{Duration, Instant};

use gpui::Context;

use super::Browser;
use super::cdp::Event;
use super::tabs::Tabs;

const GRACE: Duration = Duration::from_secs(2);

#[derive(Default)]
pub(crate) struct Follow {
    pub(crate) off: bool,
    pub(crate) foreign: HashSet<String>,
    pub(crate) engaged: bool,
    since: Option<Instant>,
}

impl Follow {
    pub(crate) fn ready(&mut self, now: Instant) {
        self.since = Some(now);
    }

    pub(crate) fn follows(&self, now: Instant, busy: bool) -> bool {
        let settled = self.since.is_some_and(|since| now.saturating_duration_since(since) >= GRACE);
        settled && !self.off && !self.engaged && !busy
    }
}

pub(crate) fn foreign(claimed: bool, workspace: Option<u64>) -> bool {
    !claimed && workspace.is_none()
}

pub(crate) fn agents_tab(foreign: &HashSet<String>, tabs: &Tabs, target: &str) -> bool {
    let unclaimed = tabs.get(target).is_some_and(|t| t.workspace.is_none());
    unclaimed && foreign.contains(target)
}

fn page(url: &str) -> &str {
    url.split('#').next().unwrap_or(url)
}

pub(crate) fn moved(old: &str, new: &str) -> bool {
    !new.is_empty() && page(old) != page(new)
}

impl Browser {
    pub(super) fn adopt(&mut self, target: &str, foreign: bool, cx: &mut Context<Self>) {
        if !foreign {
            return;
        }
        self.agents.follow.foreign.insert(target.to_string());
        if self.agents.follow.follows(Instant::now(), self.agents.pick.is_some()) {
            self.tabs.activate(self.workspace, target);
            self.recast(cx);
        }
    }

    pub(super) fn agent_tab(&self, target: &str) -> bool {
        agents_tab(&self.agents.follow.foreign, &self.tabs, target)
    }

    pub(super) fn agents_event(&mut self, event: &Event, cx: &mut Context<Self>) {
        match event.method.as_str() {
            method if method.starts_with("Overlay.") => self.overlay_event(event, cx),
            "Page.frameNavigated" => self.agent_navigated(event, cx),
            "Target.targetDestroyed" => {
                let target = event.params["targetId"].as_str().unwrap_or_default();
                self.agents.follow.foreign.remove(target);
            }
            _ => {}
        }
    }

    fn agent_navigated(&mut self, event: &Event, cx: &mut Context<Self>) {
        let frame = &event.params["frame"];
        let tab =
            event.session.as_deref().and_then(|s| self.tabs.by_session(s)).filter(|_| frame.get("parentId").is_none());
        let Some(tab) = tab.filter(|t| self.agent_tab(&t.target)) else { return };
        let shown = self.tabs.active_target(self.workspace).as_deref() == Some(tab.target.as_str());
        let now = Instant::now();
        if !shown
            && moved(&tab.url, frame["url"].as_str().unwrap_or_default())
            && self.agents.follow.follows(now, self.agents.pick.is_some())
        {
            let target = tab.target.clone();
            self.show_tab(&target, cx);
        }
    }
}

#[cfg(test)]
mod tests;
