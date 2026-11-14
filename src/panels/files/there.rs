use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash as _, Hasher as _};
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Result, bail};
use gpui::{App, Context, Div};
use serde::Deserialize;

use crate::cli::Target;
use crate::model::{Aim, ReadReleases};
use crate::panels::on_machine::{self, Retry};
use crate::store::{MachineRead, Reach, Store, read_refusal, restart_note};

use super::disk::{Content, Stamp};
use super::gutter::Gutter;
use super::served::{self, Listed};
use super::source::Source;
use super::{FilesPanel, Mode, Results};

const ON_A_MACHINE: u64 = 1 << 63;

type Ask<'a> = &'a dyn Fn(&Target, &[String]) -> Result<String>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct There {
    target: Target,
    workspace: String,
    aim: Aim,
    releases: ReadReleases,
}

#[derive(Debug, Clone)]
struct Restarted(String);

impl fmt::Display for Restarted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Restarted {}

#[derive(Deserialize)]
struct Booted {
    boot: Option<String>,
}

fn restarted(json: &str, boot: &str) -> bool {
    let answered = serde_json::from_str::<Booted>(json).ok().and_then(|booted| booted.boot);
    answered.is_some_and(|answered| answered != boot)
}

pub(super) fn source_of(read: &MachineRead, releases: ReadReleases) -> Option<Source> {
    Some(Source::There(There::new(read.reach.target()?.clone(), &read.aim, releases)))
}

pub(super) fn place_key(read: &MachineRead) -> u64 {
    let mut hasher = DefaultHasher::new();
    read.aim.hash(&mut hasher);
    read.destination.hash(&mut hasher);
    hasher.finish() | ON_A_MACHINE
}

fn on_a_machine(key: u64) -> bool {
    key & ON_A_MACHINE != 0
}

pub(super) fn is_lost(error: &Error) -> bool {
    error.kind() == ErrorKind::NotConnected
}

fn lost_in(listed: &Listed) -> Option<String> {
    listed.iter().find_map(|(_, listing)| listing.as_ref().err().filter(|e| is_lost(e)).map(Error::to_string))
}

fn lost_content(error: &anyhow::Error) -> Content {
    match error.downcast_ref::<Restarted>() {
        Some(restarted) => Content::Restarted(restarted.to_string()),
        None => Content::Lost(error.to_string()),
    }
}

fn not_connected(error: &anyhow::Error) -> Error {
    match error.downcast_ref::<Restarted>() {
        Some(restarted) => Error::new(ErrorKind::NotConnected, restarted.clone()),
        None => Error::new(ErrorKind::NotConnected, error.to_string()),
    }
}

fn restarted_in(listed: &Listed) -> bool {
    let refused = |error: &Error| error.get_ref().is_some_and(<dyn std::error::Error + Send + Sync>::is::<Restarted>);
    listed.iter().any(|(_, listing)| listing.as_ref().err().is_some_and(refused))
}

impl There {
    fn new(target: Target, aim: &Aim, releases: ReadReleases) -> Self {
        Self { target, workspace: aim.workspace_arg(), aim: aim.clone(), releases }
    }

    fn asked(&self, ask: Ask<'_>, args: &[String]) -> Result<String> {
        let sent = self.aim.reading(args.to_vec(), self.releases);
        let json = ask(&self.target, &sent).map_err(|e| match read_refusal(&self.aim.machine, &e) {
            Some(why) => anyhow::Error::new(Restarted(why)),
            None => e,
        })?;
        if restarted(&json, &self.aim.boot) {
            bail!("{}", restart_note(&self.aim.machine));
        }
        Ok(json)
    }

    fn list_with(&self, ask: Ask<'_>, folders: Vec<PathBuf>) -> Listed {
        let args = served::files_args("list", &self.workspace, &[], folders.iter().map(|rel| served::arg(rel)));
        match self.asked(ask, &args) {
            Ok(json) => served::listings(folders, Ok(json)),
            Err(e) => folders.into_iter().map(|rel| (rel, Err(not_connected(&e)))).collect(),
        }
    }

    fn read_with(&self, ask: Ask<'_>, rel: &Path) -> (Content, Option<Stamp>) {
        let args = served::files_args("read", &self.workspace, &[], [served::arg(rel)]);
        match self.asked(ask, &args) {
            Ok(json) => served::content(rel, Ok(json)),
            Err(e) => (lost_content(&e), None),
        }
    }

    fn stamp_with(&self, ask: Ask<'_>, rel: &Path) -> Option<Stamp> {
        let (folder, name) = served::folder_and_name(rel)?;
        let args = served::files_args("list", &self.workspace, &[], [folder]);
        served::stamp_in(&self.asked(ask, &args).ok()?, &name)
    }

    fn same_run(&self, ask: Ask<'_>) -> Result<()> {
        if self.releases.carry_boot() {
            return Ok(());
        }
        let root = served::arg(Path::new(""));
        self.asked(ask, &served::files_args("list", &self.workspace, &[], [root])).map(drop)
    }

    fn gutter_with(&self, ask: Ask<'_>, rel: &Path) -> Option<Gutter> {
        self.same_run(ask).ok()?;
        Gutter::parse(&self.asked(ask, &served::changes_args(&self.workspace, rel)).ok()?)
    }

    fn search_with(&self, ask: Ask<'_>, query: &str, mode: Mode) -> Results {
        let args = served::files_args("search", &self.workspace, &[served::mode_flag(mode)], [query.to_string()]);
        served::results(mode, self.same_run(ask).and_then(|()| self.asked(ask, &args)))
    }

    pub(super) fn list(&self, folders: Vec<PathBuf>) -> Listed {
        self.list_with(&served::answer_on, folders)
    }

    pub(super) fn read(&self, rel: &Path) -> (Content, Option<Stamp>) {
        self.read_with(&served::answer_on, rel)
    }

    pub(super) fn stamp(&self, rel: &Path) -> Option<Stamp> {
        self.stamp_with(&served::answer_on, rel)
    }

    pub(super) fn gutter(&self, rel: &Path) -> Option<Gutter> {
        self.gutter_with(&served::answer_on, rel)
    }

    pub(super) fn search(&self, query: &str, mode: Mode) -> Results {
        self.search_with(&served::answer_on, query, mode)
    }
}

pub(super) fn unread_root(read: &MachineRead, listed: bool, retry: &Retry) -> Option<String> {
    let waits = matches!(read.reach, Reach::Waiting(_) | Reach::Older(_)) || retry.refused();
    (waits && !listed).then(|| on_machine::nothing_read(read))
}

impl FilesPanel {
    pub(super) fn follow_machine(&mut self, read: &MachineRead, cx: &mut Context<Self>) {
        let key = place_key(read);
        if self.current == Some(key) {
            self.stay_on_machine(read, cx);
            return;
        }
        self.leave_machine(cx);
        let releases = self.store.read(cx).read_releases(&read.aim.machine);
        match source_of(read, releases) {
            Some(source) => self.switch_to(key, PathBuf::from(&read.path), source, cx),
            None => {
                if self.current.take().is_some() {
                    self.left_place = true;
                    cx.notify();
                }
            }
        }
    }

    fn stay_on_machine(&mut self, read: &MachineRead, cx: &mut Context<Self>) {
        if read.reach.target().is_none() {
            self.retry = Retry::default();
            cx.notify();
        }
        if self.search.refused && self.showing(cx) && on_machine::may_ask(read, &self.retry).is_ok() {
            self.run_search(cx);
        }
    }

    pub(super) fn leave_machine(&mut self, cx: &mut Context<Self>) {
        self.retry = Retry::default();
        self.places.retain(|key, _| !on_a_machine(*key));
        if self.current.is_some_and(on_a_machine) {
            self.current = None;
            self.menu = None;
            self.left_place = true;
            self.search.forget();
            cx.notify();
        }
    }

    pub(super) fn may_poll(&self, cx: &Context<Self>) -> bool {
        let Some(read) = &self.machine else { return true };
        read.reach.target().is_some() && self.showing(cx) && self.retry.due(Instant::now())
    }

    pub(super) fn refuses_asking(&mut self, cx: &mut Context<Self>) -> Option<String> {
        let why = on_machine::may_ask(self.machine.as_ref()?, &self.retry).err()?;
        self.store.update(cx, |s, cx| s.not_sent(&why, cx));
        Some(why)
    }

    pub(super) fn list_machine_on_opening(&mut self, cx: &mut Context<Self>) {
        if self.machine.is_some() && self.place().is_some_and(|place| place.listings.is_empty()) {
            self.relist(cx);
        }
    }

    pub(super) fn machine_answered(&mut self, listed: &Listed, cx: &mut Context<Self>) {
        match lost_in(listed) {
            Some(_) if restarted_in(listed) => {
                self.retry.restarted();
                cx.notify();
            }
            Some(why) => {
                self.retry.failed(why, Instant::now());
                self.there_failed(cx);
                cx.notify();
            }
            None => {
                if self.retry.heard() {
                    cx.notify();
                }
            }
        }
    }

    fn there_failed(&mut self, cx: &mut Context<Self>) {
        let Some(machine) = self.machine.as_ref().map(|read| read.aim.machine.clone()) else { return };
        self.store.update(cx, |s, cx| s.there_failed(&machine, cx));
    }

    pub(super) fn machine_read_answered(&mut self, id: u64, content: Option<&Content>, cx: &mut Context<Self>) {
        if self.current != Some(id) {
            return;
        }
        match content {
            Some(Content::Lost(why)) => {
                self.retry.failed(why.clone(), Instant::now());
                self.there_failed(cx);
            }
            Some(Content::Restarted(_)) => {
                self.retry.restarted();
                cx.notify();
            }
            Some(_) => {
                self.retry.heard();
            }
            None => {}
        }
    }

    pub(super) fn machine_strip(&self, cx: &App) -> Option<Div> {
        self.machine.as_ref().map(|read| {
            let offer = Store::get(cx).read(cx).update_offer(read.reach.older());
            on_machine::strip(read, &self.retry, &self.counting, (true, &offer))
        })
    }

    pub(super) fn no_place_note(&self) -> String {
        self.machine.as_ref().map_or_else(|| "No workspace shown".to_string(), on_machine::nothing_read)
    }
}

#[cfg(test)]
mod tests;
