use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, FocusHandle, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, SharedString, Stateful, StatefulInteractiveElement, StyleRefinement, Styled, Window,
    div, prelude::FluentBuilder as _, px,
};

use crate::app::frame::{self, Frame};
use crate::app::{Root, TITLEBAR_HEIGHT};
use crate::context_menu::{Entry, act, danger, item};
use crate::dialogs;
use crate::keymap::{self, Command, with_keys};
use crate::model::{MachinePlace, MachineSeen, Tab, ThereId, ThereWorkspace, memory_size};
use crate::sidebar::{Dragged, Look, Renaming, Target, cmd, draggable, order, tip};
use crate::store::{Busy, ToastKind};
use crate::terminal::cycled;
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, badge};

mod strip;

pub(crate) use strip::TabStrip;

const TAB_HEIGHT: f32 = 28.;

fn faint(text: String) -> Div {
    div().text_size(px(11.)).text_color(colours::faint()).child(text)
}

fn tab_detail<Id>(tab: &Tab<Id>, mark: Option<&str>, look: &Look) -> Option<String> {
    let agent = tab.agent_pane();
    let share = agent.filter(|a| a.model.is_some() || a.context.is_none()).and_then(|a| look.share_of(a));
    let model = agent.and_then(|agent| look.model_of(agent));
    let memory = tab.memory().map(memory_size);
    let parts: Vec<String> = model.into_iter().chain(mark.map(str::to_string)).chain(share).chain(memory).collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
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
        .size(px(16.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::CHIP))
        .text_size(px(10.))
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
    ui::tool_button("new-tab", Icon::Plus, TAB_HEIGHT, false)
        .tooltip(tip(with_keys("New tab", Command::NewTab)))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, e: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                this.open_menu(e.position, new_tab_menu(), window, cx);
            }),
        )
        .drag_over::<Dragged>(
            |s, d, _, _| {
                if matches!(d.target, Target::Tab(_)) { s.bg(colours::accent_soft()) } else { s }
            },
        )
        .on_drop(cx.listener(|this, d: &Dragged, _, _| {
            if let Target::Tab(id) = d.target {
                this.own.queue.cmd(cmd(&["move", "--tab", &id.to_string(), "--last"]));
            }
        }))
}

fn split_buttons(cx: &mut Context<Root>) -> Div {
    div()
        .flex()
        .flex_none()
        .gap(px(2.))
        .child(
            ui::tool_button("split-right", Icon::SplitRight, TAB_HEIGHT, false)
                .tooltip(tip(with_keys("Split right", Command::SplitRight)))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.split(false, cx))),
        )
        .child(
            ui::tool_button("split-down", Icon::SplitDown, TAB_HEIGHT, false)
                .tooltip(tip(with_keys("Split down", Command::SplitDown)))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.split(true, cx))),
        )
}

fn tools(window_frame: Frame, terminal: &FocusHandle, cx: &mut Context<Root>) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .child(frame::claimed(split_buttons(cx), window_frame, terminal))
        .children(frame::window_controls(window_frame, terminal))
}

fn new_tab_menu() -> Vec<Entry> {
    vec![
        act("New shell tab", cmd(&["new-tab", "--focus"])),
        act("Start the default agent", cmd(&["start", "--focus"])),
        item("Start an agent…", |_, window, cx| dialogs::start_agent(window, cx)),
    ]
}

fn tab_pill(group: SharedString, selected: bool) -> Stateful<Div> {
    div()
        .id(group.clone())
        .group(group)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .h(px(TAB_HEIGHT))
        .pl(px(10.))
        .pr(px(6.))
        .relative()
        .rounded(px(radius::CONTROL))
        .border_1()
        .border_color(gpui::transparent_black())
        .cursor_pointer()
        .when(selected, |d| d.bg(colours::selected()).border_color(colours::border()).text_color(colours::text()))
        .when(!selected, |d| {
            d.text_color(colours::muted()).hover(|s| s.bg(colours::hover()).text_color(colours::soft()))
        })
}

fn machine_tab(
    (seen, ws): (&MachineSeen, &ThereWorkspace),
    tab: &Tab<ThereId>,
    selected: bool,
    look: &Look,
    cx: &mut Context<Root>,
) -> Stateful<Div> {
    let place = MachinePlace::at(seen, ws, Some(tab.id));
    let others = tab.panes.len().saturating_sub(1);
    let detail = tab_detail(tab, None, look);
    let status =
        tab.loudest().map(|status| if seen.linked() { badge(Some(status)) } else { ui::still_badge(Some(status)) });
    tab_pill(SharedString::from(format!("machine-tab-{}", tab.id.key())), selected)
        .pr(px(10.))
        .children(status)
        .child(div().whitespace_nowrap().child(tab.name.clone()))
        .when(others > 0, |d| d.child(faint(format!("+{others}"))))
        .when_some(detail, |d, text| d.child(faint(text).whitespace_nowrap()))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.pick_on_machine(place.clone(), cx)))
}

fn agent_entries(tab: &Tab, switch: Option<Entry>) -> Vec<Entry> {
    let Some(agent) = tab.agent_pane() else { return Vec::new() };
    let t = tab.id.to_string();
    let mark = if agent.status.as_deref() == Some("pending") {
        act("Mark as reviewed", cmd(&["mark", "--tab", &t, "--reviewed"]))
    } else {
        act("Mark as pending review", cmd(&["mark", "--tab", &t]))
    };
    let pane = agent.id;
    let mut entries = vec![Entry::Separator, mark];
    entries.extend(switch);
    entries.extend(
        dialogs::context_choices(tab)
            .into_iter()
            .map(|(pane, label)| item(label, move |_, window, cx| dialogs::view_context(pane, window, cx))),
    );
    entries.push(danger("Close agent…", move |_, window, cx| crate::sidebar::close_agent(pane, window, cx)));
    entries
}

impl Root {
    pub(crate) fn render_tabs(&self, window_frame: Frame, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let row = div()
            .h(px(TITLEBAR_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .pr(px(window_frame.title_end()))
            .child(frame::claimed(self.tab_group(cx), window_frame, &self.focus))
            .child(tools(window_frame, &self.focus, cx));
        frame::title_row(row, window_frame)
    }

    fn tab_group(&self, cx: &mut Context<Self>) -> Div {
        let group = div().flex().items_center().min_w_0().gap(px(6.)).overflow_hidden();
        if self.side.look.current.tabs_in_sidebar {
            return group;
        }
        group
            .child(self.scrolled_tabs(cx))
            .when(self.machine.is_none(), |d| d.child(new_tab_button(cx)))
            .child(strip::watcher(&self.side.strip, cx.entity().downgrade()))
    }

    fn scrolled_tabs(&self, cx: &mut Context<Self>) -> Div {
        let strip = &self.side.strip;
        let arrows = strip.arrows();
        div()
            .flex()
            .items_center()
            .min_w_0()
            .gap(px(strip::ARROW_GAP))
            .when(arrows, |d| d.child(strip::arrow(false, strip.can_step(false), cx)))
            .child(self.tab_strip(cx))
            .when(arrows, |d| d.child(strip::arrow(true, strip.can_step(true), cx)))
    }

    fn tab_strip(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let strip = div()
            .id("tab-strip")
            .flex()
            .items_center()
            .min_w_0()
            .gap(px(4.))
            .overflow_x_scroll()
            .track_scroll(self.side.strip.handle());
        if self.machine.is_some() {
            return strip.children(self.machine_tabs(cx));
        }
        let Some((_, ws)) = self.shown_workspace() else { return strip };
        let shown_tab = self.session.as_ref().and_then(|s| s.shown().tab);
        let index = ws.tabs.iter().position(|t| Some(t.id) == shown_tab);
        self.side.strip.show(Some(ws.id), shown_tab, index);
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
        let mark = tab.agent_pane().and_then(|agent| self.account_mark(agent, cx));
        let detail = tab_detail(tab, mark.as_deref(), &self.side.look.current);
        tab_pill(group.clone(), selected)
            .children(tab.loudest().map(|status| badge(Some(status))))
            .child(self.tab_name(tab))
            .when(others > 0, |d| d.child(faint(format!("+{others}"))))
            .when_some(detail, |d, text| d.child(faint(text).whitespace_nowrap()))
            .child(close_button(tab, group, selected, cx))
    }

    fn tab_name(&self, tab: &Tab) -> AnyElement {
        match self.side.rename.as_ref().filter(|r| r.what == Renaming::Here(Target::Tab(tab.id))) {
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

    fn machine_workspace(&self) -> Option<(&MachineSeen, &ThereWorkspace)> {
        let place = self.side.machine_place.as_ref()?;
        let seen = self.session.as_ref()?.machine(&place.machine)?;
        Some((seen, seen.workspace(place.workspace)?))
    }

    fn machine_tabs(&self, cx: &mut Context<Self>) -> Vec<Stateful<Div>> {
        let Some((seen, ws)) = self.machine_workspace() else { return Vec::new() };
        let shown = self.side.machine_place.as_ref().and_then(|place| place.tab);
        let index = ws.tabs.iter().position(|tab| Some(tab.id) == shown);
        self.side.strip.show(None, index.and_then(|at| u64::try_from(at).ok()), index);
        let look = &self.side.look.current;
        ws.tabs.iter().map(|tab| machine_tab((seen, ws), tab, Some(tab.id) == shown, look, cx)).collect()
    }

    fn pick_on_machine(&mut self, place: MachinePlace, cx: &mut Context<Self>) {
        self.store.update(cx, |s, cx| s.pick_machine_place(place, cx));
    }

    fn machine_tab_place(
        &self,
        tab: impl FnOnce(&ThereWorkspace, Option<ThereId>) -> Option<ThereId>,
    ) -> Option<MachinePlace> {
        let (seen, ws) = self.machine_workspace()?;
        let shown = self.side.machine_place.as_ref().and_then(|place| place.tab);
        Some(MachinePlace::at(seen, ws, Some(tab(ws, shown)?)))
    }

    pub(crate) fn pick_machine_tab_number(&mut self, key: &str, cx: &mut Context<Self>) {
        let number =
            |ws: &ThereWorkspace, _| keymap::tab_index(key, ws.tabs.len()).and_then(|i| ws.tabs.get(i)).map(|t| t.id);
        if let Some(place) = self.machine_tab_place(number) {
            self.pick_on_machine(place, cx);
        }
    }

    pub(crate) fn cycle_machine_tab(&mut self, by: i32, cx: &mut Context<Self>) {
        let next = |ws: &ThereWorkspace, shown| cycled(&ws.tabs.iter().map(|t| t.id).collect::<Vec<_>>(), shown, by);
        if let Some(place) = self.machine_tab_place(next) {
            self.pick_on_machine(place, cx);
        }
    }

    pub(crate) fn not_offered_there(&mut self, what: &str, cx: &mut Context<Self>) {
        let Some(machine) = self.machine.as_ref().map(|tab| tab.machine.clone()) else { return };
        let text = format!("{what} on {machine} is not offered here yet");
        self.store.update(cx, |s, cx| s.toast(text, ToastKind::Info, cx));
    }

    pub(crate) fn tab_menu(&self, id: u64, cx: &App) -> Vec<Entry> {
        let Some(tab) = self.session.as_ref().and_then(|s| s.tab(id)).cloned() else { return Vec::new() };
        let mut entries = vec![
            item("Rename tab", move |this, window, cx| this.start_rename(Target::Tab(id), window, cx)),
            act("New shell tab", cmd(&["new-tab", "--focus"])),
        ];
        let switch = tab.agent_pane().and_then(|agent| self.switch_account_item(agent.id, cx));
        entries.extend(agent_entries(&tab, switch));
        entries.push(Entry::Separator);
        entries.push(danger("Close tab", move |this, window, cx| this.close_tab(&tab, window, cx)));
        entries
    }

    pub(crate) fn close_tab(&mut self, tab: &Tab, window: &mut Window, cx: &mut Context<Self>) {
        let command = cmd(&["close", "--tab", &tab.id.to_string()]);
        let Some(agent) = tab.agent_pane() else {
            self.own.queue.cmd(command);
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
            )
            .busy(Busy::new(format!("close-tab:{}", tab.id), format!("Closing {}…", tab.name))),
            window,
            cx,
        );
    }
}

#[cfg(test)]
mod tests;
