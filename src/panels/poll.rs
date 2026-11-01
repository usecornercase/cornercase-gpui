use std::collections::HashMap;
use std::time::Duration;

use gpui::{AsyncApp, Context, Entity, Task, WeakEntity, Window};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::dialogs::{self, Question};
use crate::panels::PanelKind;
use crate::store::{self, Busy, Store, ToastKind};

pub(crate) trait Polled: Sized + 'static {
    const KIND: PanelKind;
    const EVERY: Duration;

    fn watch(&mut self) -> &mut Watch;

    fn measure(&mut self, cx: &mut Context<Self>);
}

pub(crate) struct Watch {
    open: bool,
    busy: bool,
    openings: u64,
    pub(crate) stopped: Stopped,
    _tick: Task<()>,
}

#[derive(Default)]
pub(crate) struct Stopped {
    began: u64,
    marks: HashMap<String, u64>,
}

impl Stopped {
    fn began(&mut self) {
        self.began += 1;
    }

    fn ended(&mut self) {
        let began = self.began;
        self.marks.retain(|_, at| *at >= began);
    }

    pub(crate) fn mark(&mut self, key: &str) {
        self.marks.insert(key.to_string(), self.began);
    }

    pub(crate) fn holds(&self, key: &str) -> bool {
        self.marks.contains_key(key)
    }
}

impl Watch {
    pub(crate) fn start<T: Polled>(cx: &mut Context<T>) -> Self {
        let store = Store::get(cx);
        cx.observe(&store, |this, store, cx| {
            let open = shows(store.read(cx), T::KIND);
            let opened = open && !this.watch().open;
            this.watch().open = open;
            if opened {
                this.watch().openings += 1;
                this.measure(cx);
            }
        })
        .detach();
        let tick = cx.spawn(async move |this, cx| ticks(this, cx).await);
        Self { open: true, busy: false, openings: 0, stopped: Stopped::default(), _tick: tick }
    }

    pub(crate) fn opening(&self) -> u64 {
        self.openings
    }

    pub(crate) fn begin(&mut self) -> bool {
        let begins = !std::mem::replace(&mut self.busy, true);
        if begins {
            self.stopped.began();
        }
        begins
    }

    pub(crate) fn end(&mut self) {
        self.busy = false;
        self.stopped.ended();
    }

    pub(crate) fn stopping(&self, key: &str, store: &Store) -> bool {
        self.stopped.holds(key) || store.busy(key)
    }
}

fn shows(store: &Store, kind: PanelKind) -> bool {
    if kind.docked() { store.shown_dock() == Some(kind) } else { store.sheet == Some(kind) }
}

async fn ticks<T: Polled>(this: WeakEntity<T>, cx: &mut AsyncApp) {
    loop {
        cx.background_executor().timer(T::EVERY).await;
        if this.update(cx, due).is_err() {
            return;
        }
    }
}

fn due<T: Polled>(this: &mut T, cx: &mut Context<T>) {
    let w = this.watch();
    if w.open && !w.busy {
        this.measure(cx);
    }
}

pub(crate) enum Stop<T> {
    Never,
    Ask(Entity<T>),
    Stopping,
}

impl<T> Stop<T> {
    pub(crate) fn of(stoppable: bool, stopping: bool, panel: &Entity<T>) -> Self {
        match (stoppable, stopping) {
            (false, _) => Stop::Never,
            (true, true) => Stop::Stopping,
            (true, false) => Stop::Ask(panel.clone()),
        }
    }
}

pub(crate) fn confirm_stop<T: Polled>(
    question: Question,
    busy: Busy,
    command: Vec<String>,
    window: &mut Window,
    cx: &mut Context<T>,
) {
    let this = cx.entity();
    let question = question.on_yes(move |cx| {
        let (busy, command) = (busy.clone(), command.clone());
        let key = busy.key().to_string();
        this.update(cx, |_, cx| store::run_busy(cx, busy, command, move |this, r, cx| stopped(this, &key, r, cx)));
    });
    dialogs::confirm(question, window, cx);
}

fn stopped<T: Polled>(this: &mut T, key: &str, result: anyhow::Result<String>, cx: &mut Context<T>) {
    match result {
        Ok(_) => this.watch().stopped.mark(key),
        Err(e) => Store::get(cx).update(cx, |s, cx| s.toast(e.to_string(), ToastKind::Error, cx)),
    }
    this.measure(cx);
}

pub(crate) fn parse<T: DeserializeOwned>(result: anyhow::Result<Value>) -> Result<T, String> {
    result.and_then(|v| serde_json::from_value(v).map_err(anyhow::Error::from)).map_err(|e| e.to_string())
}

pub(crate) fn parse_text<T: DeserializeOwned>(result: anyhow::Result<String>) -> Result<T, String> {
    parse(result.and_then(|out| serde_json::from_str(&out).map_err(anyhow::Error::from)))
}

#[cfg(test)]
mod tests;
