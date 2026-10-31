use std::time::{Duration, Instant};

use gpui::{
    AnyElement, ClickEvent, ClipboardItem, Context, Corner, Div, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, ParentElement as _, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, anchored,
    deferred, div, point, prelude::FluentBuilder as _, px,
};

use super::Browser;
use super::agent::{Menu, driving_note, toast};
use super::shots::Shot;
use crate::dialogs;
use crate::notify::tip;
use crate::store::ToastKind;
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, Tone, glass};

const BUTTON: f32 = 26.;
const REOPEN: Duration = Duration::from_millis(400);

fn popover(card: impl IntoElement, below: bool) -> Div {
    let (spot, corner, lift) = if below {
        (div().absolute().top_full().right_0(), Corner::TopRight, 6.)
    } else {
        (div().absolute().top_0().left_0(), Corner::BottomLeft, -6.)
    };
    let placed = anchored().anchor(corner).offset(point(px(0.), px(lift))).snap_to_window_with_margin(px(8.));
    spot.child(deferred(placed.child(card)).with_priority(1))
}

fn entry(id: impl Into<gpui::ElementId>, glyph: Icon, label: impl Into<SharedString>) -> Stateful<Div> {
    ui::row(id, false)
        .text_size(px(13.))
        .text_color(colours::soft())
        .child(ui::icon(glyph, 14.).text_color(colours::muted()))
        .child(ui::one_line(label))
}

fn menu_card(id: &'static str, width: f32, cx: &mut Context<Browser>) -> Stateful<Div> {
    glass::card(radius::CARD)
        .id(id)
        .occlude()
        .w(px(width))
        .p(px(4.))
        .flex()
        .flex_col()
        .gap(px(2.))
        .on_mouse_down(MouseButton::Left, |_: &MouseDownEvent, _, cx| cx.stop_propagation())
        .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| this.close_menu(cx)))
}

impl Browser {
    pub(super) fn toggle_menu(&mut self, menu: Menu, cx: &mut Context<Self>) {
        let just_closed = self.agents.closed.take().is_some_and(|(m, at)| m == menu && at.elapsed() < REOPEN);
        if !just_closed {
            self.agents.menu = if self.agents.menu == Some(menu) { None } else { Some(menu) };
            cx.notify();
        }
    }

    fn close_menu(&mut self, cx: &mut Context<Self>) {
        if let Some(menu) = self.agents.menu.take() {
            self.agents.closed = Some((menu, Instant::now()));
            cx.notify();
        }
    }

    pub(super) fn pick_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let enabled = self.active_session().is_some();
        ui::tool_button("browser-pick", Icon::Pick, BUTTON, self.picking())
            .when(!enabled, |b| b.opacity(0.35))
            .tooltip(tip("Pick an element to hand to the agent"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_pick(cx)))
    }

    pub(super) fn shots_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let open = self.agents.menu == Some(Menu::Shots);
        let button = if self.agents.shooting.is_some() {
            div().size(px(BUTTON)).flex().items_center().justify_center().child(ui::spinner(
                "browser-shooting",
                14.,
                colours::accent(),
            ))
        } else {
            div().child(
                ui::tool_button("browser-shot", Icon::Camera, BUTTON, open)
                    .when(self.active_session().is_none(), |b| b.opacity(0.35))
                    .tooltip(tip("Screenshot for the agent"))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_menu(Menu::Shots, cx))),
            )
        };
        let menu = (open && self.agents.shooting.is_none()).then(|| popover(shots_menu(cx), true));
        div().relative().flex_none().child(button).children(menu)
    }

    pub(super) fn agents_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let open = self.agents.menu == Some(Menu::Agents);
        let button = ui::tool_button("browser-agents", Icon::Agent, BUTTON, open)
            .tooltip(tip("Agents and this browser"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_menu(Menu::Agents, cx)));
        let menu = open.then(|| popover(self.agents_menu(cx), true));
        div().relative().flex_none().child(button).children(menu)
    }

    fn agents_menu(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let endpoint = self.agent_endpoint();
        menu_card("browser-agents-menu", 320., cx)
            .child(ui::section("Agents and this browser").px(px(10.)).pt(px(6.)))
            .child(
                div()
                    .px(px(10.))
                    .text_size(px(12.))
                    .text_color(colours::muted())
                    .child("Agents can drive this Chrome over the DevTools protocol, and you see their tabs here."),
            )
            .child(endpoint_row(endpoint.clone(), cx))
            .child(self.tell_row(endpoint.is_some(), cx))
            .children(self.agent_rows(cx))
            .child(div().mx(px(6.)).my(px(4.)).h(px(1.)).bg(colours::border()))
            .child(self.toggles(cx))
    }

    fn tell_row(&self, ready: bool, cx: &mut Context<Self>) -> AnyElement {
        let Some(pane) = self.target_agent(cx) else {
            return div()
                .flex()
                .items_center()
                .gap(px(8.))
                .p(px(6.))
                .child(div().flex_1().child(ui::note("No agent in this workspace")))
                .child(
                    ui::icon_text_button("browser-agents-start", Icon::Plus, "Start an agent", Tone::Plain).on_click(
                        cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.agents.menu = None;
                            dialogs::start_agent(window, cx);
                        }),
                    ),
                )
                .into_any_element();
        };
        let label = format!("Tell {} about it", Self::agent_name(pane, cx));
        entry("browser-agents-tell", Icon::Send, label)
            .when(!ready, |e| e.opacity(0.35))
            .tooltip(tip("Pastes a note into its prompt: where this Chrome runs and how to connect to it"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.tell_agent(cx)))
            .into_any_element()
    }

    fn agent_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let choices = self.agent_choices(cx);
        if choices.len() < 2 {
            return Vec::new();
        }
        let target = self.target_agent(cx);
        let rows = choices.into_iter().map(|choice| {
            let chosen = Some(choice.pane) == target;
            let glyph = if chosen { Icon::Check } else { Icon::Agent };
            entry(("browser-agent", choice.pane), glyph, choice.label)
                .when(chosen, |e| e.text_color(colours::accent()))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.choose_agent(choice.pane, cx)))
                .into_any_element()
        });
        std::iter::once(ui::section("Hand things to").px(px(10.)).pt(px(6.)).into_any_element()).chain(rows).collect()
    }

    pub(super) fn agent_menu_up(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let rows = self.agent_rows(cx);
        popover(menu_card("browser-agent-chooser", 280., cx).children(rows), false)
    }

    fn toggles(&self, cx: &mut Context<Self>) -> Div {
        let reload =
            ui::checkbox("browser-agents-reload", !self.agents.watch.off, "Reload pages when an agent finishes")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.agents.watch.off = !this.agents.watch.off;
                    cx.notify();
                }));
        let follow = ui::checkbox("browser-agents-follow", !self.agents.follow.off, "Show the tabs agents open")
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.agents.follow.off = !this.agents.follow.off;
                cx.notify();
            }));
        div().flex().flex_col().gap(px(6.)).px(px(10.)).py(px(6.)).child(reload).child(follow)
    }

    fn tell_agent(&mut self, cx: &mut Context<Self>) {
        let (Some(url), Some(pane)) = (self.agent_endpoint(), self.target_agent(cx)) else { return };
        self.agents.menu = None;
        if Self::give_to_agent(pane, &driving_note(&url), &[], cx) {
            toast(format!("Told {} where this Chrome runs", Self::agent_name(pane, cx)), ToastKind::Info, cx);
        }
        cx.notify();
    }
}

fn shots_menu(cx: &mut Context<Browser>) -> impl IntoElement + use<> {
    menu_card("browser-shots-menu", 220., cx)
        .child(ui::section("Screenshot for the agent").px(px(10.)).pt(px(6.)).pb(px(2.)))
        .child(
            entry("browser-shot-visible", Icon::Monitor, "Visible area")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.shoot(Shot::Visible, cx))),
        )
        .child(
            entry("browser-shot-full", Icon::File, "Full page")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.shoot(Shot::Full, cx))),
        )
}

fn endpoint_row(endpoint: Option<String>, cx: &mut Context<Browser>) -> impl IntoElement + use<> {
    let Some(url) = endpoint else {
        return div().px(px(10.)).py(px(6.)).child(ui::busy("browser-agents-endpoint", "Starting the browser…"));
    };
    let copied = url.clone();
    div()
        .mx(px(6.))
        .my(px(4.))
        .pl(px(8.))
        .pr(px(4.))
        .py(px(4.))
        .flex()
        .items_center()
        .gap(px(6.))
        .rounded(px(radius::CONTROL))
        .bg(colours::panel())
        .child(div().flex_1().min_w_0().flex().text_size(px(12.)).text_color(colours::text()).child(ui::one_line(url)))
        .child(ui::tool_button("browser-agents-copy", Icon::Copy, 22., false).tooltip(tip("Copy")).on_click(
            cx.listener(move |_, _: &ClickEvent, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(copied.clone()));
                toast("Copied the browser's DevTools address", ToastKind::Info, cx);
            }),
        ))
}

pub(super) fn reloaded(label: String) -> impl IntoElement {
    let pill = glass::card(radius::CARD)
        .flex()
        .items_center()
        .gap(px(6.))
        .max_w(px(260.))
        .px(px(10.))
        .py(px(5.))
        .text_size(px(12.))
        .text_color(colours::soft())
        .child(ui::icon(Icon::Refresh, 12.).text_color(colours::accent()))
        .child(ui::one_line(label));
    div().absolute().top(px(12.)).right(px(12.)).child(pill)
}
