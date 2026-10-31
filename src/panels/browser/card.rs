use std::path::PathBuf;

use gpui::{
    AnyElement, AppContext as _, ClickEvent, Context, Div, Entity, Focusable as _, InteractiveElement as _,
    IntoElement, MouseButton, MouseDownEvent, ParentElement as _, StatefulInteractiveElement as _, Styled as _, Window,
    div, prelude::FluentBuilder as _, px,
};

use super::Browser;
use super::agent::{Menu, toast};
use super::agent_tools::reloaded;
use super::picker::{Crop, Pick, Picked};
use crate::dialogs;
use crate::store::ToastKind;
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, TextField, TextFieldEvent, Tone, glass};

const INSET: f32 = 12.;

impl Browser {
    pub(super) fn sync_agents(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ask = self.ask_field(window, cx);
        let focused = |handle: gpui::FocusHandle| handle.is_focused(window);
        let typing = focused(self.address.read(cx).focus_handle(cx)) || focused(ask.read(cx).focus_handle(cx));
        self.agents.follow.engaged = typing || self.focus.is_focused(window) || self.agents.menu.is_some();
        if std::mem::take(&mut self.agents.focus_ask) {
            cx.defer_in(window, move |_, window, cx| window.focus(&ask.read(cx).focus_handle(cx)));
        }
        let wandered = self.agents.pick.as_ref().and_then(|p| p.inspecting()).map(str::to_string);
        if wandered.is_some_and(|session| Some(session) != self.active_session()) {
            self.cancel_pick(cx);
        }
    }

    fn ask_field(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<TextField> {
        if let Some(ask) = &self.agents.ask {
            return ask.clone();
        }
        let ask = cx.new(|cx| TextField::new("What should change?", cx).wraps());
        cx.subscribe_in(&ask, window, |this, _, event, window, cx| match event {
            TextFieldEvent::Submit => this.send_pick(cx),
            TextFieldEvent::Cancel => {
                this.cancel_pick(cx);
                window.focus(&this.focus);
            }
            TextFieldEvent::Changed => {}
        })
        .detach();
        self.agents.ask = Some(ask.clone());
        ask
    }

    pub(super) fn ask_for_change(&mut self, cx: &mut Context<Self>) {
        if let Some(ask) = &self.agents.ask {
            ask.update(cx, |field, cx| field.set_text("", cx));
        }
        self.agents.focus_ask = true;
    }

    fn instruction(&self, cx: &Context<Self>) -> String {
        self.agents.ask.as_ref().map(|a| a.read(cx).text().trim().to_string()).unwrap_or_default()
    }

    pub(super) fn send_pick(&mut self, cx: &mut Context<Self>) {
        let Some(Pick::Picked(picked)) = &self.agents.pick else { return };
        if matches!(picked.crop, Crop::Capturing) {
            return;
        }
        let Some(pane) = self.target_agent(cx) else {
            toast("No agent in this workspace", ToastKind::Info, cx);
            return;
        };
        let crop: Vec<PathBuf> = match &picked.crop {
            Crop::Ready(path) => vec![path.clone()],
            Crop::Capturing | Crop::Missing => Vec::new(),
        };
        let brief = picked.element.brief(!crop.is_empty());
        let instruction = self.instruction(cx);
        let sent = if instruction.is_empty() {
            let files: Vec<&std::path::Path> = crop.iter().map(PathBuf::as_path).collect();
            Self::give_to_agent(pane, &format!("{brief}\n"), &files, cx)
        } else {
            Self::submit_to_agent(pane, format!("{brief}\n\n{instruction}"), &crop, cx)
        };
        if sent {
            toast(format!("Handed to {}", Self::agent_name(pane, cx)), ToastKind::Info, cx);
            self.agents.pick = None;
            cx.notify();
        }
    }

    pub(super) fn agent_overlay(&self, window: &Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let pick = self.agents.pick.as_ref().map(|pick| match pick {
            Pick::Picking(_) => banner(false, cx).into_any_element(),
            Pick::Reading(_) => banner(true, cx).into_any_element(),
            Pick::Picked(picked) => self.picked_card(picked, window, cx).into_any_element(),
        });
        let flash = self.agents.watch.flash.as_ref().map(|(_, label)| reloaded(label.clone()).into_any_element());
        pick.into_iter().chain(flash).collect()
    }

    fn picked_card(&self, picked: &Picked, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let ask = self.agents.ask.as_ref().map(|ask| ui::boxed(ask, window, cx));
        let card = glass::card(radius::CARD)
            .id("browser-picked")
            .occlude()
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .on_mouse_down(MouseButton::Left, |_: &MouseDownEvent, _, cx| cx.stop_propagation())
            .child(picked_heading(picked, cx))
            .children(
                picked
                    .element
                    .detail()
                    .map(|d| div().text_size(px(12.)).text_color(colours::muted()).child(ui::one_line(d))),
            )
            .children(ask)
            .child(self.picked_footer(picked, cx));
        div().absolute().left(px(INSET)).right(px(INSET)).bottom(px(INSET)).child(card)
    }

    fn picked_footer(&self, picked: &Picked, cx: &mut Context<Self>) -> Div {
        let row = div().flex().items_center().gap(px(8.));
        let Some(pane) = self.target_agent(cx) else {
            return row.child(div().flex_1().child(ui::note("No agent in this workspace"))).child(start_button(cx));
        };
        let send = match picked.crop {
            Crop::Capturing => ui::busy_button("browser-pick-send", "Capturing…", Tone::Primary),
            Crop::Ready(_) | Crop::Missing => {
                ui::icon_text_button("browser-pick-send", Icon::Send, "Send to agent", Tone::Primary)
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.send_pick(cx)))
            }
        };
        row.child(div().flex_1().min_w_0().child(self.chooser(pane, cx))).child(send)
    }

    fn chooser(&self, pane: u64, cx: &mut Context<Self>) -> Div {
        let name = Self::agent_name(pane, cx);
        let many = self.agent_choices(cx).len() > 1;
        let label = div().min_w_0().flex().text_size(px(12.)).text_color(colours::soft()).child(ui::one_line(name));
        let open = self.agents.menu == Some(Menu::Chooser);
        let chip = div()
            .id("browser-pick-agent")
            .flex()
            .items_center()
            .gap(px(6.))
            .px(px(8.))
            .py(px(4.))
            .rounded(px(radius::CONTROL))
            .child(ui::icon(Icon::Agent, 13.).text_color(colours::accent()))
            .child(label)
            .when(many, |chip| {
                chip.cursor_pointer()
                    .hover(|s| s.bg(colours::hover()))
                    .child(ui::icon(Icon::ChevronDown, 12.).text_color(colours::faint()))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_menu(Menu::Chooser, cx)))
            });
        let menu = (open && many).then(|| self.agent_menu_up(cx));
        div().relative().flex().child(chip).children(menu)
    }
}

fn picked_heading(picked: &Picked, cx: &mut Context<Browser>) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(ui::icon(Icon::Pick, 14.).text_color(colours::accent()))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .text_size(px(13.))
                .text_color(colours::text())
                .child(ui::one_line(picked.element.heading())),
        )
        .child(ui::icon_button("browser-pick-close", "✕").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.cancel_pick(cx);
        })))
}

fn start_button(cx: &mut Context<Browser>) -> impl IntoElement + use<> {
    ui::icon_text_button("browser-pick-start", Icon::Plus, "Start an agent", Tone::Plain)
        .on_click(cx.listener(|_, _: &ClickEvent, window, cx| dialogs::start_agent(window, cx)))
}

fn banner(reading: bool, cx: &mut Context<Browser>) -> impl IntoElement + use<> {
    let lead = if reading {
        ui::busy("browser-pick-reading", "Reading the element…").into_any_element()
    } else {
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .text_size(px(12.))
            .text_color(colours::soft())
            .child(ui::icon(Icon::Pick, 14.).text_color(colours::accent()))
            .child("Click an element to pick it")
            .into_any_element()
    };
    let cancel = ui::button("browser-pick-cancel", "Cancel").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
        this.cancel_pick(cx);
    }));
    let pill = glass::card(radius::CARD)
        .id("browser-picking")
        .occlude()
        .flex()
        .items_center()
        .gap(px(12.))
        .pl(px(12.))
        .pr(px(6.))
        .py(px(6.))
        .child(lead)
        .child(cancel);
    div().absolute().top(px(INSET)).left_0().right_0().flex().justify_center().child(pill)
}
