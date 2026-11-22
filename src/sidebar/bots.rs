use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, ParentElement as _, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, div,
    prelude::FluentBuilder as _, px,
};

use super::machine_rows::DOWN_OPACITY;
use super::rows::{hover_listener, rename_box};
use super::{Renaming, card, head_row, tip};
use crate::app::Root;
use crate::cli;
use crate::dialogs;
use crate::keymap::{Command, with_keys};
use crate::model::MachineSeen;
use crate::model::bots::{ASKS_FOR_A_SECRET, Bot, BotKey, Home, Making, Offer, Says, says};
use crate::panels::bot_chat::{list_time, row_detail};
use crate::panels::queue;
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, icon, orbits};

pub(super) const MACHINE_BOTS_ISSUE: &str = "usecornercase/cornercase-terminal#928";
const FACE: f32 = 34.;
const DISC: f32 = 17.;
const STATUS: f32 = 14.;
const BOTS_LEAD: f32 = 16.;
pub(super) const NO_BOTS: &str = "No bots yet. The ones you make are listed here.";

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
    let mut groups: Vec<Group> = mine.into_iter().chain(machines).collect();
    if groups.len() > 1
        && let Some(first) = groups.first_mut().filter(|group| group.machine.is_none())
    {
        first.machine = Some(Heading { icon: Icon::Laptop, name: THIS_COMPUTER.to_string() });
    }
    groups
}

pub(super) const THIS_COMPUTER: &str = "This computer";

pub(super) fn linked_bots_wait(host: Option<&str>, machines: usize) -> Option<String> {
    let host = host.filter(|_| machines > 0)?;
    Some(format!("The bots of machines linked from {host} are not listed here: needs {MACHINE_BOTS_ISSUE}"))
}

struct RowState {
    shown: bool,
    unread: Option<usize>,
    busy: bool,
    hovered: bool,
}

impl Root {
    pub(super) fn bots_card(&self, cx: &mut Context<Self>) -> Div {
        let making = self.store.read(cx).making() == Some(Making::Offered);
        card()
            .min_w_0()
            .min_h_0()
            .child(head_row(BOTS_LEAD, div(), "Bots", div()))
            .child(div().id("sidebar-bots").flex_1().min_h_0().overflow_y_scroll().child(self.bots_body(cx)))
            .when(making, |d| d.child(bots_footer()))
    }

    fn bots_body(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.store.read(cx).bots_offer() {
            Offer::Lacks(text) => ui::note(text).text_size(px(12.)).into_any_element(),
            Offer::Hidden => ui::note(NO_BOTS).text_size(px(12.)).into_any_element(),
            Offer::Rows => self.bot_rows(cx),
        }
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
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .px(px(8.))
            .pb(px(8.))
            .children(rows)
            .children(wait.map(|wait| ui::note(wait).px(px(6.)).py(px(4.)).text_size(px(11.))))
            .into_any_element()
    }

    fn group_rows(&self, group: &Group, (start, active): (usize, bool), cx: &mut Context<Self>) -> Vec<AnyElement> {
        let heading = group.machine.as_ref().map(|heading| group_heading(heading, group.note.as_deref()));
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
        let busy =
            bot.thinking() || self.store.read(cx).bots_at(&bot.home).is_some_and(|bots| bots.runs.answering(&bot.id));
        let unread = Some(bot.unread).filter(|count| *count > 0 && !(shown && active));
        let state = RowState { shown, unread, busy, hovered: self.row_hovered(&gid) };
        let row = ui::row(("sidebar-bot", index), shown)
            .group(gid.clone())
            .gap(px(10.))
            .px(px(8.))
            .py(px(7.))
            .rounded(px(radius::card() - 4.))
            .when(dim, |d| d.opacity(DOWN_OPACITY))
            .child(self.bot_face(self.store.read(cx).face_of(&bot.key()), bot, busy))
            .child(self.bot_text(bot, &state, &gid, cx));
        Self::bot_row_acts(row.on_hover(hover_listener(gid, cx)), bot, cx).into_any_element()
    }

    fn bot_row_acts(row: Stateful<Div>, bot: &Bot, cx: &mut Context<Self>) -> Stateful<Div> {
        let own = bot.home == Home::Here;
        let (key, press, id) = (bot.key(), bot.key(), bot.id.clone());
        row.on_mouse_down(
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
    }

    fn bot_face(&self, colour: u8, bot: &Bot, busy: bool) -> Div {
        let mark = if busy {
            Some(orbits::badge(Some("working"), &self.parts.spots, 1.))
        } else if bot.asks.is_empty() {
            None
        } else {
            Some(ui::still_badge(Some("waiting")))
        };
        let status = mark.map(|mark| {
            div()
                .absolute()
                .right(px(-3.))
                .bottom(px(-3.))
                .size(px(STATUS + 2.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(colours::raised_hover())
                .child(mark)
        });
        div().relative().flex_none().child(ui::face::face(colour, FACE)).children(status)
    }

    fn bot_text(&self, bot: &Bot, state: &RowState, gid: &SharedString, cx: &mut Context<Self>) -> Div {
        let renaming = (bot.home == Home::Here).then(|| self.rename_field(&Renaming::Bot(bot.id.clone()))).flatten();
        let name = match renaming {
            Some(field) => rename_box(field).flex_1().min_w_0().into_any_element(),
            None => name_label(bot, state).into_any_element(),
        };
        let first = div().flex().items_center().gap(px(6.)).child(name).child(Self::first_end(bot, state, gid, cx));
        let second = div().flex().items_center().gap(px(6.)).child(preview_line(bot, state)).children(unread(state));
        div().flex_1().min_w_0().flex().flex_col().gap(px(1.)).child(first).child(second)
    }

    fn first_end(bot: &Bot, state: &RowState, gid: &SharedString, cx: &mut Context<Self>) -> AnyElement {
        if state.hovered {
            return Self::bot_menu_button(&bot.key(), gid, cx).into_any_element();
        }
        let when = bot.last.as_ref().map(|last| list_time(last.at, queue::now())).unwrap_or_default();
        let colour = if state.unread.is_some() { colours::brand() } else { colours::faint() };
        div().flex_none().text_size(px(11.)).text_color(colour).child(when).into_any_element()
    }

    fn bot_menu_button(key: &BotKey, gid: &SharedString, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let key = key.clone();
        div()
            .id(SharedString::from(format!("bot-menu-{}", key.tag())))
            .flex_none()
            .h(px(16.))
            .px(px(5.))
            .flex()
            .items_center()
            .rounded(px(radius::chip()))
            .text_size(px(12.))
            .text_color(colours::muted())
            .bg(colours::raised())
            .hover(|s| s.text_color(colours::text()))
            .group_hover(gid.clone(), |s| s.bg(colours::hover()))
            .child("⋯")
            .tooltip(tip("More"))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.open_bot_menu(&key, e.position, window, cx);
                }),
            )
    }
}

fn name_label(bot: &Bot, state: &RowState) -> Div {
    let strong = state.unread.is_some() || state.shown;
    div()
        .flex_1()
        .min_w_0()
        .text_size(px(13.))
        .when(strong, |d| d.font_weight(FontWeight::SEMIBOLD))
        .text_color(if strong { colours::text() } else { colours::soft() })
        .child(ui::one_line(bot.name.clone()))
}

fn preview_line(bot: &Bot, state: &RowState) -> Div {
    let (text, colour) = match says(bot.last.as_ref(), (!bot.asks.is_empty(), state.busy)) {
        Says::Asks => (ASKS_FOR_A_SECRET.to_string(), colours::waiting()),
        Says::Thinking => ("Thinking…".to_string(), colours::working()),
        Says::Line(line) => (line, if state.unread.is_some() { colours::soft() } else { colours::muted() }),
        Says::Nothing => (row_detail(bot, state.busy, queue::now()), colours::faint()),
    };
    div().flex_1().min_w_0().text_size(px(12.)).text_color(colour).child(ui::one_line(text))
}

fn unread(state: &RowState) -> Option<Div> {
    let count = state.unread?;
    let text = if count > 99 { "99+".to_string() } else { count.to_string() };
    Some(
        div()
            .flex_none()
            .min_w(px(DISC))
            .h(px(DISC))
            .px(px(5.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(colours::brand())
            .text_size(px(10.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(colours::on_brand())
            .child(text),
    )
}

fn group_heading(heading: &Heading, note: Option<&str>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(1.))
        .px(px(8.))
        .pt(px(10.))
        .pb(px(3.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(5.))
                .text_size(px(11.))
                .font_weight(FontWeight::SEMIBOLD)
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

fn bots_footer() -> impl IntoElement {
    div().flex().px(px(12.)).pt(px(8.)).pb(px(12.)).child(
        ui::labelled_button("sidebar-bots-new", Icon::Plus, "New bot", true)
            .tooltip(tip(with_keys("New bot", Command::NewBot)))
            .on_click(|_: &ClickEvent, window, cx| dialogs::new_bot(window, cx)),
    )
}

#[cfg(test)]
mod tests;
