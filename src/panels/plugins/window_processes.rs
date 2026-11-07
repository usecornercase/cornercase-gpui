use std::collections::BTreeMap;
use std::time::Instant;

use futures::StreamExt as _;
use futures::channel::mpsc;
use gpui::{App, AppContext as _, Context, Entity, Global};
use serde::Deserialize;

use super::status::{Health, Service};
use crate::cli;
use crate::store::{Store, ToastKind};

const ENDED: &str = "`cornercase plugin windows` ended";

#[derive(Debug, Deserialize)]
struct Line {
    plugin: String,
    #[serde(flatten)]
    service: Service,
}

#[derive(Debug)]
struct Entry {
    service: Service,
    at: Instant,
}

impl Entry {
    fn at(&self, now: Instant) -> Service {
        let passed = now.saturating_duration_since(self.at).as_secs();
        let restart_in = self.service.restart_in.map(|seconds| seconds.saturating_sub(passed));
        Service { restart_in, ..self.service.clone() }
    }
}

fn will_not_start_again(service: &Service) -> bool {
    service.health() == Health::Stopped && service.restart_in.is_none()
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Card {
    Starting,
    NotRunning(String),
    Processes(Vec<Service>),
}

#[derive(Debug, Default)]
pub(super) struct Seen {
    entries: BTreeMap<(String, String), Entry>,
    ended: Option<String>,
}

impl Seen {
    fn read(&mut self, text: &str, now: Instant) -> serde_json::Result<()> {
        let line: Line = serde_json::from_str(text)?;
        let key = (line.plugin, line.service.id.clone());
        if will_not_start_again(&line.service) {
            self.entries.remove(&key);
        } else {
            self.entries.insert(key, Entry { service: line.service, at: now });
        }
        Ok(())
    }

    fn end(&mut self, why: &str) -> String {
        let why = why.lines().last().map(str::trim).filter(|why| !why.is_empty()).unwrap_or(ENDED).to_string();
        self.entries.clear();
        self.ended = Some(why.clone());
        why
    }

    pub(super) fn card(&self, plugin: &str, now: Instant) -> Card {
        if let Some(why) = &self.ended {
            return Card::NotRunning(why.clone());
        }
        let of_it = self.entries.iter().filter(|((owner, _), _)| owner == plugin);
        let services: Vec<Service> = of_it.map(|(_, entry)| entry.at(now)).collect();
        if services.is_empty() { Card::Starting } else { Card::Processes(services) }
    }

    pub(super) fn counting(&self) -> bool {
        self.entries.values().any(|entry| entry.service.restart_in.is_some())
    }
}

enum Heard {
    Line(String),
    End(String),
}

#[derive(Default)]
pub(crate) struct WindowProcesses {
    pub(super) seen: Seen,
    kept: bool,
    tries: u32,
}

struct GlobalWindowProcesses(Entity<WindowProcesses>);
impl Global for GlobalWindowProcesses {}

pub(super) fn get(cx: &mut App) -> Entity<WindowProcesses> {
    if let Some(GlobalWindowProcesses(entity)) = cx.try_global::<GlobalWindowProcesses>() {
        return entity.clone();
    }
    let entity = cx.new(|_| WindowProcesses::default());
    cx.set_global(GlobalWindowProcesses(entity.clone()));
    entity
}

pub(crate) fn keep_window_processes(cx: &mut App) {
    get(cx).update(cx, WindowProcesses::keep);
}

impl WindowProcesses {
    fn keep(&mut self, cx: &mut Context<Self>) {
        if self.kept {
            return;
        }
        self.kept = true;
        self.seen.ended = None;
        let (tx, mut rx) = mpsc::unbounded();
        let ending = tx.clone();
        let started = cli::keep_window_processes(
            move |line| {
                let _ = tx.unbounded_send(Heard::Line(line));
            },
            move |why| {
                let _ = ending.unbounded_send(Heard::End(why));
            },
        );
        if let Err(e) = started {
            self.fell(&e.to_string(), cx);
            return;
        }
        cx.spawn(async move |this, cx| {
            while let Some(heard) = rx.next().await
                && this.update(cx, |this, cx| this.heard(heard, cx)).is_ok()
            {}
        })
        .detach();
        cx.notify();
    }

    fn heard(&mut self, heard: Heard, cx: &mut Context<Self>) {
        match heard {
            Heard::Line(text) => self.line(&text, cx),
            Heard::End(why) => self.fell(&why, cx),
        }
    }

    fn line(&mut self, text: &str, cx: &mut Context<Self>) {
        self.tries = 0;
        if let Err(e) = self.seen.read(text, Instant::now()) {
            say(format!("Could not read a plugin's window process: {e}"), cx);
        }
        cx.notify();
    }

    fn fell(&mut self, why: &str, cx: &mut Context<Self>) {
        self.kept = false;
        let why = self.seen.end(why);
        if self.tries == 0 {
            say(format!("The plugins' window processes stopped: {why}"), cx);
        }
        let wait = cli::backoff(self.tries);
        self.tries += 1;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let _ = this.update(cx, Self::keep);
        })
        .detach();
        cx.notify();
    }
}

fn say(text: String, cx: &mut App) {
    Store::get(cx).update(cx, |s, cx| s.toast(text, ToastKind::Error, cx));
}

#[cfg(test)]
mod tests;
