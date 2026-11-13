use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use futures::StreamExt as _;
use futures::channel::mpsc;
use gpui::{App, AppContext as _, Context, Entity, Global};
use serde::Deserialize;

use super::status::{Health, Service};
use crate::cli;
use crate::store::{Store, ToastKind};

const REASON_MOST: usize = 300;
const ENDED: &str = "`cornercase plugin windows` ended";
const UNTIED: &str = "If this window crashes, the plugins' window processes will keep running";

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

pub(super) fn shortened(why: &str) -> Option<String> {
    let (cut, _) = why.char_indices().nth(REASON_MOST)?;
    Some(format!("{}…", why[..cut].trim_end()))
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

#[derive(Debug, PartialEq, Eq)]
enum Step {
    Start,
    Stop,
    Stay,
}

#[derive(Debug)]
struct Fell {
    say: Option<String>,
    wait: Duration,
}

#[derive(Default)]
pub(crate) struct WindowProcesses {
    pub(super) seen: Seen,
    kept: bool,
    run: u64,
    tries: u32,
    told_untied: bool,
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

pub(crate) fn follow_window_processes(cx: &mut App) {
    get(cx).update(cx, WindowProcesses::follow);
}

impl WindowProcesses {
    fn step(&self, host: Option<&str>) -> Step {
        match (host, self.kept) {
            (None, false) => Step::Start,
            (Some(_), true) => Step::Stop,
            _ => Step::Stay,
        }
    }

    fn began(&mut self) -> u64 {
        self.kept = true;
        self.run += 1;
        self.seen.ended = None;
        self.run
    }

    fn stopped(&mut self) {
        self.kept = false;
        self.run += 1;
        self.tries = 0;
        self.seen = Seen::default();
    }

    fn untied(&mut self, why: Option<String>) -> Option<String> {
        let why = why?;
        if self.told_untied {
            return None;
        }
        self.told_untied = true;
        Some(format!("{UNTIED}. {why}"))
    }

    fn read_line(&mut self, run: u64, text: &str, now: Instant) -> serde_json::Result<()> {
        if run != self.run {
            return Ok(());
        }
        self.tries = 0;
        self.seen.read(text, now)
    }

    fn ended(&mut self, run: u64, why: &str) -> Option<Fell> {
        if run != self.run {
            return None;
        }
        self.kept = false;
        let why = self.seen.end(why);
        let say = (self.tries == 0).then(|| format!("The plugins' window processes stopped: {why}"));
        let wait = cli::backoff(self.tries);
        self.tries += 1;
        Some(Fell { say, wait })
    }

    fn follow(&mut self, cx: &mut Context<Self>) {
        match self.step(cli::host().as_deref()) {
            Step::Start => self.keep(cx),
            Step::Stop => {
                self.stopped();
                cli::stop_window_processes();
                cx.notify();
            }
            Step::Stay => {}
        }
    }

    fn keep(&mut self, cx: &mut Context<Self>) {
        let run = self.began();
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
        if !self.kept_it(run, started, cx) {
            return;
        }
        cx.spawn(async move |this, cx| {
            while let Some(heard) = rx.next().await
                && this.update(cx, |this, cx| this.heard(run, heard, cx)).is_ok()
            {}
        })
        .detach();
        cx.notify();
    }

    fn kept_it(&mut self, run: u64, started: anyhow::Result<Option<String>>, cx: &mut Context<Self>) -> bool {
        match started {
            Ok(untied) => {
                if let Some(text) = self.untied(untied) {
                    say(text, cx);
                }
                true
            }
            Err(e) => {
                self.fell(run, &e.to_string(), cx);
                false
            }
        }
    }

    fn heard(&mut self, run: u64, heard: Heard, cx: &mut Context<Self>) {
        match heard {
            Heard::Line(text) => {
                if let Err(e) = self.read_line(run, &text, Instant::now()) {
                    say(format!("Could not read a plugin's window process: {e}"), cx);
                }
                cx.notify();
            }
            Heard::End(why) => self.fell(run, &why, cx),
        }
    }

    fn fell(&mut self, run: u64, why: &str, cx: &mut Context<Self>) {
        let Some(fell) = self.ended(run, why) else { return };
        if let Some(text) = fell.say {
            say(text, cx);
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(fell.wait).await;
            let _ = this.update(cx, Self::follow);
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
