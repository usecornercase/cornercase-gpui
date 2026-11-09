use gpui::{
    App, AppContext as _, ClickEvent, Context, Div, FocusHandle, Focusable, IntoElement, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use serde_json::Value;

use super::{Check, Checked, bullet, buttons, keyed, line, load_check, show, warning};
use crate::dialogs::frame;
use crate::store::{Busy, Store, args};
use crate::theme::colours;
use crate::ui;

pub(in crate::sidebar) struct CloseAgent {
    pane: u64,
    check: Check,
    focus: FocusHandle,
}

pub(crate) fn close_agent(pane: u64, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| {
        load_check(args(&["close", "--agent", "--pane", &pane.to_string(), "--check", "--json"]), cx);
        CloseAgent { pane, check: Check::Loading, focus: cx.focus_handle() }
    });
    show(view, window, cx);
}

impl CloseAgent {
    fn run(&mut self, cx: &mut Context<Self>) {
        let Check::Ready(v) = &self.check else { return };
        let agent = v.get("agent").and_then(Value::as_str).unwrap_or("the agent");
        let busy = Busy::new(format!("close-agent:{}", self.pane), format!("Closing {agent}…"));
        let command = args(&["close", "--agent", "--pane", &self.pane.to_string()]);
        Store::get(cx).update(cx, |s, cx| {
            s.run_aside(busy, command, cx);
            s.close_dialog(cx);
        });
    }
}

fn pane_label(p: &Value, cx: &App) -> String {
    let agent = p.get("agent").and_then(Value::as_str).unwrap_or("shell");
    let tab = p.get("tab").and_then(Value::as_u64);
    let store = Store::get(cx);
    let name = store.read(cx).session.as_ref().and_then(|s| {
        s.projects
            .iter()
            .flat_map(|p| &p.workspaces)
            .flat_map(|w| &w.tabs)
            .find(|t| Some(t.id) == tab)
            .map(|t| t.name.clone())
    });
    match name {
        Some(name) => format!("{agent} in “{name}”"),
        None => agent.to_string(),
    }
}

fn pane_list(v: &Value, key: &str, title: &str, colour: gpui::Rgba, cx: &App) -> Vec<Div> {
    let panes: Vec<String> =
        v.get(key).and_then(Value::as_array).into_iter().flatten().map(|p| pane_label(p, cx)).collect();
    if panes.is_empty() {
        return Vec::new();
    }
    std::iter::once(line(title.to_string())).chain(panes.iter().map(|p| bullet(p, colour))).collect()
}

fn close_report(v: &Value, cx: &App) -> (String, Div) {
    let agent = v.get("agent").and_then(Value::as_str).unwrap_or("agent");
    let tabs = v.get("tabs").and_then(Value::as_array).map_or(0, Vec::len);
    let body = div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(line(format!(
            "Closes {tabs} tab{}. Nothing is removed from disk: its worktree and branch stay.",
            if tabs == 1 { "" } else { "s" }
        )))
        .children(pane_list(v, "stops", "Stops:", colours::text(), cx))
        .children(pane_list(v, "keeps", "Keeps running elsewhere:", colours::muted(), cx))
        .children(v.get("warning").and_then(Value::as_str).map(|w| warning(w.to_string())));
    (format!("Close {agent}?"), body)
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
        let (title, body) = match &self.check {
            Check::Loading => {
                ("Close agent?".to_string(), div().child(ui::busy("checking", "Checking what would stop…")))
            }
            Check::Failed(e) => ("Close agent?".to_string(), div().child(warning(e.clone()))),
            Check::Ready(v) => close_report(v, cx),
        };
        keyed(&self.focus, Self::run, cx).child(frame(
            title,
            body,
            buttons([ui::danger_button("close-agent", "Close agent")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.run(cx)))]),
        ))
    }
}
