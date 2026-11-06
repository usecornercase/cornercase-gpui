use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use gpui::{
    AnyElement, AppContext as _, ClickEvent, Context, Entity, Focusable as _, HighlightStyle, InteractiveElement as _,
    IntoElement, KeyDownEvent, ParentElement as _, ScrollStrategy, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, div, point, px, relative,
};
use grep_matcher::Matcher as _;
use grep_regex::RegexMatcher;

use crate::theme::{colours, radius};
use crate::ui::selection::{Styles, overlay};
use crate::ui::{self, TextField, TextFieldEvent};

use super::disk::Content;
use super::style::lit_background;
use super::{FilesPanel, Viewer, search};

const FIELD_TOP: f32 = 8.;
const FIELD_HEIGHT: f32 = 48.;
pub(super) const FIELD_ROOM: f32 = FIELD_TOP + FIELD_HEIGHT;
const DRAWN_BYTES: u32 = 4096;
const BUSY_AFTER: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Hit {
    line: u32,
    start: u32,
    end: u32,
}

fn small(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

impl Hit {
    fn new(line: usize, range: Range<usize>) -> Self {
        Self { line: small(line), start: small(range.start), end: small(range.end) }
    }

    fn range_in(self, line: &str) -> Option<Range<usize>> {
        let (start, end) = (self.start as usize, self.end as usize);
        (start < end && line.is_char_boundary(start) && line.is_char_boundary(end)).then_some(start..end)
    }
}

pub(super) fn all_hits(matcher: &RegexMatcher, lines: &[String], cancel: &AtomicBool) -> Option<Vec<Hit>> {
    let mut hits = Vec::new();
    for (ix, line) in lines.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let _ = matcher.find_iter(line.as_bytes(), |m| {
            hits.push(Hit::new(ix, m.start()..m.end()));
            true
        });
    }
    Some(hits)
}

pub(super) fn first_from(hits: &[Hit], line: usize) -> Option<usize> {
    if hits.is_empty() {
        return None;
    }
    let at = hits.partition_point(|hit| hit.line < small(line));
    Some(if at == hits.len() { 0 } else { at })
}

pub(super) fn stepped(current: Option<usize>, len: usize, back: bool) -> Option<usize> {
    let last = len.checked_sub(1)?;
    Some(match (current, back) {
        (None, false) => 0,
        (None, true) => last,
        (Some(at), false) => {
            if at >= last {
                0
            } else {
                at + 1
            }
        }
        (Some(at), true) => at.checked_sub(1).unwrap_or(last),
    })
}

pub(super) fn on_line(hits: &[Hit], line: usize) -> &[Hit] {
    let line = small(line);
    let from = hits.partition_point(|hit| hit.line < line);
    let shown = hits[from..].partition_point(|hit| hit.line == line && hit.start < DRAWN_BYTES);
    &hits[from..from + shown]
}

pub(super) fn count(current: Option<usize>, len: usize) -> String {
    match current {
        Some(at) if len > 0 => format!("{} of {len}", at + 1),
        _ => "No matches".to_string(),
    }
}

fn other_hits() -> HighlightStyle {
    HighlightStyle { background_color: Some(lit_background(0.3)), ..HighlightStyle::default() }
}

fn current_hit() -> HighlightStyle {
    HighlightStyle {
        background_color: Some(colours::brand().into()),
        color: Some(colours::on_brand().into()),
        ..HighlightStyle::default()
    }
}

pub(super) struct Find {
    pub(super) field: Entity<TextField>,
    hits: Arc<Vec<Hit>>,
    current: Option<usize>,
    running: Option<Arc<AtomicBool>>,
    slow: bool,
    answered: bool,
    generation: u64,
    _events: Subscription,
}

impl Find {
    pub(super) fn new(query: &str, cx: &mut Context<FilesPanel>) -> Self {
        let field = cx.new(|cx| {
            let mut field = TextField::new("Find in this file…", cx).clears_on_escape();
            field.set_text(query.to_string(), cx);
            field
        });
        let events = cx.subscribe(&field, |this, _, event: &TextFieldEvent, cx| match event {
            TextFieldEvent::Changed => this.find_changed(cx),
            TextFieldEvent::Submit => this.find_step(false, cx),
            TextFieldEvent::Cancel => {}
        });
        Self {
            field,
            hits: Arc::default(),
            current: None,
            running: None,
            slow: false,
            answered: false,
            generation: 0,
            _events: events,
        }
    }

    fn stop(&mut self) {
        if let Some(running) = self.running.take() {
            running.store(true, Ordering::Relaxed);
        }
        self.generation += 1;
        self.slow = false;
    }

    fn clear(&mut self) {
        self.hits = Arc::default();
        self.current = None;
        self.answered = false;
    }

    fn answer(&mut self, hits: Vec<Hit>, start: usize) {
        self.current = first_from(&hits, start);
        self.hits = Arc::new(hits);
        self.running = None;
        self.slow = false;
        self.answered = true;
    }

    fn current_line(&self) -> Option<usize> {
        self.hits.get(self.current?).map(|hit| hit.line as usize)
    }

    pub(super) fn paint_line(&self, styles: Styles, ix: usize, line: &str) -> Styles {
        let lit = on_line(&self.hits, ix);
        if lit.is_empty() {
            return styles;
        }
        let current = self.current.and_then(|at| self.hits.get(at)).copied();
        let others: Vec<Range<usize>> =
            lit.iter().filter(|hit| Some(**hit) != current).filter_map(|hit| hit.range_in(line)).collect();
        let styles = overlay(styles, &others, other_hits());
        match current.filter(|hit| lit.contains(hit)).and_then(|hit| hit.range_in(line)) {
            Some(range) => overlay(styles, &[range], current_hit()),
            None => styles,
        }
    }
}

struct Asked {
    place: u64,
    path: PathBuf,
    generation: u64,
    start: usize,
    jump: bool,
}

fn top_line(viewer: &Viewer) -> usize {
    viewer.selecting.laid().drawn_ends().map_or(0, |(first, _)| first)
}

fn selected_chars(viewer: &Viewer) -> Option<String> {
    let (line, range) = viewer.selecting.selection?.one_line_chars()?;
    let Some(Content::Text(text)) = &viewer.content else { return None };
    text.lines.get(line)?.get(range).map(str::to_string)
}

impl FilesPanel {
    fn viewer_mut(&mut self) -> Option<&mut Viewer> {
        self.place_mut()?.viewer.as_mut()
    }

    pub(crate) fn can_find(&self) -> bool {
        matches!(self.place().and_then(|p| p.viewer.as_ref()).and_then(|v| v.content.as_ref()), Some(Content::Text(_)))
    }

    pub(crate) fn open_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(viewer) = self.viewer_mut() else { return };
        let fresh = viewer.find.is_none();
        let seed = selected_chars(viewer).unwrap_or_default();
        let field = viewer.find.get_or_insert_with(|| Find::new(&seed, cx)).field.clone();
        window.focus(&field.read(cx).focus_handle(cx));
        if fresh {
            self.keep_view(true);
            self.find_changed(cx);
        }
        cx.notify();
    }

    fn keep_view(&self, field_opened: bool) {
        let handle = self.code_scroll.0.borrow().base_handle.clone();
        let offset = handle.offset();
        if offset.y < px(0.) {
            let by = px(if field_opened { -FIELD_ROOM } else { FIELD_ROOM });
            handle.set_offset(point(offset.x, (offset.y + by).min(px(0.))));
        }
    }

    pub(super) fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(viewer) = self.viewer_mut() {
            viewer.find = None;
        }
        self.keep_view(false);
        window.focus(&self.focus);
        cx.notify();
    }

    fn find_changed(&mut self, cx: &mut Context<Self>) {
        let Some(place) = self.current else { return };
        let start = self.viewer_mut().map_or(0, |viewer| top_line(viewer));
        self.run_find(place, start, true, cx);
    }

    pub(super) fn find_again(&mut self, place: u64, cx: &mut Context<Self>) {
        let Some(viewer) = self.places.get(&place).and_then(|p| p.viewer.as_ref()) else { return };
        let Some(find) = viewer.find.as_ref() else { return };
        let start = find
            .current_line()
            .or_else(|| viewer.selecting.selection.map(|selection| selection.ends().0))
            .unwrap_or_else(|| top_line(viewer));
        let jump = !find.answered;
        self.run_find(place, start, jump, cx);
    }

    fn run_find(&mut self, place: u64, start: usize, jump: bool, cx: &mut Context<Self>) {
        let Some(viewer) = self.places.get_mut(&place).and_then(|p| p.viewer.as_mut()) else { return };
        let Some(Content::Text(text)) = &viewer.content else { return };
        let (lines, path) = (text.lines.clone(), viewer.path.clone());
        let Some(find) = viewer.find.as_mut() else { return };
        find.stop();
        let Some(matcher) = search::matcher(&find.field.read(cx).text()) else {
            find.clear();
            cx.notify();
            return;
        };
        let cancel = Arc::new(AtomicBool::new(false));
        find.running = Some(cancel.clone());
        let asked = Asked { place, path, generation: find.generation, start, jump };
        Self::show_slow(place, asked.generation, cx);
        let task = cx.background_spawn(async move { all_hits(&matcher, &lines, &cancel) });
        cx.spawn(async move |this, cx| {
            let hits = task.await;
            let _ = this.update(cx, |this, cx| this.found(&asked, hits, cx));
        })
        .detach();
    }

    fn show_slow(place: u64, generation: u64, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(BUSY_AFTER).await;
            let _ = this.update(cx, |this, cx| this.find_is_slow(place, generation, cx));
        })
        .detach();
    }

    fn find_is_slow(&mut self, place: u64, generation: u64, cx: &mut Context<Self>) {
        let viewer = self.places.get_mut(&place).and_then(|p| p.viewer.as_mut());
        let find = viewer.and_then(|viewer| viewer.find.as_mut());
        if let Some(find) = find.filter(|find| find.generation == generation && find.running.is_some()) {
            find.slow = true;
            cx.notify();
        }
    }

    fn found(&mut self, asked: &Asked, hits: Option<Vec<Hit>>, cx: &mut Context<Self>) {
        let Some(viewer) = self.places.get_mut(&asked.place).and_then(|p| p.viewer.as_mut()) else { return };
        let Some(find) = viewer.find.as_mut() else { return };
        let (Some(hits), true, true) = (hits, viewer.path == asked.path, find.generation == asked.generation) else {
            return;
        };
        find.answer(hits, asked.start);
        let line = find.current_line().filter(|_| asked.jump && self.current == Some(asked.place));
        if let Some(line) = line {
            self.show_line(line);
        }
        cx.notify();
    }

    fn show_line(&self, line: usize) {
        self.code_scroll.scroll_to_item(line, ScrollStrategy::Center);
    }

    pub(super) fn find_step(&mut self, back: bool, cx: &mut Context<Self>) {
        let Some(find) = self.viewer_mut().and_then(|viewer| viewer.find.as_mut()) else { return };
        find.current = stepped(find.current, find.hits.len(), back);
        if let Some(line) = find.current_line() {
            self.show_line(line);
        }
        cx.notify();
    }

    fn find_key(&mut self, e: &KeyDownEvent, cx: &mut Context<Self>) {
        let back = match (e.keystroke.key.as_str(), e.keystroke.modifiers.shift) {
            ("enter", true) | ("up", false) => true,
            ("down", false) => false,
            _ => return,
        };
        self.find_step(back, cx);
        cx.stop_propagation();
    }

    pub(crate) fn escape(&mut self, cx: &mut Context<Self>) -> bool {
        if self.menu.take().is_some() {
            cx.notify();
            return true;
        }
        let Some(viewer) = self.viewer_mut() else { return false };
        let field_closed = viewer.find.take().is_some();
        let gone = field_closed || viewer.selecting.selection.take().is_some();
        if field_closed {
            self.keep_view(false);
        }
        if gone {
            cx.notify();
        }
        gone
    }

    fn find_count(find: &Find, blank: bool) -> impl IntoElement + use<> {
        let label = if blank || (find.running.is_some() && !find.answered) {
            String::new()
        } else {
            count(find.current, find.hits.len())
        };
        if find.slow {
            ui::busy("files-find-busy", "").into_any_element()
        } else {
            ui::dim(ui::one_line(label)).flex_none().into_any_element()
        }
    }

    fn find_buttons(cx: &mut Context<Self>) -> [AnyElement; 3] {
        let step = |id: &'static str, glyph: &'static str, back: bool, cx: &mut Context<Self>| {
            ui::icon_button(id, glyph)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.find_step(back, cx)))
                .into_any_element()
        };
        [
            step("files-find-previous", "↑", true, cx),
            step("files-find-next", "↓", false, cx),
            ui::icon_button("files-find-close", "✕")
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.close_find(window, cx)))
                .into_any_element(),
        ]
    }

    pub(super) fn find_bar(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let viewer = self.place()?.viewer.as_ref()?;
        let find = viewer.find.as_ref().filter(|_| self.can_find())?;
        let focused = find.field.read(cx).focus_handle(cx).is_focused(window);
        let blank = find.field.read(cx).text().trim().is_empty();
        let card = ui::glass::card(radius::CARD)
            .flex()
            .items_center()
            .gap(px(4.))
            .p(px(6.))
            .h(px(FIELD_HEIGHT))
            .child(div().flex_1().min_w_0().child(ui::search_field(find.field.clone(), focused, None)))
            .child(div().flex_none().px(px(4.)).child(Self::find_count(find, blank)))
            .children(Self::find_buttons(cx));
        let bar = div()
            .absolute()
            .top(px(FIELD_TOP))
            .right(px(12.))
            .w(px(360.))
            .max_w(relative(0.94))
            .occlude()
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| this.find_key(e, cx)))
            .child(card);
        Some(bar.into_any_element())
    }
}

#[cfg(test)]
mod tests;
