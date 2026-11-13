use std::collections::HashMap;

use serde_json::Value;

use super::cdp::Event;
use super::frame::Shot;
use crate::model::Owner;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Through {
    pub(crate) remote: u16,
    pub(crate) local: u16,
}

pub(crate) struct Tab {
    pub(crate) target: String,
    pub(crate) session: Option<String>,
    pub(crate) owner: Owner,
    pub(crate) through: Option<Through>,
    pub(crate) url: String,
    pub(crate) title: String,
    pub(crate) loading: bool,
    pub(crate) main_frame: Option<String>,
    pub(crate) back: Option<i64>,
    pub(crate) forward: Option<i64>,
    pub(crate) shot: Option<Shot>,
}

impl Tab {
    pub(crate) fn new(target: &str, url: &str, owner: Owner) -> Self {
        Self {
            target: target.to_string(),
            session: None,
            owner,
            through: None,
            url: url.to_string(),
            title: String::new(),
            loading: true,
            main_frame: None,
            back: None,
            forward: None,
            shot: None,
        }
    }

    pub(crate) fn holds_local(&self, port: u16) -> bool {
        self.through.is_some_and(|through| through.local == port)
    }

    pub(crate) fn label(&self) -> &str {
        if self.title.trim().is_empty() { &self.url } else { &self.title }
    }

    fn is_main(&self, frame: Option<&str>) -> bool {
        frame.is_some() && self.main_frame.as_deref() == frame
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Claim {
    pub(crate) owner: Owner,
    pub(crate) through: Option<Through>,
}

#[derive(Debug, Clone)]
pub(crate) struct Restore {
    pub(crate) url: String,
    pub(crate) owner: Owner,
    pub(crate) through: Option<Through>,
}

impl Restore {
    pub(crate) fn of(tab: &Tab) -> Self {
        Self { url: tab.url.clone(), owner: tab.owner.clone(), through: tab.through }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Followup {
    Nothing,
    Navigated(String),
    Closed(String),
}

#[derive(Default)]
pub(crate) struct Tabs {
    pub(crate) list: Vec<Tab>,
    active: HashMap<Owner, String>,
}

fn text<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    path.iter().try_fold(value, |v, key| v.get(key))?.as_str()
}

impl Tabs {
    pub(crate) fn shown(&self, owner: &Owner) -> impl Iterator<Item = &Tab> {
        let owner = owner.clone();
        self.list.iter().filter(move |t| t.owner == Owner::Nobody || t.owner == owner)
    }

    pub(crate) fn active(&self, owner: &Owner) -> Option<&Tab> {
        let chosen = self.active.get(owner);
        self.shown(owner).find(|t| Some(&t.target) == chosen).or_else(|| self.shown(owner).last())
    }

    pub(crate) fn active_target(&self, owner: &Owner) -> Option<String> {
        self.active(owner).map(|t| t.target.clone())
    }

    pub(crate) fn activate(&mut self, owner: Owner, target: &str) {
        self.active.insert(owner, target.to_string());
    }

    pub(crate) fn rekey(&mut self, from: &Owner, to: &Owner) {
        for tab in self.list.iter_mut().filter(|t| t.owner == *from) {
            tab.owner = to.clone();
        }
        if let Some(chosen) = self.active.remove(from) {
            self.active.insert(to.clone(), chosen);
        }
    }

    pub(crate) fn get(&self, target: &str) -> Option<&Tab> {
        self.list.iter().find(|t| t.target == target)
    }

    pub(crate) fn get_mut(&mut self, target: &str) -> Option<&mut Tab> {
        self.list.iter_mut().find(|t| t.target == target)
    }

    pub(crate) fn by_session(&self, session: &str) -> Option<&Tab> {
        self.list.iter().find(|t| t.session.as_deref() == Some(session))
    }

    pub(super) fn by_session_mut(&mut self, session: Option<&str>) -> Option<&mut Tab> {
        let session = session?;
        self.list.iter_mut().find(|t| t.session.as_deref() == Some(session))
    }

    pub(crate) fn add(&mut self, tab: Tab) {
        if self.get(&tab.target).is_none() {
            self.list.push(tab);
        }
    }

    pub(crate) fn remove(&mut self, target: &str) -> Option<Tab> {
        let at = self.list.iter().position(|t| t.target == target)?;
        self.active.retain(|_, chosen| chosen != target);
        Some(self.list.remove(at))
    }

    pub(crate) fn apply(&mut self, event: &Event) -> Followup {
        let p = &event.params;
        match event.method.as_str() {
            "Target.targetInfoChanged" => self.info_changed(p),
            "Target.targetDestroyed" => text(p, &["targetId"]).map_or(Followup::Nothing, |t| self.destroyed(t)),
            "Page.frameNavigated" => self.navigated(event.session.as_deref(), p),
            "Page.navigatedWithinDocument" => self.moved_within(event.session.as_deref(), p),
            "Page.frameStartedLoading" | "Page.frameStoppedLoading" => self.loading(event, p),
            "Page.loadEventFired" => {
                self.by_session_mut(event.session.as_deref()).into_iter().for_each(|t| t.loading = false);
                Followup::Nothing
            }
            _ => Followup::Nothing,
        }
    }

    fn info_changed(&mut self, p: &Value) -> Followup {
        let info = &p["targetInfo"];
        let Some(tab) = text(info, &["targetId"]).and_then(|t| self.get_mut(t)) else { return Followup::Nothing };
        if let Some(title) = text(info, &["title"]) {
            tab.title = title.to_string();
        }
        if let Some(url) = text(info, &["url"]) {
            tab.url = url.to_string();
        }
        Followup::Nothing
    }

    fn destroyed(&mut self, target: &str) -> Followup {
        match self.remove(target) {
            Some(_) => Followup::Closed(target.to_string()),
            None => Followup::Nothing,
        }
    }

    fn navigated(&mut self, session: Option<&str>, p: &Value) -> Followup {
        let frame = &p["frame"];
        if frame.get("parentId").is_some() {
            return Followup::Nothing;
        }
        let Some(tab) = self.by_session_mut(session) else { return Followup::Nothing };
        tab.main_frame = text(frame, &["id"]).map(str::to_string);
        if let Some(url) = text(frame, &["url"]) {
            tab.url = url.to_string();
        }
        Followup::Navigated(tab.target.clone())
    }

    fn moved_within(&mut self, session: Option<&str>, p: &Value) -> Followup {
        let Some(tab) = self.by_session_mut(session).filter(|t| t.is_main(text(p, &["frameId"]))) else {
            return Followup::Nothing;
        };
        if let Some(url) = text(p, &["url"]) {
            tab.url = url.to_string();
        }
        Followup::Navigated(tab.target.clone())
    }

    fn loading(&mut self, event: &Event, p: &Value) -> Followup {
        let started = event.method == "Page.frameStartedLoading";
        if let Some(tab) = self.by_session_mut(event.session.as_deref()) {
            let main = tab.main_frame.is_none() || tab.is_main(text(p, &["frameId"]));
            if main {
                tab.loading = started;
            }
        }
        Followup::Nothing
    }
}

pub(crate) fn history_steps(history: &Value) -> (Option<i64>, Option<i64>) {
    let current = history["currentIndex"].as_i64().unwrap_or(0);
    let entries = history["entries"].as_array().map(Vec::as_slice).unwrap_or_default();
    let id = |at: i64| usize::try_from(at).ok().and_then(|at| entries.get(at)).and_then(|e| e["id"].as_i64());
    (id(current - 1), id(current + 1))
}

#[cfg(test)]
mod tests;
