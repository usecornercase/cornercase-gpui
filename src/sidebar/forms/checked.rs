use std::rc::Rc;

use gpui::{
    App, AppContext as _, ClickEvent, Context, Div, FocusHandle, Focusable, IntoElement, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, div, px,
};
use serde_json::Value;

use super::{Check, Checked, bullet, buttons, keyed, line, load_check, show, warning};
use crate::dialogs::{agent_doing, agent_title, closes_line, frame};
use crate::store::{Busy, Store, args};
use crate::theme::colours;
use crate::ui;

pub(crate) type CloseIt = Rc<dyn Fn(&mut App)>;

pub(crate) struct AgentCloseThere {
    pub(crate) machine: String,
    pub(crate) tabs: Vec<(u64, String)>,
    pub(crate) close: CloseIt,
}

pub(crate) struct CloseAgent {
    pane: u64,
    check: Check,
    focus: FocusHandle,
    there: Option<AgentCloseThere>,
}

pub(crate) fn close_agent(pane: u64, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| {
        load_check(args(&["close", "--agent", "--pane", &pane.to_string(), "--check", "--json"]), cx);
        CloseAgent { pane, check: Check::Loading, focus: cx.focus_handle(), there: None }
    });
    show(view, window, cx);
}

pub(crate) fn close_agent_there(
    pane: u64,
    there: AgentCloseThere,
    window: &mut Window,
    cx: &mut App,
) -> WeakEntity<CloseAgent> {
    let view = cx.new(|cx| CloseAgent { pane, check: Check::Loading, focus: cx.focus_handle(), there: Some(there) });
    let weak = view.downgrade();
    show(view, window, cx);
    weak
}

pub(crate) fn agent_checked(dialog: &WeakEntity<CloseAgent>, result: anyhow::Result<String>, cx: &mut App) {
    let value = result.and_then(|out| serde_json::from_str::<Value>(&out).map_err(anyhow::Error::from));
    let _ = dialog.update(cx, |this, cx| {
        this.check = Check::from(value);
        cx.notify();
    });
}

fn pane_label(p: &Value, tab_name: Option<String>) -> String {
    let agent = p.get("agent").and_then(Value::as_str).unwrap_or("shell");
    match tab_name {
        Some(name) => format!("{agent} in “{name}”"),
        None => agent.to_string(),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Report {
    title: String,
    closes: String,
    stops: Vec<String>,
    keeps: Vec<String>,
    warning: Option<String>,
}

fn labels(v: &Value, key: &str, name: &impl Fn(u64) -> Option<String>) -> Vec<String> {
    let panes = v.get(key).and_then(Value::as_array).into_iter().flatten();
    panes.map(|p| pane_label(p, p.get("tab").and_then(Value::as_u64).and_then(name))).collect()
}

fn report(v: &Value, machine: Option<&str>, name: impl Fn(u64) -> Option<String>) -> Report {
    let tabs = v.get("tabs").and_then(Value::as_array).map_or(0, Vec::len);
    Report {
        title: agent_title(Some(v.get("agent").and_then(Value::as_str).unwrap_or("agent")), machine),
        closes: closes_line(tabs, machine),
        stops: labels(v, "stops", &name),
        keeps: labels(v, "keeps", &name),
        warning: v.get("warning").and_then(Value::as_str).map(str::to_string),
    }
}

fn list(title: &str, items: &[String], colour: gpui::Rgba) -> Vec<Div> {
    if items.is_empty() {
        return Vec::new();
    }
    std::iter::once(line(title.to_string())).chain(items.iter().map(|p| bullet(p, colour))).collect()
}

fn report_body(report: Report) -> (String, Div) {
    let body = div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(line(report.closes))
        .children(list("Stops:", &report.stops, colours::text()))
        .children(list("Keeps running elsewhere:", &report.keeps, colours::muted()))
        .children(report.warning.map(warning));
    (report.title, body)
}

impl CloseAgent {
    fn machine(&self) -> Option<&str> {
        self.there.as_ref().map(|there| there.machine.as_str())
    }

    fn tab_name(&self, tab: u64, cx: &App) -> Option<String> {
        if let Some(there) = &self.there {
            return there.tabs.iter().find(|(id, _)| *id == tab).map(|(_, name)| name.clone());
        }
        let store = Store::get(cx);
        let session = store.read(cx).session.as_ref()?;
        session.tabs().find(|t| t.id == tab).map(|t| t.name.clone())
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        let Check::Ready(v) = &self.check else { return };
        if let Some(there) = &self.there {
            let close = there.close.clone();
            Store::get(cx).update(cx, Store::close_dialog);
            close(cx);
            return;
        }
        let agent = v.get("agent").and_then(Value::as_str).unwrap_or("the agent");
        let busy = Busy::new(format!("close-agent:{}", self.pane), agent_doing(agent, None));
        let command = args(&["close", "--agent", "--pane", &self.pane.to_string()]);
        Store::get(cx).update(cx, |s, cx| {
            s.run_aside(busy, command, cx);
            s.close_dialog(cx);
        });
    }
}

impl Checked for CloseAgent {
    fn check(&mut self) -> &mut Check {
        &mut self.check
    }
}

impl Focusable for CloseAgent {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for CloseAgent {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let waiting = agent_title(None, self.machine());
        let (title, body) = match &self.check {
            Check::Loading => (waiting, div().child(ui::busy("checking", "Checking what would stop…"))),
            Check::Failed(e) => (waiting, div().child(warning(e.clone()))),
            Check::Ready(v) => report_body(report(v, self.machine(), |tab| self.tab_name(tab, cx))),
        };
        keyed(&self.focus, Self::run, cx).child(frame(
            title,
            body,
            buttons([ui::danger_button("close-agent", "Close agent")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.run(cx)))]),
        ))
    }
}

#[cfg(test)]
mod tests;
