use gpui::{App, Context, FocusHandle, Window};

use super::Root;
use crate::panels::{PanelKind, bot_chat};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Rest {
    Terminal,
    Field,
}

pub(crate) fn rest_of(chat_shown: bool) -> Rest {
    if chat_shown { Rest::Field } else { Rest::Terminal }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Place {
    Terminal,
    Sheet(PanelKind),
    Dock(PanelKind),
    Window,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Overlays {
    pub(super) dialog: bool,
    pub(super) sheet: Option<PanelKind>,
    pub(super) full: bool,
}

impl Overlays {
    pub(super) fn ended_into(self, now: Self) -> bool {
        self.dialog && !now.dialog
    }

    fn open(self) -> bool {
        self.dialog || self.sheet.is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Change {
    pub(crate) before: Overlays,
    pub(crate) now: Overlays,
    pub(crate) dock: Option<PanelKind>,
    pub(crate) in_dock: bool,
    pub(crate) to_terminal: bool,
    pub(crate) back: Option<Place>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Keys {
    Stay,
    Back,
    Root,
    DockCard,
}

#[derive(Default)]
pub(super) struct Held {
    pub(super) had: Overlays,
    home: Noted<gpui::WeakFocusHandle>,
}

pub(crate) fn sheet_gave_way(before: Option<PanelKind>, now: Option<PanelKind>) -> bool {
    before.is_some() && now.is_some() && before != now
}

fn shows(place: Place, now: Overlays, dock: Option<PanelKind>) -> bool {
    match place {
        Place::Sheet(kind) => now.sheet == Some(kind),
        Place::Dock(kind) => now.sheet.is_none() && dock == Some(kind),
        Place::Terminal | Place::Window => now.sheet.is_none() && !now.full,
    }
}

pub(crate) fn keys_after(change: &Change) -> Keys {
    let Change { before, now, dock, in_dock, to_terminal, back } = *change;
    let ended = before.ended_into(now);
    if ended && !to_terminal && back.is_some_and(|place| shows(place, now, dock)) {
        return Keys::Back;
    }
    let closed = before.open() && !now.open();
    let gave_way = !now.dialog && sheet_gave_way(before.sheet, now.sheet);
    if to_terminal || ended || closed || gave_way || (dock.is_none() && in_dock) {
        return if now.full { Keys::DockCard } else { Keys::Root };
    }
    Keys::Stay
}

pub(crate) struct Noted<T> {
    held: Option<(Place, T)>,
}

impl<T> Default for Noted<T> {
    fn default() -> Self {
        Self { held: None }
    }
}

impl<T> Noted<T> {
    pub(crate) fn note(&mut self, dialog: bool, now: Option<(Place, T)>) {
        if !dialog {
            self.held = now;
        }
    }

    pub(crate) fn take(&mut self) -> Option<(Place, T)> {
        self.held.take()
    }
}

impl Root {
    pub(super) fn mind_the_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.keep_the_keys(window, cx);
        self.note_keys(window, cx);
    }

    fn note_keys(&mut self, window: &Window, cx: &App) {
        let dialog = self.store.read(cx).dialog.is_some();
        let noted = window.focused(cx).filter(|_| !dialog);
        let noted = noted.map(|focused| (self.place_of(&focused, window, cx), focused.downgrade()));
        self.held.home.note(dialog, noted);
    }

    fn place_of(&self, focused: &FocusHandle, window: &Window, cx: &App) -> Place {
        let s = self.store.read(cx);
        if *focused == self.focus {
            return Place::Terminal;
        }
        if let Some(kind) = s.sheet {
            return Place::Sheet(kind);
        }
        match s.shown_dock() {
            Some(kind) if self.dock_focus.contains_focused(window, cx) => Place::Dock(kind),
            _ => Place::Window,
        }
    }

    fn way_back(&mut self, ended: bool, window: &Window) -> Option<(Place, FocusHandle)> {
        if !ended {
            return None;
        }
        let (place, weak) = self.held.home.take()?;
        let handle = weak.upgrade()?;
        self.focus.contains(&handle, window).then_some((place, handle))
    }

    pub(crate) fn keys_rest(&self, cx: &mut App) -> FocusHandle {
        match rest_of(self.chat_shown()) {
            Rest::Field => bot_chat::field_focus(cx),
            Rest::Terminal => self.focus.clone(),
        }
    }

    pub(super) fn keys_follow(&self, rested: &FocusHandle, window: &mut Window, cx: &mut App) {
        if rested.is_focused(window) {
            window.focus(&self.keys_rest(cx));
        }
    }

    pub(super) fn keys_changed(
        &mut self,
        (before, now): (Overlays, Overlays),
        to_terminal: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let s = self.store.read(cx);
        let dock = s.shown_dock();
        let in_dock = self.dock_focused(window, cx);
        let way = self.way_back(before.ended_into(now), window);
        let change = Change { before, now, dock, in_dock, to_terminal, back: way.as_ref().map(|(place, _)| *place) };
        match (keys_after(&change), way) {
            (Keys::Back, Some((Place::Terminal, _))) | (Keys::Root, _) => window.focus(&self.keys_rest(cx)),
            (Keys::Back, Some((_, handle))) => window.focus(&handle),
            (Keys::DockCard, _) => window.focus(&self.dock_focus),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
