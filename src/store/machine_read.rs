use std::collections::HashMap;
use std::time::{Duration, Instant};

use gpui::Context;
use serde_json::Value;

use crate::cli::{self, Older, Remote, Target, Version};
use crate::desktop;
use crate::model::machines::{LinkState, Machine, MachineIcon, NextTry};
use crate::model::{Aim, MachinePlace, MachineSeen};
use crate::panels::PanelKind;
use crate::platform::{Os, paths};

use super::machines_there::CliRelease;
use super::{Busy, Store, UpdateThere, args, run_with, update_job, update_key};

const WAITING: &str = "waiting for the machines list";
const CHECK_EVERY: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct OlderNote {
    pub line: String,
    pub tip: String,
    pub tunnels: String,
    pub update: Option<Box<UpdateThere>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Reach {
    Up(Target),
    Waiting(String),
    Older(OlderNote),
    Down(String),
    Refused(String),
}

impl Reach {
    pub(crate) fn target(&self) -> Option<&Target> {
        match self {
            Self::Up(target) => Some(target),
            Self::Waiting(_) | Self::Older(_) | Self::Down(_) | Self::Refused(_) => None,
        }
    }

    pub(crate) fn older(&self) -> Option<&OlderNote> {
        match self {
            Self::Older(note) => Some(note),
            Self::Up(_) | Self::Waiting(_) | Self::Down(_) | Self::Refused(_) => None,
        }
    }

    pub(crate) fn why_not(&self) -> Option<&str> {
        match self {
            Self::Up(_) => None,
            Self::Older(note) => Some(&note.line),
            Self::Waiting(why) | Self::Down(why) | Self::Refused(why) => Some(why),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Listing<'a> {
    pub machines: &'a [Machine],
    pub known: bool,
}

impl Listing<'_> {
    pub(super) fn find(&self, name: &str) -> Option<&Machine> {
        self.machines.iter().find(|machine| machine.name == name)
    }
}

pub(crate) fn release_clause(name: &str, older: &Older, os: Os) -> String {
    let (here, mine) = (desktop::machine(os), &older.here);
    match &older.there {
        Some(there) => format!("{name} runs cornercase {there} and this {here} runs {mine}"),
        None => format!("{name} runs a cornercase too old to say which release it is, and this {here} runs {mine}"),
    }
}

pub(crate) fn older_note(machine: &str, older: &Older, os: Os) -> OlderNote {
    let here = desktop::machine(os);
    let tip = match cli::update_args(older) {
        Some(rest) => format!("Update it from this {here} with cornercase {}", rest.join(" ")),
        None => format!("Update cornercase on {machine} from this {here}"),
    };
    let clause = release_clause(machine, older, os);
    let update = cli::update_command(older).map(|args| {
        Box::new(UpdateThere { machine: machine.to_string(), destination: older.destination.clone(), args })
    });
    OlderNote {
        line: format!("{clause}: update it"),
        tip,
        tunnels: format!("{clause}: nothing new is tunnelled to {machine} until it is updated"),
        update,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Facts {
    boot: String,
    state: LinkState,
    version: Option<String>,
    server_version: Option<String>,
    destination: Option<String>,
    here: Option<Version>,
}

impl Facts {
    pub(super) fn of(seen: &MachineSeen, listed: Option<&Machine>, here: Option<Version>) -> Self {
        Self {
            boot: seen.boot.clone(),
            state: seen.state,
            version: listed.and_then(|machine| machine.version.clone()),
            server_version: listed.and_then(|machine| machine.server_version.clone()),
            destination: listed.map(|machine| machine.destination.clone()),
            here,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct KeptOlder {
    pub(super) facts: Facts,
    pub(super) older: Older,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Probe {
    Older(Older),
    Fine,
    Failed,
}

fn probe_of(result: &anyhow::Result<String>) -> Probe {
    match result {
        Ok(_) => Probe::Fine,
        Err(error) => cli::older_refusal(error).cloned().map_or(Probe::Failed, Probe::Older),
    }
}

pub(super) fn sharing_destination(kept: &HashMap<String, KeptOlder>, first: &str, destination: &str) -> Vec<String> {
    let mut sharing: Vec<String> =
        kept.iter().filter(|(_, kept)| kept.older.destination == destination).map(|(name, _)| name.clone()).collect();
    sharing.sort_by_key(|name| (name != first, name.clone()));
    sharing
}

pub(super) fn still_current(asked: Option<Facts>, now: Option<&Facts>) -> Option<Facts> {
    asked.filter(|asked| now == Some(asked))
}

pub(super) fn kept_reach(reach: Reach, kept: Option<&KeptOlder>, facts: &Facts, machine: &str) -> Reach {
    match (reach, kept) {
        (Reach::Up(_), Some(kept)) if kept.facts == *facts => {
            Reach::Older(older_note(machine, &kept.older, Os::CURRENT))
        }
        (reach, _) => reach,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Check {
    InFlight,
    Answered { facts: Facts, at: Instant },
}

pub(super) fn may_check(reach: &Reach, last: Option<&Check>, facts: &Facts, now: Instant) -> bool {
    if !matches!(reach, Reach::Up(_)) {
        return false;
    }
    match last {
        None => true,
        Some(Check::InFlight) => false,
        Some(Check::Answered { facts: was, at }) => was != facts || now.saturating_duration_since(*at) >= CHECK_EVERY,
    }
}

#[derive(Default)]
pub(super) struct Homes {
    known: HashMap<String, (String, String)>,
    asked: HashMap<String, String>,
}

impl Homes {
    fn ask(&mut self, machine: &str, boot: &str) -> bool {
        let known = self.known.get(machine).is_some_and(|(held, _)| held == boot);
        if known || self.asked.get(machine).is_some_and(|asked| asked == boot) {
            return false;
        }
        self.asked.insert(machine.to_owned(), boot.to_owned());
        true
    }

    fn answered(&mut self, machine: &str, boot: &str, home: Option<String>) {
        if self.asked.get(machine).is_none_or(|asked| asked != boot) {
            return;
        }
        self.asked.remove(machine);
        if let Some(home) = home {
            self.known.insert(machine.to_owned(), (boot.to_owned(), home));
        }
    }

    fn home(&self, machine: &str, boot: &str) -> Option<&str> {
        self.known.get(machine).filter(|(held, _)| held == boot).map(|(_, home)| home.as_str())
    }
}

fn home_in(status: &str) -> Option<String> {
    let status: Value = serde_json::from_str(status).ok()?;
    status.get("home").and_then(Value::as_str).map(str::to_string)
}

impl ThereState {
    fn keep(&mut self, machine: &str, asked: Option<Facts>, current: Option<&Facts>, older: Older) -> bool {
        let Some(facts) = still_current(asked, current) else { return false };
        let kept = KeptOlder { facts, older };
        self.older.insert(machine.to_owned(), kept.clone()) != Some(kept)
    }

    fn probe_answered(&mut self, machine: &str, answer: (Facts, Probe), current: Option<&Facts>) -> bool {
        let (facts, probe) = answer;
        self.checks.insert(machine.to_owned(), Check::Answered { facts: facts.clone(), at: Instant::now() });
        match probe {
            Probe::Older(older) => self.keep(machine, Some(facts), current, older),
            Probe::Fine => self.older.remove(machine).is_some(),
            Probe::Failed => false,
        }
    }

    pub(super) fn recheck(&mut self, machine: &str) -> bool {
        if self.checks.get(machine) == Some(&Check::InFlight) {
            return false;
        }
        self.checks.remove(machine);
        self.older.contains_key(machine)
    }
}

#[derive(Default)]
pub(super) struct ThereState {
    pub(super) homes: Homes,
    pub(super) release: CliRelease,
    pub(super) tab: Option<TabThere>,
    pub(super) older: HashMap<String, KeptOlder>,
    pub(super) checks: HashMap<String, Check>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentReach {
    Ready(Target),
    Down(String),
    Gone(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TabThere {
    pub machine: String,
    pub boot: String,
    pub target: Target,
    pub down: Option<String>,
    pub agents: Vec<u64>,
}

fn reach_agent(tab: Option<&TabThere>, machine: &str, boot: &str, pane: u64) -> AgentReach {
    let Some(tab) = tab.filter(|tab| tab.machine == machine) else {
        return AgentReach::Gone(format!("The tab on {machine} is no longer shown"));
    };
    if tab.boot != boot {
        return AgentReach::Gone(format!("{machine} restarted"));
    }
    if let Some(why) = &tab.down {
        return AgentReach::Down(why.clone());
    }
    if tab.agents.contains(&pane) {
        return AgentReach::Ready(tab.target.clone());
    }
    AgentReach::Gone(format!("that agent is no longer on {machine}"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MachineRead {
    pub aim: Aim,
    pub destination: String,
    pub icon: MachineIcon,
    pub place: String,
    pub path: String,
    pub folder: String,
    pub reach: Reach,
    pub retry_at: Option<String>,
}

impl MachineRead {
    pub(crate) fn next_try(&self, now: i64) -> Option<NextTry> {
        NextTry::at(self.retry_at.as_deref(), now)
    }
}

fn link_reach(window: &Target, seen: &MachineSeen) -> Option<Reach> {
    let name = &seen.name;
    if let Some(host) = window.host() {
        let why = format!("{name} is linked from {host}: its address means nothing from this {}", desktop::here());
        return Some(Reach::Refused(why));
    }
    if seen.linked() {
        return None;
    }
    let comes_back = matches!(seen.state, LinkState::Unreachable | LinkState::Connecting);
    let why = seen.link_note();
    Some(if comes_back { Reach::Down(why) } else { Reach::Refused(why) })
}

pub(super) fn reach(window: &Target, listing: Listing<'_>, seen: &MachineSeen) -> Reach {
    if let Some(reach) = link_reach(window, seen) {
        return reach;
    }
    let name = &seen.name;
    let Some(listed) = listing.find(name) else {
        return if listing.known {
            Reach::Refused(format!("{name} is no longer listed"))
        } else {
            Reach::Waiting(WAITING.to_string())
        };
    };
    match Remote::new(&listed.destination, "") {
        Some(remote) => Reach::Up(Target::Remote(remote)),
        None => Reach::Refused(format!("{name}'s ssh destination cannot be used")),
    }
}

fn read_of(
    place: &MachinePlace,
    seen: &MachineSeen,
    (reach, destination): (Reach, String),
    home: Option<&str>,
) -> MachineRead {
    let listed = seen.place_name(place.workspace).filter(|_| seen.boot == place.boot);
    MachineRead {
        aim: place.aim(),
        destination,
        icon: seen.icon,
        place: listed.unwrap_or_else(|| place.name().to_string()),
        path: place.path().to_string(),
        folder: home.map_or_else(|| place.path().to_string(), |home| paths::home_relative(place.path(), home)),
        reach,
        retry_at: seen.retry_at.clone(),
    }
}

impl Store {
    pub(crate) fn set_tab_there(&mut self, tab: Option<TabThere>, cx: &mut Context<Self>) {
        if self.there.tab != tab {
            self.there.tab = tab;
            cx.notify();
        }
    }

    pub(crate) fn agent_reach(&self, machine: &str, boot: &str, pane: u64) -> AgentReach {
        reach_agent(self.there.tab.as_ref(), machine, boot, pane)
    }

    pub(crate) fn machine_read(&self) -> Option<MachineRead> {
        let place = self.machine_place.as_ref()?;
        let seen = self.session.as_ref()?.machine(&place.machine)?;
        let destination = self.listing().find(&seen.name).map_or_else(String::new, |m| m.destination.clone());
        let home = self.machine_home(&place.machine, &place.boot);
        Some(read_of(place, seen, (self.reach_seen(seen), destination), home))
    }

    pub(crate) fn machine_home(&self, machine: &str, boot: &str) -> Option<&str> {
        self.there.homes.home(machine, boot)
    }

    pub(crate) fn ask_machine_home(&mut self, machine: &str, cx: &mut Context<Self>) {
        let Some(seen) = self.session.as_ref().and_then(|session| session.machine(machine)) else { return };
        let (machine, boot) = (seen.name.clone(), seen.boot.clone());
        if !self.may_probe(seen) || !self.there.homes.ask(&machine, &boot) {
            return;
        }
        self.probe_there(&machine, None, cx);
    }

    pub(crate) fn listing(&self) -> Listing<'_> {
        Listing { machines: &self.machines, known: self.machines_known() }
    }

    fn facts_of(&self, seen: &MachineSeen) -> Facts {
        Facts::of(seen, self.listing().find(&seen.name), self.cli_release())
    }

    pub(crate) fn reach_seen(&self, seen: &MachineSeen) -> Reach {
        let reach = reach(&cli::target(), self.listing(), seen);
        match self.there.older.get(&seen.name) {
            Some(kept) => kept_reach(reach, Some(kept), &self.facts_of(seen), &seen.name),
            None => reach,
        }
    }

    pub(crate) fn there_facts(&self, machine: &str) -> Option<Facts> {
        let seen = self.session.as_ref()?.machine(machine)?;
        Some(self.facts_of(seen))
    }

    pub(crate) fn there_failed(&mut self, machine: &str, cx: &mut Context<Self>) {
        self.check_there(machine, cx);
    }

    pub(crate) fn there_refused(
        &mut self,
        machine: &str,
        (error, asked): (&anyhow::Error, Option<Facts>),
        cx: &mut Context<Self>,
    ) {
        let Some(older) = cli::older_refusal(error) else { return self.check_there(machine, cx) };
        self.keep_older(machine, asked, older.clone(), cx);
    }

    fn keep_older(&mut self, machine: &str, asked: Option<Facts>, older: Older, cx: &mut Context<Self>) {
        let current = self.there_facts(machine);
        if self.there.keep(machine, asked, current.as_ref(), older) {
            self.older_changed(cx);
        }
    }

    fn check_there(&mut self, machine: &str, cx: &mut Context<Self>) {
        let Some(seen) = self.session.as_ref().and_then(|session| session.machine(machine)) else { return };
        if self.may_probe(seen) {
            self.probe_there(machine, None, cx);
        }
    }

    fn may_probe(&self, seen: &MachineSeen) -> bool {
        let last = self.there.checks.get(&seen.name);
        may_check(&self.reach_seen(seen), last, &self.facts_of(seen), Instant::now())
    }

    pub(super) fn probe_there(&mut self, machine: &str, busy: Option<Busy>, cx: &mut Context<Self>) {
        let Some(seen) = self.session.as_ref().and_then(|session| session.machine(machine)) else { return };
        let Ok(target) = self.machine_target(seen) else { return };
        let (machine, boot, facts) = (seen.name.clone(), seen.boot.clone(), self.facts_of(seen));
        self.there.checks.insert(machine.clone(), Check::InFlight);
        self.there.homes.ask(&machine, &boot);
        let id = busy.map(|busy| self.start_job(busy, cx));
        let args = args(&["status", "--json"]);
        let job = id.map(|id| (cx.entity(), id));
        run_with(
            cx,
            job,
            move || cli::run_on(&target, &args, None),
            move |this: &mut Self, result, cx| {
                this.probed((&machine, &boot), facts, &result, cx);
            },
        );
    }

    fn probed(
        &mut self,
        (machine, boot): (&str, &str),
        facts: Facts,
        result: &anyhow::Result<String>,
        cx: &mut Context<Self>,
    ) {
        let home = result.as_ref().ok().map(String::as_str).and_then(home_in);
        self.there.homes.answered(machine, boot, home);
        let current = self.there_facts(machine);
        if self.there.probe_answered(machine, (facts, probe_of(result)), current.as_ref()) {
            self.older_changed(cx);
        }
        cx.notify();
    }

    fn older_changed(&mut self, cx: &mut Context<Self>) {
        cx.notify();
        self.follow_machine_tunnels(cx);
    }

    pub(crate) fn recheck_there(&mut self, machine: &str, cx: &mut Context<Self>) {
        let updating =
            self.there.older.get(machine).is_some_and(|kept| self.busy(&update_key(&kept.older.destination)));
        if !updating && self.there.recheck(machine) {
            self.probe_there(machine, None, cx);
        }
    }

    pub(super) fn probe_after_update(&mut self, update: &UpdateThere, cx: &mut Context<Self>) {
        let sharing = sharing_destination(&self.there.older, &update.machine, &update.destination);
        for (at, machine) in sharing.into_iter().enumerate() {
            self.there.checks.remove(&machine);
            let busy = (at == 0).then(|| update_job(&update.destination, &update.machine));
            self.probe_there(&machine, busy, cx);
        }
    }

    fn picked_machine(&self) -> Option<String> {
        self.machine_place.as_ref().map(|place| place.machine.clone())
    }

    pub(super) fn recheck_picked(&mut self, cx: &mut Context<Self>) {
        if let Some(machine) = self.picked_machine() {
            self.recheck_there(&machine, cx);
        }
    }

    pub(super) fn ask_picked_home(&mut self, cx: &mut Context<Self>) {
        if let Some(machine) = self.picked_machine() {
            self.ask_machine_home(&machine, cx);
        }
    }

    pub(super) fn shown_again(&mut self, kind: PanelKind, was_shown: bool, cx: &mut Context<Self>) {
        if kind.reads_machine() && !was_shown {
            self.recheck_picked(cx);
        }
    }
}

#[cfg(test)]
mod tests;
