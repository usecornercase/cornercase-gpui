use gpui::{
    AnyElement, ClickEvent, Context, Div, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _,
    px, relative,
};

use super::arrange::divider;
use super::head_row;
use super::machine_rows::DOWN_OPACITY;
use super::rail::count_disc;
use super::rows::{End, chevron, hover_button, hover_listener, list_text, rename_box, trailing};
use super::workspace::{agent_lines, agent_lines_in, name_line};
use super::{Renaming, tip};
use crate::app::Root;
use crate::cli;
use crate::dialogs;
use crate::keymap::{Command, with_keys};
use crate::model::MachineSeen;
use crate::model::bots::{ASKS_FOR_A_SECRET, Bot, BotKey, Home, Making, Offer};
use crate::panels::bot_chat::row_detail;
use crate::panels::queue;
use crate::theme::colours;
use crate::ui::{self, Icon, icon, orbits};

pub(super) const MACHINE_BOTS_ISSUE: &str = "usecornercase/cornercase-terminal#928";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Heading {
    pub(super) icon: Icon,
    pub(super) name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Group {
    pub(super) machine: Option<Heading>,
    pub(super) note: Option<String>,
    pub(super) bots: Vec<Bot>,
}

pub(super) fn bot_groups(here: &[Bot], there: Vec<(&MachineSeen, &[Bot], Option<String>)>) -> Vec<Group> {
    let mine = (!here.is_empty()).then(|| Group { machine: None, note: None, bots: here.to_vec() });
    let machines = there.into_iter().filter(|(_, bots, _)| !bots.is_empty()).map(|(seen, bots, note)| Group {
        machine: Some(Heading { icon: seen.icon.icon(), name: seen.name.clone() }),
        note,
        bots: bots.to_vec(),
    });
    mine.into_iter().chain(machines).collect()
}

pub(super) fn linked_bots_wait(host: Option<&str>, machines: usize) -> Option<String> {
    let host = host.filter(|_| machines > 0)?;
    Some(format!("The bots of machines linked from {host} are not listed here: needs {MACHINE_BOTS_ISSUE}"))
}

impl Root {
    pub(super) fn bots_section(&self, cx: &mut Context<Self>) -> Option<Div> {
        let store = self.store.read(cx);
        let offer = store.bots_offer();
        if offer == Offer::Hidden {
            return None;
        }
        let folded = self.side.bots_folded;
        let (thinking, unread) = (store.any_bot_thinking(), store.bots_unread() as u64);
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
        let groups = bot_groups(&store.bots.list, store.bots_on_machines());
        let machines = store.session.as_ref().map_or(0, |session| session.machines.len());
        let wait = linked_bots_wait(cli::host().as_deref(), machines);
        let mut rows: Vec<AnyElement> = Vec::new();
        for group in &groups {
            let start = rows.len();
            rows.extend(self.group_rows(group, (start, active), cx));
        }
        let list = list_text(div().flex().flex_col().gap(px(1.)).px(px(10.)).pb(px(4.)))
            .children(rows)
            .children(wait.map(|wait| ui::note(wait).px(px(6.)).py(px(4.)).text_size(px(11.))));
        div().id("sidebar-bots").min_h_0().overflow_y_scroll().child(list).into_any_element()
    }

    fn group_rows(&self, group: &Group, (start, active): (usize, bool), cx: &mut Context<Self>) -> Vec<AnyElement> {
        let heading = group.machine.as_ref().map(|heading| machine_heading(heading, group.note.as_deref()));
        let dim = group.note.is_some();
        let rows = group.bots.iter().enumerate().map(|(at, bot)| {
            let shown = self.side.bot.as_ref() == Some(&bot.key());
            self.bot_row(start + at, bot, (shown, active, dim), cx)
        });
        heading.into_iter().chain(rows.collect::<Vec<_>>()).collect()
    }

    fn bot_row(&self, index: usize, bot: &Bot, on_screen: (bool, bool, bool), cx: &mut Context<Self>) -> AnyElement {
        let gid = SharedString::from(format!("bot-{}", bot.key().tag()));
        let (shown, active, dim) = on_screen;
        let own = bot.home == Home::Here;
        let hovered = self.row_hovered(&gid);
        let busy =
            bot.thinking() || self.store.read(cx).bots_at(&bot.home).is_some_and(|bots| bots.runs.answering(&bot.id));
        let row = ui::row(("sidebar-bot", index), shown)
            .group(gid.clone())
            .gap(px(4.))
            .px(px(6.))
            .py(px(3.))
            .when(dim, |d| d.opacity(DOWN_OPACITY))
            .child(self.bot_lead(busy))
            .child(self.bot_text(bot, (shown, busy)))
            .child(Self::bot_end(bot, (shown && active, hovered), &gid, cx));
        let (key, press, id) = (bot.key(), bot.key(), bot.id.clone());
        row.on_hover(hover_listener(gid, cx))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    this.open_bot_menu(&press, e.position, window, cx);
                }),
            )
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                if own && e.click_count() == 2 {
                    this.start_bot_rename(&id, window, cx);
                } else {
                    this.store.update(cx, |s, cx| s.show_bot(&key, cx));
                }
            }))
            .into_any_element()
    }

    fn bot_lead(&self, busy: bool) -> Div {
        if busy {
            orbits::badge(Some("working"), &self.parts.spots, 1.)
        } else {
            ui::badge_slot().child(icon(Icon::Message, 12.).text_color(colours::faint()))
        }
    }

    fn bot_end(bot: &Bot, (seen, hovered): (bool, bool), gid: &SharedString, cx: &mut Context<Self>) -> Div {
        let unread = u64::try_from(bot.unread).unwrap_or(u64::MAX);
        let disc = count_disc(unread).filter(|_| !seen);
        trailing(End { gap: 4., buttons: 1, figures: disc.is_some() }, hovered)
            .children(disc.map(gpui::Styled::flex_none))
            .when(hovered, |d| d.child(Self::bot_menu_button(&bot.key(), gid, cx)))
    }

    fn bot_text(&self, bot: &Bot, (shown, busy): (bool, bool)) -> impl IntoElement + use<> {
        let renaming = (bot.home == Home::Here).then(|| self.rename_field(&Renaming::Bot(bot.id.clone()))).flatten();
        let first = match renaming {
            Some(field) => rename_box(field).py(px(1.)).into_any_element(),
            None => name_line(bot.name.clone(), shown).into_any_element(),
        };
        if bot.asks.is_empty() {
            agent_lines(first, row_detail(bot, busy, queue::now()))
        } else {
            agent_lines_in(first, ASKS_FOR_A_SECRET.to_string(), colours::waiting())
        }
    }

    fn bot_menu_button(key: &BotKey, gid: &SharedString, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let key = key.clone();
        hover_button(SharedString::from(format!("bot-menu-{}", key.tag())), gid.clone(), "⋯", 0).on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                this.open_bot_menu(&key, e.position, window, cx);
            }),
        )
    }
}

fn machine_heading(heading: &Heading, note: Option<&str>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(1.))
        .px(px(6.))
        .pt(px(6.))
        .pb(px(2.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .text_size(px(11.))
                .text_color(colours::muted())
                .child(icon(heading.icon, 11.).text_color(colours::muted()))
                .child(div().flex_1().min_w_0().child(ui::one_line(heading.name.clone()))),
        )
        .children(
            note.map(|note| {
                div().text_size(px(11.)).text_color(colours::faint()).child(ui::one_line(note.to_string()))
            }),
        )
        .into_any_element()
}

fn new_bot_button() -> impl IntoElement {
    ui::tool_button("sidebar-bots-new", Icon::Plus, 24., false)
        .tooltip(tip(with_keys("New bot", Command::NewBot)))
        .on_click(|_: &ClickEvent, window, cx| {
            cx.stop_propagation();
            dialogs::new_bot(window, cx);
        })
}

#[cfg(test)]
mod tests;
