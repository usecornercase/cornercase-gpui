use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui::{
    AnyElement, Bounds, Context, Div, Entity, FocusHandle, InteractiveElement as _, IntoElement, ListAlignment,
    ListOffset, ListState, ParentElement as _, Pixels, Render, ScrollHandle, StatefulInteractiveElement as _,
    Styled as _, Window, div, list, prelude::FluentBuilder as _, px,
};

pub(crate) use self::controls::ask_new_conversation;
use self::controls::{Fit, header, turn_row};
use self::entries::{Entry, EntryKey, Retry};
pub(crate) use self::global::{field_focus, scroll, view};
use self::rows::{Blocks, Meta, chip, earlier_row, entry_row};
use self::runs::offers_again;
pub(crate) use self::words::{EXAMPLES, answering, row_detail};
use self::words::{empty_lines, examples_shown, read_failed};
use crate::model::bots::Give as Giving;
use crate::model::bots::Reach;
use crate::model::bots::{Aim, Bot, Chat, Read, Runs, aim, consumed, reset_key, say_key};
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
mod run_block;
mod runs;
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
    heard: (Option<String>, u64, Controls, Option<Reach>),
    retry: Option<Rc<Retry>>,
    givable: Rc<HashMap<u64, u64>>,
    giving: Giving,
    examples: [FocusHandle; EXAMPLES.len()],
    retry_stop: FocusHandle,
    empty: ScrollHandle,
    keyed: (Option<usize>, bool, Option<u64>),
    due: Option<usize>,
    runs: Rc<Runs>,
    again_stops: HashMap<u64, FocusHandle>,
    aimed: u64,
    routines: (bool, f32),
    routines_stop: FocusHandle,
}

fn counting(chat: &mut BotChat) -> &mut Counting {
    &mut chat.counting
}

pub(crate) type Stamp = (String, u64, u64, Option<u64>, Vec<u64>, entries::Give);

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

fn asks_of(store: &Store, bot: &str) -> (Vec<u64>, Giving) {
    let waiting = store.bots.find(bot).map(|bot| bot.asks.iter().map(|ask| ask.id).collect()).unwrap_or_default();
    (waiting, store.asks_offer())
}

pub(crate) fn give_key(giving: &Giving) -> entries::Give {
    match giving {
        Giving::Offered => entries::Give::Button,
        Giving::Here(_) => entries::Give::Needs,
        Giving::No => entries::Give::No,
    }
}

pub(crate) const NAME_ROOM: f32 = 116.;
const GAP: f32 = 10.;
const SLACK: f32 = 8.;
const ROUTINES_WIDTH: f32 = 80.;

pub(crate) fn routines_fit(room: f32, drawn: bool, width: f32) -> bool {
    if drawn { room >= NAME_ROOM } else { room - width - GAP >= NAME_ROOM + SLACK }
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
            heard: (None, 0, (false, false, false, false, false), None),
            retry: None,
            givable: Rc::default(),
            giving: Giving::No,
            examples: std::array::from_fn(|_| cx.focus_handle()),
            retry_stop: cx.focus_handle(),
            empty: ScrollHandle::new(),
            keyed: (None, false, None),
            due: None,
            runs: Rc::new(Runs::default()),
            again_stops: HashMap::new(),
            aimed: 0,
            routines: (true, ROUTINES_WIDTH),
            routines_stop: cx.focus_handle(),
        }
    }

    fn moved(&mut self, store: &Store) -> bool {
        let shown = store.shown_bot();
        let controls = shown.map_or((false, false, false, false, false), |id| {
            let busy = (store.busy(&say_key(id)), store.busy(&reset_key(id)));
            (store.bots.stopping(id), busy.0, busy.1, store.chat_takes_keys(), store.bot_setup_offered())
        });
        let reach = shown.and_then(|id| store.bot_reach(id));
        let now = (shown.map(str::to_string), store.bots_revision(), controls, reach);
        (std::mem::replace(&mut self.heard, now.clone()) != now) || self.bot.is_some() && now.0.is_none()
    }

    fn set_routines_fit(&mut self, fits: bool, width: Option<f32>, cx: &mut Context<Self>) {
        if let Some(width) = width {
            self.routines.1 = width;
        }
        if self.routines.0 != fits {
            self.routines.0 = fits;
            cx.notify();
        }
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
        let (waiting, giving) = asks_of(store, &chat.bot);
        let give = give_key(&giving);
        let revision = store.bots_revision();
        let stamp = (chat.bot.clone(), chat.born, revision, retry.as_ref().map(|retry| retry.error), waiting, give);
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
        self.givable = Rc::new(entries::givable(&chat.lines, self.mark.as_ref().map_or(&[][..], |mark| &mark.4)));
        self.giving = giving;
        self.runs = Rc::new(store.bots.runs.clone());
        self.keep_again_stops(&entries, cx);
        let retried = self.retry.as_ref().map(|retry| retry.error);
        let keys = entries::keys(&entries, &self.opened, retried, (&self.givable, give), &self.runs);
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
            } else if entries::arrived(&self.keys, &keys) {
                self.news = true;
            }
        }
        self.keys = keys;
        self.earlier = earlier;
        self.rows = Rc::new(rows);
    }

    fn keep_again_stops(&mut self, entries: &[Entry], cx: &Context<Self>) {
        let offered: Vec<u64> = entries
            .iter()
            .filter_map(|entry| match entry {
                Entry::Run(block) if offers_again(block, &self.runs) => Some(block.run),
                _ => None,
            })
            .collect();
        self.again_stops.retain(|run, _| offered.contains(run));
        for run in offered {
            self.again_stops.entry(run).or_insert_with(|| cx.focus_handle());
        }
    }

    fn reveal_stops(&mut self, window: &Window) {
        let again = self.again_stops.iter().find(|(_, handle)| handle.is_focused(window)).map(|(run, _)| *run);
        let now = (
            self.examples.iter().position(|handle| handle.is_focused(window)),
            self.retry_stop.is_focused(window),
            again,
        );
        self.due = example_to_reveal(now.0, self.keyed.0);
        if reveals(now.1, self.keyed.1, false) {
            self.following = true;
            self.news = false;
        }
        if let Some(ix) = again.filter(|_| again != self.keyed.2).and_then(|run| self.row_of(run)) {
            self.list.scroll_to_reveal_item(ix);
            self.following = false;
        }
        self.keyed = now;
    }

    fn row_runs(&self) -> Vec<Option<u64>> {
        self.rows
            .iter()
            .map(|row| match row {
                Row::Entry(entry) => entry.run(),
                Row::Earlier => None,
            })
            .collect()
    }

    fn row_of(&self, run: u64) -> Option<usize> {
        self.row_runs().iter().position(|found| *found == Some(run))
    }

    fn take_aim(&mut self, cx: &Context<Self>) {
        let store = self.store.read(cx);
        let Some(chat) = store.bots.chat.as_ref().filter(|chat| Some(chat.bot.as_str()) == self.bot.as_deref()) else {
            return;
        };
        let anchor = store.bots.anchor.clone().filter(|anchor| anchor.bot == chat.bot);
        let read = chat.read == Read::Done;
        let aimed = aim(&self.row_runs(), anchor.as_ref(), self.aimed, read);
        if let Aim::Go(item_ix) = aimed {
            self.list.scroll_to(ListOffset { item_ix, offset_in_item: px(0.) });
            self.following = false;
            self.news = false;
        }
        if let Some(anchor) = anchor.filter(|_| consumed(aimed, read)) {
            self.aimed = anchor.serial;
        }
    }

    fn fit(&self, routines: bool, cx: &Context<Self>) -> Option<Fit> {
        let chat = cx.entity().downgrade();
        let (drawn, width) = self.routines;
        routines.then(|| Fit { chat, drawn, width, stop: self.routines_stop.clone() })
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
            givable: self.givable.clone(),
            giving: self.giving.clone(),
            runs: self.runs.clone(),
            again_busy: Rc::new(self.store.read(cx).bots.again_busy().clone()),
            again_stops: Rc::new(self.again_stops.clone()),
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
        let routines = self.store.read(cx).routines_offered();
        let [what, when] = empty_lines(routines);
        let stops = self.store.read(cx).chat_takes_keys();
        let examples = EXAMPLES
            .iter()
            .zip(&self.examples)
            .take(examples_shown(routines))
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
        self.take_aim(cx);
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
        let reach = store.bot_reach(&bot.id);
        let routines = store.routines_offered();
        let now = queue::now();
        if bot.thinking() {
            self.counting.drawn();
        }
        self.counting.keep(counting, cx);
        let body = self.body(&bot, &chat, cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(header(&bot, starting, (stops, self.fit(routines, cx)), reach.as_ref()))
            .child(body)
            .children(turn_row(&bot, &chat, now, (stopping, stops)))
    }
}

#[cfg(test)]
mod tests;
