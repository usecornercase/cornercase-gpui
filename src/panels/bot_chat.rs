use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui::{
    AnyElement, AnyView, App, AppContext as _, Context, Entity, Global, IntoElement, ListAlignment, ListOffset,
    ListState, ParentElement as _, Render, StyleRefinement, Styled as _, Window, div, list,
    prelude::FluentBuilder as _, px,
};

use self::rows::{Blocks, Meta, chip, earlier_row, entry_row};
use self::words::{EMPTY_LINES, footer, turn_label};
use crate::model::bots::entries::{self, Entry, EntryKey};
use crate::model::bots::{Bot, Chat, Read};
use crate::panels::queue;
use crate::store::Store;
use crate::theme::{colours, radius};
use crate::ui::counting::Counting;
use crate::ui::{self, Icon};

mod rows;
mod words;

const OVERDRAW: f32 = 600.;
const LINE: f32 = 48.;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Scroll {
    Line(f32),
    Page(f32),
}

pub(crate) fn scroll_of(key: &str) -> Option<Scroll> {
    match key {
        "up" => Some(Scroll::Line(-1.)),
        "down" => Some(Scroll::Line(1.)),
        "pageup" => Some(Scroll::Page(-1.)),
        "pagedown" => Some(Scroll::Page(1.)),
        _ => None,
    }
}

pub(crate) fn scroll_distance(step: Scroll, viewport: f32) -> f32 {
    match step {
        Scroll::Line(sign) => sign * LINE,
        Scroll::Page(sign) => sign * (viewport - LINE).max(LINE),
    }
}

#[derive(Clone)]
enum Row {
    Earlier,
    Entry(Entry),
}

pub(crate) fn row_keys(earlier: bool, entries: &[Entry], opened: &HashSet<u64>) -> Vec<EntryKey> {
    let keys = entries::keys(entries, opened);
    if earlier { std::iter::once(entries::earlier_key()).chain(keys).collect() } else { keys }
}

pub(crate) struct BotChat {
    store: Entity<Store>,
    list: ListState,
    bot: Option<String>,
    rows: Rc<Vec<Row>>,
    keys: Vec<EntryKey>,
    opened: HashSet<u64>,
    news: bool,
    blocks: Blocks,
    counting: Counting,
    mark: Option<(String, u64)>,
    heard: (Option<String>, u64),
}

struct GlobalBotChat(Entity<BotChat>);
impl Global for GlobalBotChat {}

pub(crate) fn view(cx: &mut App) -> AnyView {
    AnyView::from(BotChat::get(cx)).cached(StyleRefinement::default().size_full())
}

pub(crate) fn scroll(step: Scroll, cx: &mut App) {
    BotChat::get(cx).update(cx, |chat, cx| chat.scroll(step, cx));
}

fn counting(chat: &mut BotChat) -> &mut Counting {
    &mut chat.counting
}

fn at_end(list: &ListState) -> bool {
    -list.scroll_px_offset_for_scrollbar().y >= list.max_offset_for_scrollbar().height - px(2.)
}

impl BotChat {
    pub(crate) fn get(cx: &mut App) -> Entity<Self> {
        if let Some(GlobalBotChat(chat)) = cx.try_global::<GlobalBotChat>() {
            return chat.clone();
        }
        let store = Store::get(cx);
        let chat = cx.new(|cx| Self::new(store, cx));
        cx.set_global(GlobalBotChat(chat.clone()));
        chat
    }

    fn new(store: Entity<Store>, cx: &mut Context<Self>) -> Self {
        cx.observe(&store, |this, store, cx| {
            if this.moved(store.read(cx)) {
                cx.notify();
            }
        })
        .detach();
        let list = ListState::new(0, ListAlignment::Top, px(OVERDRAW));
        let chat = cx.entity().downgrade();
        list.set_scroll_handler(move |event, _, cx| {
            if event.visible_range.end >= event.count {
                let _ = chat.update(cx, BotChat::reached_the_end);
            }
        });
        Self {
            store,
            list,
            bot: None,
            rows: Rc::new(Vec::new()),
            keys: Vec::new(),
            opened: HashSet::new(),
            news: false,
            blocks: Rc::new(RefCell::new(HashMap::new())),
            counting: Counting::default(),
            mark: None,
            heard: (None, 0),
        }
    }

    fn moved(&mut self, store: &Store) -> bool {
        let now = (store.shown_bot().map(str::to_string), store.bots_revision());
        (std::mem::replace(&mut self.heard, now.clone()) != now) || self.bot.is_some() && now.0.is_none()
    }

    fn open_steps(&mut self, first: u64, cx: &mut Context<Self>) {
        if !self.opened.remove(&first) {
            self.opened.insert(first);
        }
        self.mark = None;
        cx.notify();
    }

    fn reached_the_end(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.news) {
            cx.notify();
        }
    }

    fn scroll_to_end(&mut self, cx: &mut Context<Self>) {
        self.list.scroll_to(ListOffset { item_ix: self.rows.len(), offset_in_item: px(0.) });
        self.news = false;
        cx.notify();
    }

    fn scroll(&mut self, step: Scroll, cx: &mut Context<Self>) {
        let viewport = f32::from(self.list.viewport_bounds().size.height);
        self.list.scroll_by(px(scroll_distance(step, viewport)));
        if at_end(&self.list) {
            self.news = false;
        }
        cx.notify();
    }

    fn sync(&mut self, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let chat = store.shown_bot().and_then(|id| store.bots.chat.as_ref().filter(|chat| chat.bot == id));
        let Some(chat) = chat else {
            self.bot = None;
            return;
        };
        let fresh = self.bot.as_deref() != Some(chat.bot.as_str());
        let stamp = (chat.bot.clone(), chat.revision());
        if !fresh && self.mark.as_ref() == Some(&stamp) {
            return;
        }
        self.mark = Some(stamp);
        let (earlier, entries) = (chat.earlier(), entries::entries(&chat.lines));
        let first = chat.lines.first().map_or(0, |line| line.id);
        if fresh {
            self.bot = Some(chat.bot.clone());
            self.opened.clear();
            self.blocks.borrow_mut().clear();
        }
        self.blocks.borrow_mut().retain(|id, _| *id >= first);
        let keys = row_keys(earlier, &entries, &self.opened);
        let rows: Vec<Row> =
            earlier.then_some(Row::Earlier).into_iter().chain(entries.into_iter().map(Row::Entry)).collect();
        self.apply(fresh, keys, rows);
    }

    fn apply(&mut self, fresh: bool, keys: Vec<EntryKey>, rows: Vec<Row>) {
        if fresh {
            self.list.reset(rows.len());
            self.list.scroll_to(ListOffset { item_ix: rows.len(), offset_in_item: px(0.) });
            self.news = false;
        } else if keys != self.keys {
            let following = at_end(&self.list);
            for (range, count) in entries::spliced(&self.keys, &keys) {
                self.list.splice(range, count);
            }
            if following {
                self.list.scroll_to(ListOffset { item_ix: rows.len(), offset_in_item: px(0.) });
            } else if keys.last() != self.keys.last() {
                self.news = true;
            }
        }
        self.keys = keys;
        self.rows = Rc::new(rows);
    }

    fn meta(&self, bot: &Bot, cx: &mut Context<Self>) -> Meta {
        Meta {
            name: bot.name.clone().into(),
            now: queue::now(),
            chat: cx.entity().downgrade(),
            opened: Rc::new(self.opened.clone()),
            blocks: self.blocks.clone(),
        }
    }

    fn body(&mut self, bot: &Bot, chat: &Chat, cx: &mut Context<Self>) -> AnyElement {
        match &chat.read {
            Read::Failed(why) => ui::note(format!("Could not read the chat of {}: {why}", bot.name)).into_any_element(),
            Read::Asking if chat.lines.is_empty() => ui::loading("Reading the chat…").into_any_element(),
            _ if chat.lines.is_empty() && !bot.thinking() => empty_page(bot).into_any_element(),
            _ => self.lines(bot, cx),
        }
    }

    fn lines(&mut self, bot: &Bot, cx: &mut Context<Self>) -> AnyElement {
        let (rows, meta) = (self.rows.clone(), self.meta(bot, cx));
        let chat = cx.entity().downgrade();
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .child(
                list(self.list.clone(), move |ix, _, _| match rows.get(ix) {
                    Some(Row::Entry(entry)) => entry_row(entry, &meta),
                    Some(Row::Earlier) => earlier_row(),
                    None => div().into_any_element(),
                })
                .size_full(),
            )
            .when(self.news, |d| d.child(chip(chat)))
            .into_any_element()
    }
}

fn empty_page(bot: &Bot) -> impl IntoElement + use<> {
    let [what, when] = EMPTY_LINES;
    div().flex_1().child(
        ui::emblem_page(Icon::Message, bot.name.clone()).child(ui::emblem_line(what)).child(ui::emblem_line(when)),
    )
}

fn header(bot: &Bot) -> impl IntoElement + use<> {
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(10.))
        .px(px(16.))
        .py(px(10.))
        .border_b_1()
        .border_color(colours::border())
        .child(div().font_weight(gpui::FontWeight::SEMIBOLD).text_color(colours::text()).child(bot.name.clone()))
        .child(div().min_w_0().text_size(px(11.5)).text_color(colours::faint()).child(ui::one_line(bot.facts())))
}

fn turn_row(bot: &Bot, chat: &Chat, now: i64) -> Option<impl IntoElement + use<>> {
    let turn = entries::turn(&chat.lines, bot.thinking())
        .or_else(|| bot.thinking().then_some(entries::Turn { since: now, steps: 0 }))?;
    Some(
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(16.))
            .py(px(6.))
            .text_size(px(12.))
            .text_color(colours::muted())
            .child(ui::spinner("bot-thinking", 12., colours::working()))
            .child(turn_label(turn, now)),
    )
}

fn footer_row(bot: &Bot) -> impl IntoElement + use<> {
    div()
        .flex_none()
        .px(px(16.))
        .py(px(10.))
        .border_t_1()
        .border_color(colours::border())
        .text_size(px(12.))
        .text_color(colours::faint())
        .child(footer(&bot.name, &bot.id))
}

impl Render for BotChat {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync(cx);
        let store = self.store.read(cx);
        let found = store.shown_bot().and_then(|id| store.bots.find(id).zip(store.bots.chat.as_ref()));
        let Some((bot, chat)) = found.map(|(bot, chat)| (bot.clone(), chat.clone())) else { return div() };
        let now = queue::now();
        if bot.thinking() {
            self.counting.drawn();
        }
        self.counting.keep(counting, cx);
        let body = self.body(&bot, &chat, cx);
        div().size_full().p(px(8.)).child(
            div()
                .size_full()
                .flex()
                .flex_col()
                .overflow_hidden()
                .rounded(px(radius::card()))
                .bg(colours::pane())
                .border_1()
                .border_color(colours::border())
                .child(header(&bot))
                .child(body)
                .children(turn_row(&bot, &chat, now))
                .child(footer_row(&bot)),
        )
    }
}

#[cfg(test)]
mod tests;
