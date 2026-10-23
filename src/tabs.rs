//! The tabs of the shown workspace, above the terminal, with the split buttons. A tab renames in
//! place (double-click), drags to reorder (`cornercase move --tab`), opens its menu with a
//! right-click, and asks before closing when it runs an agent. `+` offers a shell or an agent.

use std::rc::Rc;

use gpui::{
    ClickEvent, Context, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder as _, px,
};

use crate::app::{Root, TITLEBAR_HEIGHT};
use crate::context_menu::{Entry, act, danger, item};
use crate::dialogs;
use crate::model::Tab;
use crate::sidebar::{Dragged, Target, cmd, draggable, order};
use crate::theme::colours;
use crate::ui::{self, badge, button_coloured as button};

impl Root {
    pub(crate) fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let shown_tab = self.session.as_ref().and_then(|s| s.shown().tab);
        let mut tabs = div().flex().items_end().gap(px(2.)).h_full().overflow_hidden();
        if let Some((_, ws)) = self.shown_workspace() {
            let siblings: Rc<Vec<u64>> = Rc::new(ws.tabs.iter().map(|t| t.id).collect());
            for tab in &ws.tabs {
                let id = tab.id;
                let selected = Some(id) == shown_tab;
                let agent = tab.agent_pane();
                let detail = agent.and_then(|a| match (&a.model, a.context) {
                    (Some(m), Some(c)) => Some(format!("{m} · {c}%")),
                    (Some(m), None) => Some(m.clone()),
                    _ => None,
                });
                let others = tab.panes.len().saturating_sub(1);
                let gid = SharedString::from(format!("tab-{id}"));
                let tab_clone = tab.clone();
                let name = match self.side.rename.as_ref().filter(|r| r.target == Target::Tab(id)) {
                    Some(r) => {
                        div().w(px(140.)).child(ui::input_box(r.field.clone(), true).py(px(0.))).into_any_element()
                    }
                    None => div().whitespace_nowrap().child(tab.name.clone()).into_any_element(),
                };
                let el = div()
                    .id(gid.clone())
                    .group(gid.clone())
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
                    .child(name)
                    .when(others > 0, |d| {
                        d.child(div().text_size(px(11.)).text_color(colours::faint()).child(format!("+{others}")))
                    })
                    .when_some(detail, |d, text| {
                        d.child(div().text_size(px(11.)).text_color(colours::faint()).whitespace_nowrap().child(text))
                    })
                    .child(
                        div()
                            .id(SharedString::from(format!("tab-close-{id}")))
                            .px(px(4.))
                            .rounded(px(4.))
                            .text_size(px(11.))
                            .text_color(colours::faint())
                            .when(!selected, |d| d.invisible().group_hover(gid.clone(), Styled::visible))
                            .hover(|s| s.bg(colours::selected()).text_color(colours::failed()))
                            .child("✕")
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                cx.stop_propagation();
                                this.close_tab(&tab_clone, window, cx);
                            })),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                            this.open_target_menu(Target::Tab(id), e.position, window, cx);
                        }),
                    )
                    .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                        if e.click_count() == 2 {
                            this.start_rename(Target::Tab(id), window, cx);
                        } else {
                            this.focus_item("tab", id);
                        }
                    }));
                let el = draggable(el, Target::Tab(id), tab.name.clone());
                let siblings = siblings.clone();
                let el = el
                    .drag_over::<Dragged>(move |s, d, _, _| match d.target {
                        Target::Tab(from) if from != id && siblings.contains(&from) => {
                            let s = s.border_color(colours::accent());
                            if order::lands_after(&siblings, from, id) { s.border_r_2() } else { s.border_l_2() }
                        }
                        _ => s,
                    })
                    .on_drop(cx.listener(move |this, d: &Dragged, _, _| this.drop_on(d.target, Target::Tab(id))));
                tabs = tabs.child(el);
            }
        }
        let new_tab = button("new-tab", "+", colours::muted())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    let entries = this.new_tab_menu();
                    this.open_menu(e.position, entries, window, cx);
                }),
            )
            // A tab dropped here goes last.
            .drag_over::<Dragged>(|s, d, _, _| {
                if matches!(d.target, Target::Tab(_)) { s.border_color(colours::accent()) } else { s }
            })
            .on_drop(cx.listener(|this, d: &Dragged, _, _| {
                if let Target::Tab(id) = d.target {
                    this.queue.cmd(cmd(&["move", "--tab", &id.to_string(), "--last"]));
                }
            }));
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
                    .child(tabs)
                    .child(div().pb(px(5.)).child(new_tab)),
            )
            .child(
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
                    ),
            )
    }

    /// `+`: a shell, the default agent, or the start dialog.
    fn new_tab_menu(&self) -> Vec<Entry> {
        vec![
            act("New shell tab", cmd(&["new-tab", "--focus"])),
            act("Start the default agent", cmd(&["start", "--focus"])),
            item("Start an agent…", |_, window, cx| dialogs::start_agent(window, cx)),
        ]
    }

    /// A tab's menu, from a right-click on it or on its row.
    pub(crate) fn tab_menu(&self, id: u64) -> Vec<Entry> {
        let tab = self
            .session
            .as_ref()
            .and_then(|s| s.projects.iter().flat_map(|p| &p.workspaces).flat_map(|w| &w.tabs).find(|t| t.id == id))
            .cloned();
        let Some(tab) = tab else { return Vec::new() };
        let t = id.to_string();
        let mut entries = vec![
            item("Rename tab", move |this, window, cx| this.start_rename(Target::Tab(id), window, cx)),
            act("New shell tab", cmd(&["new-tab", "--focus"])),
        ];
        if let Some(agent) = tab.agent_pane() {
            entries.push(Entry::Separator);
            if agent.status.as_deref() == Some("pending") {
                entries.push(act("Mark as reviewed", cmd(&["mark", "--tab", &t, "--reviewed"])));
            } else {
                entries.push(act("Mark as pending review", cmd(&["mark", "--tab", &t])));
            }
            let pane = agent.id;
            entries.push(danger("Close agent…", move |_, window, cx| crate::sidebar::close_agent(pane, window, cx)));
        }
        entries.push(Entry::Separator);
        entries.push(danger("Close tab", move |this, window, cx| this.close_tab(&tab, window, cx)));
        entries
    }

    /// Closes a tab; one running an agent asks first, as closing it stops the agent.
    pub(crate) fn close_tab(&mut self, tab: &Tab, window: &mut Window, cx: &mut Context<Self>) {
        let command = cmd(&["close", "--tab", &tab.id.to_string()]);
        match tab.agent_pane() {
            None => self.queue.cmd(command),
            Some(agent) => {
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
    }
}
