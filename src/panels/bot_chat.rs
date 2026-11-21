use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui::{
    AnyElement, Bounds, Context, Div, Entity, FocusHandle, InteractiveElement as _, IntoElement, ListAlignment,
    ListOffset, ListState, ParentElement as _, Pixels, Render, ScrollHandle, StatefulInteractiveElement as _,
    Styled as _, Window, div, list, prelude::FluentBuilder as _, px,
};

pub(crate) use self::controls::ask_new_conversation;
use self::controls::{header, turn_row};
use self::entries::{Entry, EntryKey, Retry};
pub(crate) use self::global::{field_focus, scroll, view};
use self::rows::{Blocks, Meta, chip, earlier_row, entry_row};
pub(crate) use self::words::answering;
use self::words::{EMPTY_LINES, EXAMPLES, read_failed};
use crate::model::bots::{Bot, Chat, Read, reset_key, say_key};
use crate::panels::queue;
use crate::store::Store;
use crate::theme::{colours, radius};
use crate::ui::account_picker::reveals;
use crate::ui::counting::Counting;
use crate::ui::{self, Icon, press, text_field};

mod controls;
mod entries;
mod field;
mod global;
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

pub(crate) struct BotChat {
    store: Entity<Store>,
    list: ListState,
    bot: Option<String>,
    rows: Rc<Vec<Row>>,
    keys: Vec<EntryKey>,
    earlier: bool,
    opened: HashSet<u64>,
    news: bool,
    following: bool,
    blocks: Blocks,
    counting: Counting,
    mark: Option<Stamp>,
    heard: (Option<String>, u64, Controls),
    retry: Option<Rc<Retry>>,
    examples: [FocusHandle; EXAMPLES.len()],
    retry_stop: FocusHandle,
    empty: ScrollHandle,
    keyed: (Option<usize>, bool),
    due: Option<usize>,
}

fn counting(chat: &mut BotChat) -> &mut Counting {
    &mut chat.counting
}

pub(crate) type Stamp = (String, u64, u64, Option<u64>);

pub(crate) type Controls = (bool, bool, bool, bool, bool);

pub(crate) fn is_fresh(shown: Option<&str>, mark: Option<&Stamp>, stamp: &Stamp) -> bool {
    shown != Some(stamp.0.as_str()) || mark.is_some_and(|mark| (&mark.0, mark.1) != (&stamp.0, stamp.1))
}

pub(crate) fn example_to_reveal(now: Option<usize>, was: Option<usize>) -> Option<usize> {
    now.filter(|_| now != was)
}

pub(crate) fn re_anchor(following: bool, rows: usize) -> Option<usize> {
    following.then_some(rows)
}

fn at_end(list: &ListState) -> bool {
    -list.scroll_px_offset_for_scrollbar().y >= list.max_offset_for_scrollbar().height - px(2.)
}

impl BotChat {
    fn new(store: Entity<Store>, cx: &mut Context<Self>) -> Self {
        cx.observe(&store, |this, store, cx| {
            if this.moved(store.read(cx)) {
                cx.notify();
            }
        })
        .detach();
        let list = ListState::new(0, ListAlignment::Top, px(OVERDRAW));
        let chat = cx.entity().downgrade();
        list.set_scroll_handler(move |_, _, cx| {
            let chat = chat.clone();
            cx.defer(move |cx| {
                let _ = chat.update(cx, BotChat::reached_the_end);
            });
        });
        Self {
            store,
            list,
            bot: None,
            rows: Rc::new(Vec::new()),
            keys: Vec::new(),
            earlier: false,
            opened: HashSet::new(),
            news: false,
            following: true,
            blocks: Rc::new(RefCell::new(HashMap::new())),
            counting: Counting::default(),
            mark: None,
            heard: (None, 0, (false, false, false, false, false)),
            retry: None,
            examples: std::array::from_fn(|_| cx.focus_handle()),
            retry_stop: cx.focus_handle(),
            empty: ScrollHandle::new(),
            keyed: (None, false),
            due: None,
        }
    }

    fn moved(&mut self, store: &Store) -> bool {
        let shown = store.shown_bot();
        let controls = shown.map_or((false, false, false, false, false), |id| {
            let busy = (store.busy(&say_key(id)), store.busy(&reset_key(id)));
            (store.bots.stopping(id), busy.0, busy.1, store.chat_takes_keys(), store.bot_setup_offered())
        });
        let now = (shown.map(str::to_string), store.bots_revision(), controls);
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
        self.following = at_end(&self.list);
        if self.following && std::mem::take(&mut self.news) {
            cx.notify();
        }
    }

    fn scroll_to_end(&mut self, cx: &mut Context<Self>) {
        self.list.scroll_to(ListOffset { item_ix: self.rows.len(), offset_in_item: px(0.) });
        self.news = false;
        self.following = true;
        cx.notify();
    }

    fn scroll(&mut self, step: Scroll, cx: &mut Context<Self>) {
        let viewport = f32::from(self.list.viewport_bounds().size.height);
        self.list.scroll_by(px(scroll_distance(step, viewport)));
        self.following = at_end(&self.list);
        if self.following {
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
        let thinking = store.bots.find(&chat.bot).is_some_and(Bot::thinking);
        let retry = entries::retry(&chat.lines, thinking, store.bot_setup_offered());
        let stamp = (chat.bot.clone(), chat.born, chat.revision(), retry.as_ref().map(|retry| retry.error));
        let fresh = is_fresh(self.bot.as_deref(), self.mark.as_ref(), &stamp);
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
        self.retry = retry.map(Rc::new);
        let keys = entries::keys(&entries, &self.opened, self.retry.as_ref().map(|retry| retry.error));
        let rows: Vec<Row> =
            earlier.then_some(Row::Earlier).into_iter().chain(entries.into_iter().map(Row::Entry)).collect();
        self.apply(fresh, (earlier, keys), rows);
    }

    fn apply(&mut self, fresh: bool, (earlier, keys): (bool, Vec<EntryKey>), rows: Vec<Row>) {
        if fresh {
            self.list.reset(rows.len());
            self.list.scroll_to(ListOffset { item_ix: rows.len(), offset_in_item: px(0.) });
            self.news = false;
            self.following = true;
        } else if keys != self.keys || earlier != self.earlier {
            let following = self.following;
            for (range, count) in entries::row_splices((self.earlier, &self.keys), (earlier, &keys)) {
                self.list.splice(range, count);
            }
            if following {
                self.list.scroll_to(ListOffset { item_ix: rows.len(), offset_in_item: px(0.) });
            } else if keys.last() != self.keys.last() {
                self.news = true;
            }
        }
        self.keys = keys;
        self.earlier = earlier;
        self.rows = Rc::new(rows);
    }

    fn reveal_stops(&mut self, window: &Window) {
        let now =
            (self.examples.iter().position(|handle| handle.is_focused(window)), self.retry_stop.is_focused(window));
        self.due = example_to_reveal(now.0, self.keyed.0);
        if reveals(now.1, self.keyed.1, false) {
            self.following = true;
            self.news = false;
        }
        self.keyed = now;
    }

    fn meta(&self, bot: &Bot, cx: &mut Context<Self>) -> Meta {
        Meta {
            name: bot.name.clone().into(),
            now: queue::now(),
            chat: cx.entity().downgrade(),
            opened: Rc::new(self.opened.clone()),
            blocks: self.blocks.clone(),
            retry: self.retry.clone(),
            bot: bot.id.clone(),
            sending: self.store.read(cx).busy(&say_key(&bot.id)),
            stops: self.store.read(cx).chat_takes_keys(),
            retry_stop: self.retry_stop.clone(),
        }
    }

    fn body(&mut self, bot: &Bot, chat: &Chat, cx: &mut Context<Self>) -> AnyElement {
        match &chat.read {
            Read::Failed(why) if chat.lines.is_empty() => ui::note(read_failed(&bot.name, why)).into_any_element(),
            Read::Failed(why) => self.failed_above_lines(bot, why, cx),
            Read::Asking if chat.lines.is_empty() => ui::loading("Reading the chat…").into_any_element(),
            _ if chat.lines.is_empty() && !bot.thinking() => self.empty_page(bot, cx).into_any_element(),
            _ => self.lines(bot, cx),
        }
    }

    fn failed_above_lines(&mut self, bot: &Bot, why: &str, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_none()
                    .px(px(16.))
                    .py(px(4.))
                    .child(ui::error_line(read_failed(&bot.name, why)).text_size(px(11.5))),
            )
            .child(self.lines(bot, cx))
            .into_any_element()
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

impl BotChat {
    fn empty_page(&self, bot: &Bot, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let [what, when] = EMPTY_LINES;
        let stops = self.store.read(cx).chat_takes_keys();
        let examples = EXAMPLES
            .iter()
            .zip(&self.examples)
            .enumerate()
            .map(|(at, (text, handle))| example_row(&bot.id, (at, text), handle, stops));
        div().id("bot-empty").flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.empty).child(
            ui::emblem_page(Icon::Message, bot.name.clone())
                .child(ui::emblem_line(what))
                .child(ui::emblem_line(when))
                .child(self.examples_column().children(examples)),
        )
    }

    fn examples_column(&self) -> Div {
        let column = div().w_full().max_w(px(470.)).flex().flex_col().gap(px(8.));
        let Some(at) = self.due else { return column };
        let scroller = self.empty.clone();
        column.on_children_prepainted(move |bounds, window, _| reveal_example(&scroller, &bounds, at, window))
    }
}

fn reveal_example(scroller: &ScrollHandle, bounds: &[Bounds<Pixels>], at: usize, window: &mut Window) {
    if let Some(found) = bounds.get(at) {
        text_field::show_box(scroller, (found.top(), found.size.height), window);
    }
}

fn example_row(
    bot: &str,
    (at, text): (usize, &'static str),
    handle: &FocusHandle,
    stops: bool,
) -> impl IntoElement + use<> {
    let bot = bot.to_string();
    let name = format!("bot-example-{at}");
    let row = div()
        .id(("bot-example", at))
        .px(px(12.))
        .py(px(9.))
        .rounded(px(radius::card()))
        .bg(colours::panel())
        .border_1()
        .border_color(colours::border())
        .text_size(px(12.5))
        .text_color(colours::soft())
        .cursor_pointer()
        .hover(|s| s.bg(colours::hover()))
        .child(text);
    press::stop_at(row, handle, name.clone(), stops).on_click(move |event, window, cx| {
        if press::pressed(event, &name) {
            Store::get(cx).update(cx, |store, cx| store.add_example(&bot, text, cx));
            global::keys_to_field(window, cx);
        }
    })
}

impl Render for BotChat {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync(cx);
        self.reveal_stops(window);
        if let Some(item_ix) = re_anchor(self.following, self.rows.len()) {
            self.list.scroll_to(ListOffset { item_ix, offset_in_item: px(0.) });
        }
        let store = self.store.read(cx);
        let found = store.shown_bot().and_then(|id| store.bots.find(id).zip(store.bots.chat.as_ref()));
        let Some((bot, chat)) = found.map(|(bot, chat)| (bot.clone(), chat.clone())) else { return div() };
        let stopping = store.bots.stopping(&bot.id);
        let starting = store.busy(&reset_key(&bot.id));
        let stops = store.chat_takes_keys();
        let now = queue::now();
        if bot.thinking() {
            self.counting.drawn();
        }
        self.counting.keep(counting, cx);
        let body = self.body(&bot, &chat, cx);
        div().size_full().flex().flex_col().child(header(&bot, starting, stops)).child(body).children(turn_row(
            &bot,
            &chat,
            now,
            (stopping, stops),
        ))
    }
}

#[cfg(test)]
mod tests;
