use std::time::Instant;

use anyhow::Result;
use gpui::Context;
use serde_json::{Value, json};

use super::cdp::{Cdp, Event};
use super::engine::Engine;
use super::frame::Shot;
use super::tabs::{Followup, Tab, history_steps};
use super::{Browser, Status, address, follow, viewport};

pub(crate) const BRIDGE: &str = "cornercaseBridge";
const PAGE_SCRIPT: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/browser/page.js"));

pub(crate) fn plain_agent(agent: &str) -> String {
    agent.replace("HeadlessChrome/", "Chrome/")
}

pub(crate) fn bridged(params: &Value) -> Option<Value> {
    if params["name"].as_str() != Some(BRIDGE) {
        return None;
    }
    params["payload"].as_str().and_then(|p| serde_json::from_str::<Value>(p).ok())
}

impl Browser {
    pub(super) fn cdp(&self) -> Option<Cdp> {
        match &self.status {
            Status::Ready(engine) => Some(engine.cdp.clone()),
            _ => None,
        }
    }

    pub(super) fn port(&self) -> Option<u16> {
        match &self.status {
            Status::Ready(engine) => Some(engine.port),
            _ => None,
        }
    }

    pub(super) fn send(&self, session: Option<&str>, method: &str, params: Value) {
        if let Some(cdp) = self.cdp() {
            cdp.send(session, method, params);
        }
    }

    pub(super) fn call(
        &self,
        (session, method, params): (Option<&str>, &str, Value),
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, Result<Value>, &mut Context<Self>) + 'static,
    ) {
        let Some(cdp) = self.cdp() else { return };
        let reply = cdp.call(session, method, params);
        cx.spawn(async move |this, cx| {
            let result = reply.await.unwrap_or_else(|_| Err(anyhow::anyhow!("the browser went away")));
            let _ = this.update(cx, |this, cx| then(this, result, cx));
        })
        .detach();
    }

    pub(super) fn active_session(&self) -> Option<String> {
        self.tabs.active(self.workspace).and_then(|t| t.session.clone())
    }

    pub(super) fn ready(&mut self, engine: Engine, cx: &mut Context<Self>) {
        self.status = Status::Ready(engine);
        self.attempts = 0;
        self.agents.follow.ready(Instant::now());
        self.allow_downloads();
        self.call((None, "Browser.getVersion", json!({})), cx, |this, result, cx| {
            this.user_agent = result.ok().and_then(|v| v["userAgent"].as_str().map(plain_agent));
            this.send(None, "Target.setDiscoverTargets", json!({ "discover": true }));
            for (url, workspace) in std::mem::take(&mut this.restore) {
                this.open_tab(&url, workspace, cx);
            }
        });
    }

    pub(super) fn open_tab(&mut self, url: &str, workspace: Option<u64>, cx: &mut Context<Self>) {
        self.call((None, "Target.createTarget", json!({ "url": url })), cx, move |this, result, cx| {
            let Some(target) = result.ok().and_then(|r| r["targetId"].as_str().map(str::to_string)) else { return };
            this.claim(&target, workspace, cx);
        });
    }

    fn claim(&mut self, target: &str, workspace: Option<u64>, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get_mut(target) else {
            self.claims.insert(target.to_string(), workspace);
            return;
        };
        tab.workspace = workspace;
        self.tabs.activate(workspace, target);
        self.recast(cx);
    }

    pub(super) fn event(&mut self, event: &Event, cx: &mut Context<Self>) {
        self.notice(event, cx);
        self.agents_event(event, cx);
        match event.method.as_str() {
            "Target.targetCreated" => self.target_created(&event.params, cx),
            "Runtime.bindingCalled" => self.binding(event, cx),
            "Page.loadEventFired" => self.refresh_title(event.session.as_deref(), cx),
            _ => {}
        }
        match self.tabs.apply(event) {
            Followup::Navigated(target) => self.refresh_history(&target, cx),
            Followup::Closed(_) => self.recast(cx),
            Followup::Nothing => {}
        }
    }

    fn target_created(&mut self, params: &Value, cx: &mut Context<Self>) {
        let info = &params["targetInfo"];
        let (Some(target), Some("page")) = (info["targetId"].as_str(), info["type"].as_str()) else { return };
        if self.tabs.get(target).is_some() {
            return;
        }
        let opener = info["openerId"].as_str().and_then(|o| self.tabs.get(o)).map(|o| o.workspace);
        let claimed = self.claims.remove(target);
        let workspace = claimed.or(opener).flatten();
        let mut tab = Tab::new(target, info["url"].as_str().unwrap_or_default(), workspace);
        tab.title = info["title"].as_str().unwrap_or_default().to_string();
        self.tabs.add(tab);
        if claimed.is_some() || opener.is_some() {
            self.tabs.activate(workspace, target);
        }
        self.adopt(target, follow::foreign(claimed.is_some(), workspace), cx);
        self.attach(target, cx);
    }

    fn attach(&mut self, target: &str, cx: &mut Context<Self>) {
        let params = json!({ "targetId": target, "flatten": true });
        let target = target.to_string();
        self.call((None, "Target.attachToTarget", params), cx, move |this, result, cx| {
            let Some(session) = result.ok().and_then(|r| r["sessionId"].as_str().map(str::to_string)) else { return };
            if let Some(tab) = this.tabs.get_mut(&target) {
                tab.session = Some(session.clone());
                this.set_up(&session);
                this.refresh_history(&target, cx);
                this.check_loading(&session, cx);
                this.recast(cx);
            }
        });
    }

    fn set_up(&self, session: &str) {
        let s = Some(session);
        self.send(s, "Page.enable", json!({}));
        self.send(s, "Runtime.enable", json!({}));
        self.set_up_notices(session);
        self.send(s, "Emulation.setFocusEmulationEnabled", json!({ "enabled": true }));
        self.send(s, "Runtime.addBinding", json!({ "name": BRIDGE }));
        self.send(s, "Page.addScriptToEvaluateOnNewDocument", json!({ "source": PAGE_SCRIPT }));
        self.send(s, "Runtime.evaluate", json!({ "expression": PAGE_SCRIPT }));
        self.size_page(session);
    }

    fn binding(&mut self, event: &Event, cx: &mut Context<Self>) {
        let Some(message) = bridged(&event.params) else { return };
        let from_active = event.session.is_some() && event.session == self.active_session();
        if message["kind"].as_str() == Some("cursor") && from_active {
            self.cursor = viewport::cursor_style(message["cursor"].as_str().unwrap_or_default());
            cx.notify();
        }
        if from_active {
            self.input_message(&message, event, cx);
        }
    }

    fn refresh_history(&mut self, target: &str, cx: &mut Context<Self>) {
        let Some(session) = self.tabs.get(target).and_then(|t| t.session.clone()) else { return };
        let target = target.to_string();
        self.call((Some(&session), "Page.getNavigationHistory", json!({})), cx, move |this, result, cx| {
            let Ok(history) = result else { return };
            if let Some(tab) = this.tabs.get_mut(&target) {
                (tab.back, tab.forward) = history_steps(&history);
                cx.notify();
            }
        });
    }

    pub(super) fn frame(&mut self, session: &str, shot: Shot, cx: &mut Context<Self>) {
        let Some(target) = self.tabs.by_session(session).map(|t| t.target.clone()) else { return };
        if let Some(old) = self.tabs.get_mut(&target).and_then(|t| t.shot.replace(shot)) {
            self.stale.borrow_mut().push(old.image);
        }
        if self.casting.as_deref() == Some(target.as_str()) {
            cx.notify();
        }
    }

    pub(super) fn go_to(&mut self, text: &str, cx: &mut Context<Self>) {
        let Some(url) = address::normalize(text).map(|url| self.tunnelled(url)) else { return };
        match self.active_session() {
            Some(session) => self.send(Some(&session), "Page.navigate", json!({ "url": url })),
            None => self.open_tab(&url, self.workspace, cx),
        }
    }

    fn step(&mut self, entry: Option<i64>) {
        if let (Some(session), Some(entry)) = (self.active_session(), entry) {
            self.send(Some(&session), "Page.navigateToHistoryEntry", json!({ "entryId": entry }));
        }
    }

    pub(super) fn back(&mut self, _cx: &mut Context<Self>) {
        self.step(self.tabs.active(self.workspace).and_then(|t| t.back));
    }

    pub(super) fn forward(&mut self, _cx: &mut Context<Self>) {
        self.step(self.tabs.active(self.workspace).and_then(|t| t.forward));
    }

    pub(super) fn reload(&mut self, _cx: &mut Context<Self>) {
        if let Some(session) = self.active_session() {
            self.send(Some(&session), "Page.reload", json!({}));
        }
    }

    pub(super) fn close_tab(&mut self, target: &str) {
        self.send(None, "Target.closeTarget", json!({ "targetId": target }));
    }

    pub(super) fn show_tab(&mut self, target: &str, cx: &mut Context<Self>) {
        self.tabs.activate(self.workspace, target);
        self.recast(cx);
        cx.notify();
    }

    pub(super) fn recast(&mut self, cx: &mut Context<Self>) {
        let wanted = if self.shown { self.tabs.active_target(self.workspace) } else { None };
        let session = |this: &Self, target: &Option<String>| {
            target.as_deref().and_then(|t| this.tabs.get(t)).and_then(|t| t.session.clone())
        };
        if wanted == self.casting && session(self, &wanted).is_some() {
            return;
        }
        if let Some(old) = session(self, &self.casting) {
            self.send(Some(&old), "Page.stopScreencast", json!({}));
        }
        self.casting = None;
        if let Some(new) = session(self, &wanted) {
            self.size_page(&new);
            self.start_cast(&new);
            self.casting = wanted;
        }
        cx.notify();
    }

    pub(super) fn start_cast(&self, session: &str) {
        let (width, height) = self.view.device_pixels();
        let params =
            json!({ "format": "jpeg", "quality": 85, "maxWidth": width, "maxHeight": height, "everyNthFrame": 1 });
        self.send(Some(session), "Page.startScreencast", params);
    }

    pub(super) fn size_page(&self, session: &str) {
        if let Some(metrics) = self.view.metrics() {
            self.send(Some(session), "Emulation.setDeviceMetricsOverride", metrics);
        }
        self.emulate(session);
    }
}

#[cfg(test)]
mod tests;
