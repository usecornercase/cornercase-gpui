use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};

use gpui::{
    AnyElement, ClickEvent, Context, FocusHandle, IntoElement, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, Window, div, px,
};
use serde_json::{Value, json};

use super::cdp::Event;
use super::console::Log;
use super::console_view::Drawer;
use super::downloads::Downloads;
use super::prompts::{self, Card, Dialog};
use super::{Browser, notice};
use crate::theme::colours;
use crate::ui::{self, Icon, icon};

#[derive(Default)]
pub(crate) struct Notices {
    pub(crate) logs: HashMap<String, Log>,
    pub(crate) drawer: Drawer,
    pub(crate) dialogs: HashMap<String, Dialog>,
    pub(crate) card: OnceCell<Card>,
    pub(crate) serial: u64,
    pub(crate) downloads: Downloads,
    panel: OnceCell<FocusHandle>,
    crashed: HashSet<String>,
    revived: HashSet<String>,
    openers: HashMap<String, String>,
}

impl Browser {
    pub(super) fn set_up_notices(&self, session: &str) {
        let s = Some(session);
        self.send(s, "Network.enable", json!({}));
        self.send(s, "Log.enable", json!({}));
        self.send(s, "Inspector.enable", json!({}));
        self.send(s, "Page.setInterceptFileChooserDialog", json!({ "enabled": true }));
    }

    pub(super) fn notice(&mut self, event: &Event, cx: &mut Context<Self>) {
        let p = &event.params;
        match event.method.as_str() {
            "Page.javascriptDialogOpening" => self.dialog_opening(event),
            "Page.javascriptDialogClosed" => self.dialog_closed(event),
            "Page.fileChooserOpened" => Self::choose_files(event, cx),
            "Browser.downloadWillBegin" => self.download_begins(p, cx),
            "Browser.downloadProgress" => self.download_progress(p, cx),
            "Target.targetCrashed" | "Inspector.targetCrashed" => self.crashed(event),
            "Inspector.targetReloadedAfterCrash" => self.reloaded_after_crash(event),
            "Target.targetCreated" => self.remember_opener(p),
            "Target.targetDestroyed" => self.closing(p),
            _ => self.log(event),
        }
    }

    pub(super) fn panel_focus(&self, cx: &mut Context<Self>) -> FocusHandle {
        self.notices.panel.get_or_init(|| cx.focus_handle()).clone()
    }

    pub(super) fn target_of(&self, event: &Event) -> Option<String> {
        let session = event.session.as_deref()?;
        self.tabs.by_session(session).map(|t| t.target.clone())
    }

    pub(super) fn active_log(&self) -> Option<&Log> {
        self.tabs.active_target(self.workspace).and_then(|t| self.notices.logs.get(&t))
    }

    fn log(&mut self, event: &Event) {
        let Some(tab) = event.session.as_deref().and_then(|s| self.tabs.by_session(s)) else { return };
        let (target, main) = (tab.target.clone(), tab.main_frame.clone().unwrap_or_else(|| tab.target.clone()));
        if event.method == "Page.frameNavigated" && event.params["frame"].get("parentId").is_none() {
            self.navigated_main(&target, event.session.as_deref());
        }
        self.notices.logs.entry(target).or_default().apply(&event.method, &event.params, &main);
    }

    fn remember_opener(&mut self, p: &Value) {
        let info = &p["targetInfo"];
        let (Some(target), Some(opener)) = (info["targetId"].as_str(), info["openerId"].as_str()) else { return };
        if self.tabs.active_target(self.workspace).as_deref() == Some(opener) {
            self.tabs.activate(self.workspace, target);
        }
        self.notices.openers.insert(target.to_string(), opener.to_string());
    }

    fn closing(&mut self, p: &Value) {
        let Some(target) = p["targetId"].as_str() else { return };
        self.back_to_opener(target);
        self.notices.logs.remove(target);
        self.notices.dialogs.remove(target);
        self.notices.crashed.remove(target);
        self.notices.revived.remove(target);
        self.notices.openers.retain(|popup, opener| popup != target && opener != target);
    }

    fn back_to_opener(&mut self, target: &str) {
        let Some(opener) = self.notices.openers.get(target).cloned() else { return };
        let Some(own) = self.tabs.get(target).map(|t| t.workspace) else { return };
        if self.tabs.get(&opener).is_none() {
            return;
        }
        for workspace in [self.workspace, own] {
            if self.tabs.active_target(workspace).as_deref() == Some(target) {
                self.tabs.activate(workspace, &opener);
            }
        }
    }

    fn crashed(&mut self, event: &Event) {
        let named = event.params["targetId"].as_str().map(str::to_string);
        let Some(target) = named.or_else(|| self.target_of(event)) else { return };
        if let Some(tab) = self.tabs.get_mut(&target) {
            tab.loading = false;
            if let Some(shot) = tab.shot.take() {
                self.stale.borrow_mut().push(shot.image);
            }
        }
        self.notices.crashed.insert(target);
    }

    fn reloaded_after_crash(&mut self, event: &Event) {
        let Some(target) = self.target_of(event) else { return };
        self.notices.crashed.remove(&target);
        self.notices.revived.insert(target);
    }

    fn navigated_main(&mut self, target: &str, session: Option<&str>) {
        self.notices.crashed.remove(target);
        if let (true, Some(session)) = (self.notices.revived.remove(target), session) {
            self.send(Some(session), "Emulation.clearDeviceMetricsOverride", json!({}));
            self.size_page(session);
            if self.casting.as_deref() == Some(target) {
                self.send(Some(session), "Page.stopScreencast", json!({}));
                self.start_cast(session);
            }
        }
    }

    fn reload_crashed(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.tabs.active_target(self.workspace) else { return };
        self.notices.crashed.remove(&target);
        self.reload(cx);
        cx.notify();
    }

    pub(super) fn page_cover(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let target = self.tabs.active_target(self.workspace)?;
        if self.notices.crashed.contains(&target) {
            return Some(crash_cover(cx));
        }
        let dialog = self.notices.dialogs.get(&target)?;
        Some(self.dialog_card(dialog, window, cx))
    }

    pub(super) fn tab_mark(&self, target: &str) -> Option<AnyElement> {
        if self.notices.crashed.contains(target) {
            return Some(icon(Icon::Failed, 11.).text_color(colours::failed()).into_any_element());
        }
        self.notices.dialogs.contains_key(target).then(|| prompts::dialog_mark().into_any_element())
    }
}

fn crash_cover(cx: &mut Context<Browser>) -> AnyElement {
    let reload = ui::icon_text_button("browser-crash-reload", Icon::Refresh, "Reload", ui::Tone::Primary)
        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.reload_crashed(cx)));
    div()
        .absolute()
        .inset_0()
        .bg(colours::pane())
        .child(
            notice("This page crashed")
                .child(ui::dim("Chrome stopped the page's process; reloading starts it again.").text_size(px(12.)))
                .child(reload),
        )
        .into_any_element()
}
