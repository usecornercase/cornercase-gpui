use std::collections::HashSet;
use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, DragMoveEvent, ElementId, InteractiveElement as _, IntoElement,
    ListAlignment, ListOffset, ListState, ParentElement as _, SharedString, Stateful, StatefulInteractiveElement as _,
    Styled as _, WeakEntity, Window, div, list, prelude::FluentBuilder as _, px,
};

use super::Browser;
use super::console::{Filter, Level, Log, Logged, hand_off};
use crate::store::{Store, ToastKind};
use crate::theme::{FONT_FAMILY, colours, radius};
use crate::ui::split::{self, Edge, Split};
use crate::ui::{self, Icon, glass, icon};

const HEIGHT: Split = Split::new(200., 90., 640.);
const KEEP: f32 = 160.;
const OVERDRAW: f32 = 200.;

#[derive(Clone)]
pub(crate) struct ConsoleGrip;

pub(crate) struct Drawer {
    pub(crate) open: bool,
    filter: Filter,
    height: Split,
    list: ListState,
    shown: Vec<u64>,
    shown_for: Option<(String, Filter)>,
    expanded: HashSet<u64>,
}

impl Default for Drawer {
    fn default() -> Self {
        Self {
            open: false,
            filter: Filter::default(),
            height: HEIGHT,
            list: ListState::new(0, ListAlignment::Top, px(OVERDRAW)),
            shown: Vec::new(),
            shown_for: None,
            expanded: HashSet::new(),
        }
    }
}

pub(crate) fn changes(old: &[u64], new: &[u64]) -> Option<(usize, usize)> {
    let dropped = new.first().map_or(old.len(), |first| old.iter().take_while(|id| *id < first).count());
    let kept = &old[dropped..];
    (new.len() >= kept.len() && new[..kept.len()] == *kept).then(|| (dropped, new.len() - kept.len()))
}

fn trailing_number(text: &str) -> Option<(&str, &str)> {
    let (head, tail) = text.rsplit_once(':')?;
    (!tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit())).then_some((head, tail))
}

pub(crate) fn short_source(source: &str) -> String {
    let (rest, line) = match trailing_number(source) {
        Some((head, last)) => trailing_number(head).map_or((head, last), |(head, line)| (head, line)),
        None => (source, ""),
    };
    let path = rest.split(['?', '#']).next().unwrap_or(rest).trim_end_matches('/');
    let file = path.rsplit('/').next().unwrap_or(path);
    if line.is_empty() { file.to_string() } else { format!("{file}:{line}") }
}

fn at_end(list: &ListState) -> bool {
    -list.scroll_px_offset_for_scrollbar().y >= list.max_offset_for_scrollbar().height - px(2.)
}

fn drawer_height(browser: &mut Browser) -> &mut Split {
    &mut browser.notices.drawer.height
}

pub(crate) fn follow_drawer(
    cx: &Context<Browser>,
) -> impl Fn(&DragMoveEvent<ConsoleGrip>, &mut Window, &mut App) + 'static {
    split::follow::<_, ConsoleGrip>(Edge::Top, KEEP, drawer_height, cx)
}

fn shown_ids(log: Option<&Log>, filter: Filter) -> Vec<u64> {
    log.map(|log| log.entries().filter(|l| filter.shows(l.entry.level)).map(|l| l.id).collect()).unwrap_or_default()
}

impl Browser {
    fn sync_console(&mut self) {
        let target = self.tabs.active_target(self.workspace).unwrap_or_default();
        let filter = self.notices.drawer.filter;
        let ids = shown_ids(self.notices.logs.get(&target), filter);
        let drawer = &mut self.notices.drawer;
        let key = Some((target, filter));
        let following = match changes(&drawer.shown, &ids).filter(|_| drawer.shown_for == key) {
            Some((0, 0)) => return,
            Some((dropped, added)) => {
                let following = at_end(&drawer.list);
                drawer.list.splice(0..dropped, 0);
                let end = drawer.shown.len() - dropped;
                drawer.list.splice(end..end, added);
                following
            }
            None => {
                drawer.list.reset(ids.len());
                true
            }
        };
        if following {
            drawer.list.scroll_to(ListOffset { item_ix: ids.len(), offset_in_item: px(0.) });
        }
        drawer.shown = ids;
        drawer.shown_for = key;
    }

    fn console_rows(&self) -> Rc<Vec<Logged>> {
        let filter = self.notices.drawer.filter;
        let rows =
            self.active_log().map(|log| log.entries().filter(|l| filter.shows(l.entry.level)).cloned().collect());
        Rc::new(rows.unwrap_or_default())
    }

    pub(super) fn console_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let errors = self.active_log().map_or(0, Log::errors);
        ui::tool_button("browser-console", Icon::Console, 26., self.notices.drawer.open)
            .relative()
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.notices.drawer.open = !this.notices.drawer.open;
                cx.notify();
            }))
            .when(errors > 0, |b| b.child(count_badge(errors)))
    }

    pub(super) fn console_drawer(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        if !self.notices.drawer.open {
            return None;
        }
        self.sync_console();
        let height = self.notices.drawer.height.size();
        Some(
            div()
                .relative()
                .flex_none()
                .h(px(height))
                .flex()
                .flex_col()
                .border_t_1()
                .border_color(colours::border())
                .child(self.drawer_header(cx))
                .child(self.drawer_body(window, cx))
                .child(split::handle("browser-console-split", ConsoleGrip, (Edge::Bottom, 0.), drawer_height, cx)),
        )
    }

    fn drawer_header(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (errors, warnings) = self.active_log().map_or((0, 0), |l| (l.errors(), l.warnings()));
        let all = self.active_log().map_or(0, Log::len);
        let filter = self.notices.drawer.filter;
        let segments =
            [(Filter::Errors, "Errors", errors), (Filter::Warnings, "Warnings", warnings), (Filter::All, "All", all)]
                .into_iter()
                .map(|(f, label, n)| filter_segment(f, label, n, f == filter, cx))
                .collect();
        div()
            .flex_none()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(4.))
            .px(px(8.))
            .py(px(5.))
            .child(ui::segments(segments))
            .child(div().flex_1())
            .when(errors > 0, |d| d.child(send_errors_button(cx)))
            .child(header_button("browser-console-clear", Icon::Trash, cx, Browser::clear_console))
            .child(header_button("browser-console-close", Icon::Close, cx, |this, cx| {
                this.notices.drawer.open = false;
                cx.notify();
            }))
    }

    fn drawer_body(&self, _window: &Window, cx: &mut Context<Self>) -> AnyElement {
        if self.notices.drawer.shown.is_empty() {
            let text = empty_text(self.notices.drawer.filter);
            return ui::note(text).text_size(px(12.)).into_any_element();
        }
        let rows = self.console_rows();
        let expanded = Rc::new(self.notices.drawer.expanded.clone());
        let browser = cx.entity().downgrade();
        list(self.notices.drawer.list.clone(), move |ix, _, _| {
            rows.get(ix).map_or_else(|| div().into_any_element(), |l| entry_row(l, expanded.contains(&l.id), &browser))
        })
        .flex_1()
        .into_any_element()
    }

    fn clear_console(&mut self, cx: &mut Context<Self>) {
        let target = self.tabs.active_target(self.workspace).unwrap_or_default();
        if let Some(log) = self.notices.logs.get_mut(&target) {
            log.clear();
        }
        self.notices.drawer.expanded.clear();
        cx.notify();
    }

    fn toggle_entry(&mut self, id: u64, cx: &mut Context<Self>) {
        let drawer = &mut self.notices.drawer;
        if !drawer.expanded.remove(&id) {
            drawer.expanded.insert(id);
        }
        if let Some(at) = drawer.shown.iter().position(|shown| *shown == id) {
            drawer.list.splice(at..at + 1, 1);
        }
        cx.notify();
    }

    fn send_console(&mut self, only: Option<u64>, cx: &mut Context<Self>) {
        let Some(log) = self.active_log() else { return };
        let picked: Vec<&Logged> = match only {
            Some(id) => log.entries().filter(|l| l.id == id).collect(),
            None => log.entries().filter(|l| l.entry.level == Level::Error).collect(),
        };
        if picked.is_empty() {
            return;
        }
        let url = self.active_url().unwrap_or_default();
        let text = hand_off(&url, &picked);
        let what = if only.is_some() { "the entry".to_string() } else { format!("{} errors", picked.len()) };
        hand_to_agent(self.target_agent(cx), &text, &what, cx);
    }
}

fn hand_to_agent(pane: Option<u64>, text: &str, what: &str, cx: &mut Context<Browser>) {
    let Some(pane) = pane else {
        let text = "There is no agent in this workspace to send the console to";
        Store::get(cx).update(cx, |s, cx| s.toast(text, ToastKind::Error, cx));
        return;
    };
    if Browser::give_to_agent(pane, text, &[], cx) {
        let text = format!("Sent {what} to {}", Browser::agent_name(pane, cx));
        Store::get(cx).update(cx, |s, cx| s.toast(text, ToastKind::Info, cx));
    }
}

fn empty_text(filter: Filter) -> &'static str {
    match filter {
        Filter::Errors => "No errors",
        Filter::Warnings => "No warnings",
        Filter::All => "Nothing logged on this page",
    }
}

fn count_badge(n: usize) -> impl IntoElement {
    let label = if n > 99 { "99+".to_string() } else { n.to_string() };
    div()
        .absolute()
        .top(px(-3.))
        .right(px(-5.))
        .h(px(14.))
        .min_w(px(14.))
        .px(px(3.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(colours::failed())
        .text_size(px(9.))
        .text_color(colours::text())
        .child(label)
}

fn filter_segment(filter: Filter, label: &str, count: usize, selected: bool, cx: &mut Context<Browser>) -> AnyElement {
    let text = if count > 0 { format!("{label} {count}") } else { label.to_string() };
    ui::segment(SharedString::from(format!("browser-console-{label}")), text, selected)
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.notices.drawer.filter = filter;
            cx.notify();
        }))
        .into_any_element()
}

fn header_button(
    id: &'static str,
    glyph: Icon,
    cx: &mut Context<Browser>,
    act: fn(&mut Browser, &mut Context<Browser>),
) -> impl IntoElement + use<> {
    ui::tool_button(id, glyph, 24., false).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| act(this, cx)))
}

fn send_errors_button(cx: &mut Context<Browser>) -> impl IntoElement + use<> {
    ui::icon_text_button("browser-console-send", Icon::Send, "Send errors", ui::Tone::Plain)
        .py(px(2.))
        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.send_console(None, cx)))
}

fn level_mark(level: Level) -> Div {
    let slot = div().w(px(14.)).h(px(16.)).flex_none().flex().items_center().justify_center();
    match level {
        Level::Error => slot.child(icon(Icon::Failed, 11.).text_color(colours::failed())),
        Level::Warning => slot.child(icon(Icon::Alert, 11.).text_color(colours::waiting())),
        Level::Info => slot,
    }
}

fn row_base(logged: &Logged) -> Stateful<Div> {
    let tint = match logged.entry.level {
        Level::Error => glass::tint(colours::failed(), 0.07),
        Level::Warning => glass::tint(colours::waiting(), 0.06),
        Level::Info => glass::tint(colours::text(), 0.),
    };
    div()
        .id(ElementId::from(("browser-console-entry", logged.id)))
        .w_full()
        .px(px(8.))
        .py(px(3.))
        .flex()
        .flex_col()
        .gap(px(2.))
        .bg(tint)
        .border_b_1()
        .border_color(colours::border())
        .font_family(FONT_FAMILY)
        .text_size(px(11.5))
        .cursor_pointer()
        .hover(|s| s.bg(colours::hover()))
}

fn entry_row(logged: &Logged, open: bool, browser: &WeakEntity<Browser>) -> AnyElement {
    let (id, toggle, send) = (logged.id, browser.clone(), browser.clone());
    row_base(logged)
        .on_click(move |_, _, cx| {
            let _ = toggle.update(cx, |this, cx| this.toggle_entry(id, cx));
        })
        .child(summary(logged, open, send))
        .when(open, |d| d.child(details(&logged.entry)))
        .into_any_element()
}

fn summary(logged: &Logged, open: bool, browser: WeakEntity<Browser>) -> impl IntoElement {
    let entry = &logged.entry;
    let colour = match entry.level {
        Level::Error => colours::failed(),
        Level::Warning => colours::waiting(),
        Level::Info => colours::soft(),
    };
    let text = if open {
        div().flex_1().min_w_0().child(entry.text.clone()).into_any_element()
    } else {
        div().flex_1().min_w_0().child(ui::one_line(entry.text.clone())).into_any_element()
    };
    div()
        .w_full()
        .min_w_0()
        .flex()
        .items_start()
        .gap(px(6.))
        .text_color(colour)
        .child(level_mark(entry.level))
        .child(text)
        .when(logged.times > 1, |d| d.child(times_chip(logged.times)))
        .children(entry.source.as_deref().map(|s| ui::dim(short_source(s)).flex_none()))
        .child(send_button(logged.id, browser))
}

fn times_chip(times: u32) -> impl IntoElement {
    div()
        .flex_none()
        .px(px(5.))
        .rounded(px(radius::CHIP))
        .bg(colours::selected())
        .text_size(px(10.))
        .text_color(colours::muted())
        .child(format!("×{times}"))
}

fn send_button(id: u64, browser: WeakEntity<Browser>) -> impl IntoElement {
    let group = SharedString::from(format!("browser-console-send-{id}"));
    let glyph = icon(Icon::Send, 11.)
        .text_color(colours::faint())
        .group_hover(group.clone(), |s| s.text_color(colours::text()));
    div()
        .id(ElementId::from(("browser-console-send-entry", id)))
        .group(group)
        .flex_none()
        .p(px(2.))
        .rounded(px(radius::CHIP))
        .hover(|s| s.bg(colours::selected()))
        .child(glyph)
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            let _ = browser.update(cx, |this, cx| this.send_console(Some(id), cx));
        })
}

fn details(entry: &super::console::Entry) -> impl IntoElement {
    let lines: Vec<String> = match (&entry.source, entry.stack.is_empty()) {
        (Some(source), true) => vec![format!("at {source}")],
        _ => entry.stack.clone(),
    };
    div()
        .pl(px(20.))
        .flex()
        .flex_col()
        .text_size(px(11.))
        .text_color(colours::muted())
        .children(lines.into_iter().map(|line| div().child(line)))
}

#[cfg(test)]
mod tests;
