use std::rc::Rc;

use gpui::{
    AnyElement, ClickEvent, Context, Div, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement,
    SharedString, Stateful, StatefulInteractiveElement, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};

use crate::app::{Root, TITLEBAR_HEIGHT};
use crate::context_menu::{Entry, act, danger, item};
use crate::dialogs;
use crate::model::Tab;
use crate::sidebar::{Dragged, Target, cmd, draggable, order};
use crate::theme::colours;
use crate::ui::{self, badge, button_coloured as button};

fn faint(text: String) -> Div {
    div().text_size(px(11.)).text_color(colours::faint()).child(text)
}

fn tab_detail(tab: &Tab) -> Option<String> {
    let agent = tab.agent_pane()?;
    match (&agent.model, agent.context) {
        (Some(m), Some(c)) => Some(format!("{m} · {c}%")),
        (Some(m), None) => Some(m.clone()),
        _ => None,
    }
}

fn tab_drop_style(style: StyleRefinement, dragged: Target, id: u64, siblings: &[u64]) -> StyleRefinement {
    let Target::Tab(from) = dragged else { return style };
    if from == id || !siblings.contains(&from) {
        return style;
    }
    let style = style.border_color(colours::accent());
    if order::lands_after(siblings, from, id) { style.border_r_2() } else { style.border_l_2() }
}

fn close_button(tab: &Tab, group: SharedString, selected: bool, cx: &mut Context<Root>) -> impl IntoElement + use<> {
    let tab = tab.clone();
    div()
        .id(SharedString::from(format!("tab-close-{}", tab.id)))
        .px(px(4.))
        .rounded(px(4.))
        .text_size(px(11.))
        .text_color(colours::faint())
        .when(!selected, |d| d.invisible().group_hover(group, Styled::visible))
        .hover(|s| s.bg(colours::selected()).text_color(colours::failed()))
        .child("✕")
        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
            cx.stop_propagation();
            this.close_tab(&tab, window, cx);
        }))
}

fn new_tab_button(cx: &mut Context<Root>) -> impl IntoElement + use<> {
    button("new-tab", "+", colours::muted())
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, e: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                this.open_menu(e.position, new_tab_menu(), window, cx);
            }),
        )
        .drag_over::<Dragged>(
            |s, d, _, _| {
                if matches!(d.target, Target::Tab(_)) { s.border_color(colours::accent()) } else { s }
            },
        )
        .on_drop(cx.listener(|this, d: &Dragged, _, _| {
            if let Target::Tab(id) = d.target {
                this.queue.cmd(cmd(&["move", "--tab", &id.to_string(), "--last"]));
            }
        }))
}

fn split_buttons(cx: &mut Context<Root>) -> impl IntoElement + use<> {
    div()
        .flex()
        .gap(px(4.))
        .pb(px(5.))
        .child(
            button("split-right", "Split →", colours::muted())
                .on_click(cx.listener(|this, _: &ClickEvent, _, _| this.split(false))),
        )
        .child(
            button("split-down", "Split ↓", colours::muted())
                .on_click(cx.listener(|this, _: &ClickEvent, _, _| this.split(true))),
        )
}

fn new_tab_menu() -> Vec<Entry> {
    vec![
        act("New shell tab", cmd(&["new-tab", "--focus"])),
        act("Start the default agent", cmd(&["start", "--focus"])),
        item("Start an agent…", |_, window, cx| dialogs::start_agent(window, cx)),
    ]
}

fn agent_entries(tab: &Tab) -> Vec<Entry> {
    let Some(agent) = tab.agent_pane() else { return Vec::new() };
    let t = tab.id.to_string();
    let mark = if agent.status.as_deref() == Some("pending") {
        act("Mark as reviewed", cmd(&["mark", "--tab", &t, "--reviewed"]))
    } else {
        act("Mark as pending review", cmd(&["mark", "--tab", &t]))
    };
    let pane = agent.id;
    vec![
        Entry::Separator,
        mark,
        danger("Close agent…", move |_, window, cx| crate::sidebar::close_agent(pane, window, cx)),
    ]
}

impl Root {
    pub(crate) fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .h(px(TITLEBAR_HEIGHT))
            .flex_none()
            .flex()
            .items_end()
            .justify_between()
            .px(px(8.))
            .bg(colours::sidebar())
            .border_b_1()
            .border_color(colours::border())
            .child(
                div()
                    .flex()
                    .items_end()
                    .h_full()
                    .gap(px(4.))
                    .overflow_hidden()
                    .child(self.tab_strip(cx))
                    .child(div().pb(px(5.)).child(new_tab_button(cx))),
            )
            .child(split_buttons(cx))
    }

    fn tab_strip(&self, cx: &mut Context<Self>) -> Div {
        let strip = div().flex().items_end().gap(px(2.)).h_full().overflow_hidden();
        let Some((_, ws)) = self.shown_workspace() else { return strip };
        let shown_tab = self.session.as_ref().and_then(|s| s.shown().tab);
        let siblings: Rc<Vec<u64>> = Rc::new(ws.tabs.iter().map(|t| t.id).collect());
        strip.children(ws.tabs.iter().map(|tab| self.tab(tab, Some(tab.id) == shown_tab, &siblings, cx)))
    }

    fn tab(&self, tab: &Tab, selected: bool, siblings: &Rc<Vec<u64>>, cx: &mut Context<Self>) -> Stateful<Div> {
        let id = tab.id;
        let el = self
            .tab_face(tab, selected, cx)
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    this.open_target_menu(Target::Tab(id), e.position, window, cx);
                }),
            )
            .on_click(
                cx.listener(move |this, e: &ClickEvent, window, cx| this.click_tab(id, e.click_count(), window, cx)),
            );
        let siblings = siblings.clone();
        draggable(el, Target::Tab(id), tab.name.clone())
            .drag_over::<Dragged>(move |s, d, _, _| tab_drop_style(s, d.target, id, &siblings))
            .on_drop(cx.listener(move |this, d: &Dragged, _, _| this.drop_on(d.target, Target::Tab(id))))
    }

    fn tab_face(&self, tab: &Tab, selected: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let group = SharedString::from(format!("tab-{}", tab.id));
        let others = tab.panes.len().saturating_sub(1);
        div()
            .id(group.clone())
            .group(group.clone())
            .flex()
            .items_center()
            .gap(px(6.))
            .h(px(30.))
            .px(px(10.))
            .rounded_t(px(6.))
            .border_l_2()
            .border_r_2()
            .border_color(gpui::transparent_black())
            .cursor_pointer()
            .when(selected, |d| d.bg(colours::terminal()).text_color(colours::text()))
            .when(!selected, |d| d.text_color(colours::muted()).hover(|s| s.bg(colours::hover())))
            .child(badge(tab.loudest()))
            .child(self.tab_name(tab))
            .when(others > 0, |d| d.child(faint(format!("+{others}"))))
            .when_some(tab_detail(tab), |d, text| d.child(faint(text).whitespace_nowrap()))
            .child(close_button(tab, group, selected, cx))
    }

    fn tab_name(&self, tab: &Tab) -> AnyElement {
        match self.side.rename.as_ref().filter(|r| r.target == Target::Tab(tab.id)) {
            Some(r) => div().w(px(140.)).child(ui::input_box(r.field.clone(), true).py(px(0.))).into_any_element(),
            None => div().whitespace_nowrap().child(tab.name.clone()).into_any_element(),
        }
    }

    fn click_tab(&mut self, id: u64, clicks: usize, window: &mut Window, cx: &mut Context<Self>) {
        if clicks == 2 {
            self.start_rename(Target::Tab(id), window, cx);
        } else {
            self.focus_item("tab", id);
        }
    }

    pub(crate) fn tab_menu(&self, id: u64) -> Vec<Entry> {
        let Some(tab) = self.session.as_ref().and_then(|s| s.tab(id)).cloned() else { return Vec::new() };
        let mut entries = vec![
            item("Rename tab", move |this, window, cx| this.start_rename(Target::Tab(id), window, cx)),
            act("New shell tab", cmd(&["new-tab", "--focus"])),
        ];
        entries.extend(agent_entries(&tab));
        entries.push(Entry::Separator);
        entries.push(danger("Close tab", move |this, window, cx| this.close_tab(&tab, window, cx)));
        entries
    }

    pub(crate) fn close_tab(&mut self, tab: &Tab, window: &mut Window, cx: &mut Context<Self>) {
        let command = cmd(&["close", "--tab", &tab.id.to_string()]);
        let Some(agent) = tab.agent_pane() else {
            self.queue.cmd(command);
            return;
        };
        let what = agent.agent.clone().unwrap_or_else(|| "an agent".into());
        let state = agent.status.as_deref().map(|s| format!(" ({s})")).unwrap_or_default();
        dialogs::confirm(
            dialogs::Question::new(
                format!("Close the tab {}?", tab.name),
                format!("It runs {what}{state}: closing the tab stops it. Its worktree and branch stay."),
                "Close tab",
                command,
            ),
            window,
            cx,
        );
    }
}
