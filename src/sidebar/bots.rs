use gpui::{
    AnyElement, ClickEvent, Context, Div, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _,
    px, relative,
};

use super::arrange::divider;
use super::head_row;
use super::rail::count_disc;
use super::rows::{End, chevron, hover_button, hover_listener, list_text, rename_box, trailing};
use super::workspace::{agent_lines, agent_lines_in, name_line};
use super::{Renaming, tip};
use crate::app::Root;
use crate::dialogs;
use crate::keymap::{Command, with_keys};
use crate::model::bots::{ASKS_FOR_A_SECRET, Bot, Making, Offer};
use crate::panels::bot_chat::row_detail;
use crate::panels::queue;
use crate::theme::colours;
use crate::ui::{self, Icon, icon, orbits};

impl Root {
    pub(super) fn bots_section(&self, cx: &mut Context<Self>) -> Option<Div> {
        let store = self.store.read(cx);
        let offer = store.bots_offer();
        if offer == Offer::Hidden {
            return None;
        }
        let folded = self.side.bots_folded;
        let (thinking, unread) = (store.bots.thinking(), store.bots.unread() as u64);
        let plus = offer == Offer::Rows && store.making() == Some(Making::Offered);
        let body = match offer {
            Offer::Lacks(text) => Some(ui::note(text).into_any_element()),
            _ if folded => None,
            _ => Some(self.bot_rows(cx)),
        };
        Some(
            div()
                .flex_none()
                .max_h(relative(0.5))
                .flex()
                .flex_col()
                .child(divider())
                .child(Self::bots_head(folded, (thinking, unread), plus, cx))
                .children(body),
        )
    }

    fn bots_head(
        folded: bool,
        (thinking, unread): (bool, u64),
        plus: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let still = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .when(folded && thinking, |d| d.child(ui::spinner("bots-thinking", 11., colours::working())))
            .children(count_disc(unread).filter(|_| folded))
            .when(plus, |d| d.child(new_bot_button()));
        let chevron = chevron(SharedString::from("bots-chevron"), !folded);
        head_row(6., chevron, "Bots", still).id("sidebar-bots-head").cursor_pointer().on_click(cx.listener(
            |this, _: &ClickEvent, _, cx| {
                this.side.bots_folded = !this.side.bots_folded;
                cx.notify();
            },
        ))
    }

    fn bot_rows(&self, cx: &mut Context<Self>) -> AnyElement {
        let store = self.store.read(cx);
        let active = store.bots.active;
        let bots: Vec<Bot> = store.bots.list.clone();
        let rows: Vec<AnyElement> = bots
            .iter()
            .enumerate()
            .map(|(index, bot)| {
                let shown = self.side.bot.as_deref() == Some(bot.id.as_str());
                self.bot_row(index, bot, (shown, active), cx)
            })
            .collect();
        let list = list_text(div().flex().flex_col().gap(px(1.)).px(px(10.)).pb(px(4.))).children(rows);
        div().id("sidebar-bots").min_h_0().overflow_y_scroll().child(list).into_any_element()
    }

    fn bot_row(&self, index: usize, bot: &Bot, on_screen: (bool, bool), cx: &mut Context<Self>) -> AnyElement {
        let (id, gid) = (bot.id.clone(), SharedString::from(format!("bot-{}", bot.id)));
        let busy = bot.thinking() || self.store.read(cx).bots.runs.answering(&bot.id);
        let lead = if busy {
            orbits::badge(Some("working"), &self.parts.spots, 1.)
        } else {
            ui::badge_slot().child(icon(Icon::Message, 12.).text_color(colours::faint()))
        };
        let unread = u64::try_from(bot.unread).unwrap_or(u64::MAX);
        let (shown, active) = on_screen;
        let disc = count_disc(unread).filter(|_| !(shown && active));
        let hovered = self.row_hovered(&gid);
        let end = trailing(End { gap: 4., buttons: 1, figures: disc.is_some() }, hovered)
            .children(disc.map(gpui::Styled::flex_none))
            .when(hovered, |d| d.child(Self::bot_menu_button(&id, &gid, cx)));
        let (click, press) = (id.clone(), id.clone());
        ui::row(("sidebar-bot", index), shown)
            .group(gid.clone())
            .gap(px(4.))
            .px(px(6.))
            .py(px(3.))
            .child(lead)
            .child(self.bot_text(bot, (shown, busy)))
            .child(end)
            .on_hover(hover_listener(gid, cx))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    this.open_bot_menu(&press, e.position, window, cx);
                }),
            )
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                if e.click_count() == 2 {
                    this.start_bot_rename(&click, window, cx);
                } else {
                    this.store.update(cx, |s, cx| s.show_bot(&click, cx));
                }
            }))
            .into_any_element()
    }

    fn bot_text(&self, bot: &Bot, (shown, busy): (bool, bool)) -> impl IntoElement + use<> {
        let first = match self.rename_field(&Renaming::Bot(bot.id.clone())) {
            Some(field) => rename_box(field).py(px(1.)).into_any_element(),
            None => name_line(bot.name.clone(), shown).into_any_element(),
        };
        if bot.asks.is_empty() {
            agent_lines(first, row_detail(bot, busy, queue::now()))
        } else {
            agent_lines_in(first, ASKS_FOR_A_SECRET.to_string(), colours::waiting())
        }
    }

    fn bot_menu_button(id: &str, gid: &SharedString, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let id = id.to_string();
        hover_button(SharedString::from(format!("bot-menu-{id}")), gid.clone(), "⋯", 0).on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                this.open_bot_menu(&id, e.position, window, cx);
            }),
        )
    }
}

fn new_bot_button() -> impl IntoElement {
    ui::tool_button("sidebar-bots-new", Icon::Plus, 24., false)
        .tooltip(tip(with_keys("New bot", Command::NewBot)))
        .on_click(|_: &ClickEvent, window, cx| {
            cx.stop_propagation();
            dialogs::new_bot(window, cx);
        })
}
