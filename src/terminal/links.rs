use gpui::{AppContext as _, Bounds, Context, Modifiers, MouseButton, MouseDownEvent, Pixels, Point, Window};

use super::selection::{class, column, word_at};
use crate::app::{Drag, Root};
use crate::cli;
use crate::model::link::{Hovered, Span, Target, is_loopback, row_cells, text_between, url_in, web_hyperlink};
use crate::model::{Link, MouseMode, PaneGrid};
use crate::panels::PanelKind;
use crate::panels::files::{self, Anchor, Peek, WebPage};
use crate::render::layout::Hit;
use crate::store::args;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Spot {
    pub pane: u64,
    pub row: u16,
    pub start: u16,
    pub end: u16,
    pub text: String,
}

#[derive(Default)]
pub(crate) struct Pointer {
    pub(super) links: Links,
    pub(super) scroll_rest: f32,
}

#[derive(Default)]
pub(super) struct Links {
    spot: Option<Spot>,
    cell: (u16, u16),
    answer: Option<Hovered>,
    on: bool,
    asking: bool,
    next: Option<(Spot, (u16, u16))>,
}

impl Links {
    pub(super) fn hovered(&self) -> Option<&Hovered> {
        self.answer.as_ref().filter(|_| self.on)
    }
}

pub(super) fn link_args(pane: u64, (x, y): (u16, u16)) -> Vec<String> {
    args(&["link", "--pane", &pane.to_string(), "--cell", &format!("{x},{y}"), "--json"])
}

pub(super) fn spot_at(pane: &PaneGrid, cell: (u16, u16)) -> Option<Spot> {
    hyperlink_spot(pane, cell).or_else(|| word_spot(pane, cell))
}

fn hyperlink_spot(pane: &PaneGrid, cell: (u16, u16)) -> Option<Spot> {
    let (span, _) = web_hyperlink(pane, cell)?;
    let text = text_between(&row_cells(pane, span.row), usize::from(span.start), usize::from(span.end));
    Some(Spot { pane: pane.pane, row: span.row, start: span.start, end: span.end, text })
}

fn word_spot(pane: &PaneGrid, (x, y): (u16, u16)) -> Option<Spot> {
    let cells = row_cells(pane, y);
    let at = usize::from(x);
    if cells.get(at).is_none_or(|cell| class(cell) != 1) {
        return None;
    }
    let (start, end) = word_at(&cells, at);
    let text = text_between(&cells, start, end + 1);
    Some(Spot { pane: pane.pane, row: y, start: column(start), end: column(end + 1), text })
}

pub(super) fn web_under(pane: &PaneGrid, spot: &Spot) -> Option<(Span, String)> {
    if let Some(found) = web_hyperlink(pane, (spot.start, spot.row)) {
        return Some(found);
    }
    let cells = row_cells(pane, spot.row);
    let (from, to) = url_in(&cells, usize::from(spot.start), usize::from(spot.end))?;
    Some((Span { row: spot.row, start: column(from), end: column(to) }, text_between(&cells, from, to)))
}

pub(super) fn web_at(pane: &PaneGrid, cell: (u16, u16)) -> Option<String> {
    let (span, url) = spot_at(pane, cell).and_then(|spot| web_under(pane, &spot))?;
    span.covers(cell).then_some(url)
}

pub(super) fn inside(pane: &PaneGrid, (x, y): (i32, i32)) -> Option<(u16, u16)> {
    let x = u16::try_from(x - i32::from(pane.rect.x)).ok().filter(|x| *x < pane.size.cols)?;
    let y = u16::try_from(y - i32::from(pane.rect.y)).ok().filter(|y| *y < pane.size.rows)?;
    Some((x, y))
}

fn plain_click(event: &MouseDownEvent) -> bool {
    event.button == MouseButton::Left && event.click_count == 1 && event.modifiers == Modifiers::default()
}

impl Root {
    fn spot_under(&self, at: Point<Pixels>) -> Option<(Spot, (u16, u16))> {
        let Some(Hit::Pane(id, raw)) = self.hit(at) else { return None };
        let pane = self.grid.pane(id)?;
        let cell = inside(pane, raw)?;
        Some((spot_at(pane, cell)?, cell))
    }

    pub(super) fn hover_link(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let found = if self.drag.is_none() && self.frame_shown() { self.spot_under(at) } else { None };
        let Some((spot, cell)) = found else {
            self.forget_link(cx);
            return;
        };
        self.pointer.links.cell = cell;
        if self.pointer.links.spot.as_ref() != Some(&spot) {
            self.enter_spot(spot, cell, window, cx);
        }
        self.settle_link(window, cx);
    }

    fn enter_spot(&mut self, spot: Spot, cell: (u16, u16), window: &mut Window, cx: &mut Context<Self>) {
        self.pointer.links.spot = Some(spot.clone());
        self.pointer.links.answer = None;
        let Some(pane) = self.grid.pane(spot.pane) else { return };
        match web_under(pane, &spot) {
            Some((span, url)) => {
                let hovered = Hovered::new(pane, span, Target::Web(url));
                self.settle_link(window, cx);
                self.pointer.links.answer = Some(hovered);
            }
            None => self.ask_link(spot, cell, window, cx),
        }
    }

    pub(super) fn forget_link(&mut self, cx: &mut Context<Self>) {
        self.pointer.links.spot = None;
        self.unlink(cx);
        Peek::get(cx).update(cx, Peek::left_link);
    }

    pub(super) fn drop_link(&mut self, cx: &mut Context<Self>) {
        self.pointer.links.spot = None;
        self.dismiss_link(cx);
    }

    pub(super) fn dismiss_link(&mut self, cx: &mut Context<Self>) {
        self.unlink(cx);
        Peek::get(cx).update(cx, Peek::close);
    }

    fn unlink(&mut self, cx: &mut Context<Self>) {
        self.pointer.links.answer = None;
        if self.pointer.links.on {
            self.pointer.links.on = false;
            cx.notify();
        }
    }

    fn ask_link(&mut self, spot: Spot, cell: (u16, u16), window: &mut Window, cx: &mut Context<Self>) {
        if self.pointer.links.asking {
            self.pointer.links.next = Some((spot, cell));
            return;
        }
        self.pointer.links.asking = true;
        let args = link_args(spot.pane, cell);
        let task = cx.background_spawn(async move { cli::run(&args).ok() });
        cx.spawn_in(window, async move |this, cx| {
            let out = task.await;
            let _ = this.update_in(cx, |this, window, cx| this.link_answered(&spot, out.as_deref(), window, cx));
        })
        .detach();
    }

    fn link_answered(&mut self, spot: &Spot, out: Option<&str>, window: &mut Window, cx: &mut Context<Self>) {
        self.pointer.links.asking = false;
        let next = self.pointer.links.next.take();
        if self.pointer.links.spot.as_ref() == Some(spot) {
            let link = out.and_then(Link::parse).filter(|link| link.row == spot.row);
            self.pointer.links.answer = link.and_then(|link| Some(Hovered::file(self.grid.pane(spot.pane)?, link)));
            self.settle_link(window, cx);
        } else if let Some((next, cell)) = next.filter(|(next, _)| self.pointer.links.spot.as_ref() == Some(next)) {
            self.ask_link(next, cell, window, cx);
        }
    }

    fn settle_link(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let cell = self.pointer.links.cell;
        let on = self.pointer.links.answer.as_ref().is_some_and(|h| h.span.covers(cell) && h.shown_in(&self.grid));
        if on == self.pointer.links.on {
            return;
        }
        self.pointer.links.on = on;
        cx.notify();
        if on {
            self.peek_link(window, cx);
        } else {
            Peek::get(cx).update(cx, Peek::left_link);
        }
    }

    fn peek_link(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(hovered) = self.pointer.links.answer.clone() else { return };
        let Some(anchor) = self.anchor_of(&hovered) else { return };
        match hovered.target {
            Target::File(link) => {
                let files = self.panel(PanelKind::Files, window, cx);
                Peek::get(cx).update(cx, |peek, cx| peek.hover(link, anchor, &files, cx));
            }
            Target::Web(url) => {
                let page = self.web_page(url, window, cx);
                Peek::get(cx).update(cx, |peek, cx| peek.hover_web(page, anchor, cx));
            }
        }
    }

    fn web_page(&mut self, url: String, window: &mut Window, cx: &mut Context<Self>) -> WebPage {
        let local = is_loopback(&url);
        let load = self.store.read(cx).prefs.previews(local);
        if load {
            self.panel(PanelKind::Browser, window, cx);
        }
        WebPage { outside: !(local && self.host.is_some()), load, url }
    }

    fn open_web(&mut self, url: String, cx: &mut Context<Self>) {
        let outside = !(is_loopback(&url) && self.host.is_some());
        if self.store.read(cx).prefs.opens_in_panel(outside) {
            self.store.update(cx, |s, cx| s.open_in_browser(url, None, cx));
        } else {
            cx.open_url(&url);
        }
    }

    fn anchor_of(&self, hovered: &Hovered) -> Option<Anchor> {
        let layout = self.layout.borrow();
        let link = layout.span(hovered.pane, hovered.span.row, (hovered.span.start, hovered.span.end))?;
        Some(Anchor { link: Bounds::new(link.origin - layout.at.origin, link.size), room: layout.at.size })
    }

    pub(super) fn link_under(&self, pane: u64, cell: (u16, u16)) -> Option<Target> {
        let hovered = self.pointer.links.hovered().filter(|h| h.at(pane, cell) && h.shown_in(&self.grid))?;
        Some(hovered.target.clone())
    }

    pub(super) fn press_link(&mut self, pane: u64, cell: (u16, u16), event: &MouseDownEvent) -> bool {
        if !plain_click(event) || self.link_under(pane, cell).is_none() {
            return false;
        }
        self.drag = Some(Drag::Link { pane, cell });
        true
    }

    pub(super) fn drag_from_link(&mut self, pane: u64, from: (u16, u16), to: (u16, u16)) {
        let Some(shown) = self.grid.pane(pane).cloned().filter(|_| to != from) else { return };
        self.drag = None;
        if shown.modes.mouse == MouseMode::None {
            self.drag = Some(Drag::Select { pane, from, to: from });
        } else {
            self.press_program(&shown, from, MouseButton::Left);
        }
    }

    pub(super) fn click_link(&mut self, pane: u64, cell: (u16, u16), window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self.link_under(pane, cell) else { return };
        self.dismiss_link(cx);
        match target {
            Target::File(link) => {
                let panel = self.panel(PanelKind::Files, window, cx);
                files::open_link(&panel, &link, cx);
            }
            Target::Web(url) => self.open_web(url, cx),
        }
    }
}

#[cfg(test)]
mod tests;
