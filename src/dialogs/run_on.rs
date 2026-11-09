mod clone;

use gpui::{
    App, ClickEvent, Context, Div, Entity, EventEmitter, FocusHandle, IntoElement, ParentElement as _, Render,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use crate::cli::{self, Target};
use crate::model::Spot;
use crate::model::ThereId;
use crate::model::accounts::pick::{AccountPick, AgentChoices};
use crate::model::agent_settings::Agents;
use crate::model::machines::THIS_COMPUTER;
use crate::model::machines::run_on::checks::{AgentNote, Check, Checks, Hold, Wanted};
use crate::model::machines::run_on::issue::{IssueThere, follow};
use crate::model::machines::run_on::{NO_REMOTE, Offer, Source, Starting, There};
use crate::model::machines::seen::LoginTab;
use crate::notify::tip;
use crate::platform::paths;
use crate::store::{self, Busy, StartThere, Store};
use crate::theme::colours;
use crate::ui::{self, Icon, TextField, Tone};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RunOnEvent {
    Chosen,
    Flows(Result<String, String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Gate {
    Here,
    There(Spot),
    Held,
}

struct Reach<'a> {
    machine: &'a str,
    target: &'a Target,
    spot: Option<&'a Spot>,
}

pub(crate) struct RunOn {
    project: Option<u64>,
    wanted: Wanted,
    chosen: Option<String>,
    checks: Checks,
}

impl EventEmitter<RunOnEvent> for RunOn {}

fn refused(why: &str, cx: &mut App) -> Gate {
    Store::get(cx).update(cx, |s, cx| s.not_sent(why, cx));
    Gate::Held
}

pub(crate) fn dialog_starts<T: 'static>(cx: &Context<T>) -> bool {
    Store::get(cx).read(cx).dialog_starts(cx.entity_id().as_u64())
}

impl RunOn {
    pub(crate) fn new(project: Option<u64>, wanted: Wanted, cx: &mut Context<Self>) -> Self {
        cx.observe(&Store::get(cx), |this, _, cx| {
            this.ask_due(cx);
            cx.notify();
        })
        .detach();
        Self { project, wanted, chosen: None, checks: Checks::default() }
    }

    pub(crate) fn machine(&self) -> Option<&str> {
        self.chosen.as_deref()
    }

    pub(crate) fn offered(&self, cx: &App) -> bool {
        self.chosen.is_some() || !Store::get(cx).read(cx).machines.is_empty()
    }

    pub(crate) fn flows_asking(&self) -> bool {
        self.checks.flows.asking()
    }

    pub(crate) fn set_project(&mut self, project: Option<u64>, cx: &mut Context<Self>) {
        if self.project != project {
            self.project = project;
            self.ask_due(cx);
            cx.notify();
        }
    }

    pub(crate) fn there(&self, cx: &App) -> Option<There> {
        Some(Store::get(cx).read(cx).there(self.chosen.as_deref()?, self.project))
    }

    pub(crate) fn account_choices(&self, cx: &App) -> Vec<AgentChoices> {
        let store = Store::get(cx).read(cx);
        match &self.chosen {
            None => store.account_choices(),
            Some(_) => self.checks.accounts.read().map(|info| store.account_settings.offered(info)).unwrap_or_default(),
        }
    }

    pub(crate) fn modes_source(&self) -> Source<'_, Agents> {
        match self.chosen {
            Some(_) => Source::There(self.checks.modes.read()),
            None => Source::Here,
        }
    }

    pub(crate) fn issue_source(&self) -> Source<'_, IssueThere> {
        match self.chosen {
            Some(_) => Source::There(self.checks.issue.read()),
            None => Source::Here,
        }
    }

    pub(crate) fn issue_there(&self) -> Option<&IssueThere> {
        match self.issue_source() {
            Source::There(read) => read,
            Source::Here => None,
        }
    }

    fn choose(&mut self, machine: Option<String>, cx: &mut Context<Self>) {
        let moved = self.chosen != machine;
        if !moved && machine.is_none() {
            return;
        }
        self.chosen = machine;
        self.checks.restart();
        if moved {
            cx.emit(RunOnEvent::Chosen);
        }
        self.ask_due(cx);
        cx.notify();
    }

    fn ask_due(&mut self, cx: &mut Context<Self>) {
        let Some(machine) = self.chosen.clone() else { return };
        Store::get(cx).update(cx, |s, cx| s.ask_machine_home(&machine, cx));
        let s = Store::get(cx).read(cx);
        let Some(seen) = s.session.as_ref().and_then(|session| session.machine(&machine)) else { return };
        let (boot, linked) = (seen.boot.clone(), seen.linked());
        let target = s.target_of(&machine).ok();
        let spot = match s.there(&machine, self.project) {
            There::Ready(spot) => Some(spot),
            _ => None,
        };
        self.checks.follow(&boot, linked, spot.as_ref().map(|spot| spot.project));
        let Some(target) = target else { return };
        let reach = Reach { machine: &machine, target: &target, spot: spot.as_ref() };
        for check in self.checks.due(&self.wanted) {
            self.ask(check, &reach, cx);
        }
    }

    fn ask(&mut self, check: Check, reach: &Reach<'_>, cx: &mut Context<Self>) {
        self.checks.asked(check);
        let asked = (self.checks.generation(), reach.spot.map(|spot| spot.project));
        let busy = Busy::new(format!("run-on:{}:{check:?}", cx.entity_id()), check.doing(reach.machine));
        store::run_on_busy(
            cx,
            busy,
            reach.target.clone(),
            check.args(reach.spot, &self.wanted),
            move |this, result, cx| {
                this.heard(check, asked, result.map_err(|e| cli::reason(&e)), cx);
            },
        );
    }

    fn heard(
        &mut self,
        check: Check,
        (generation, project): (u64, Option<ThereId>),
        answer: Result<String, String>,
        cx: &mut Context<Self>,
    ) {
        if !self.checks.current(generation, check, project) {
            return;
        }
        self.checks.heard(check, answer.as_deref().map_err(Clone::clone));
        if check == Check::Flows {
            cx.emit(RunOnEvent::Flows(answer));
        }
        self.ask_due(cx);
        cx.notify();
    }

    pub(crate) fn gate(&mut self, back: FocusHandle, cx: &mut Context<Self>) -> Gate {
        let Some(there) = self.there(cx) else { return Gate::Here };
        match there {
            There::Ready(spot) => self.ready(spot, cx),
            There::NotCloned => {
                self.open_clone(back, cx);
                Gate::Held
            }
            There::Cloning | There::Listing => Gate::Held,
            There::NoRemote => refused(NO_REMOTE, cx),
            There::Refused(why) => refused(&why, cx),
        }
    }

    fn ready(&mut self, spot: Spot, cx: &mut Context<Self>) -> Gate {
        match self.checks.hold(&self.wanted, &spot.machine, &spot.name) {
            None => Gate::There(spot),
            Some(Hold::Waiting) => Gate::Held,
            Some(Hold::Why(why)) => {
                self.checks.forget_failures();
                self.ask_due(cx);
                refused(&why, cx)
            }
        }
    }

    fn open_clone(&self, back: FocusHandle, cx: &mut Context<Self>) {
        let Some(machine) = self.chosen.clone() else { return };
        clone::open(&machine, self.project, back, cx);
    }

    fn open_login(&mut self, note: &AgentNote, cx: &mut Context<Self>) {
        let Some(There::Ready(spot)) = self.there(cx) else { return };
        if Store::get(cx).read(cx).shown_dialog_starts() {
            return;
        }
        let command = self.checks.accounts.read().and_then(|info| info.command_of(&note.agent)).map(str::to_string);
        let tab = LoginTab { agent: &note.agent, account: note.account.as_deref(), command: command.as_deref() };
        let Some(command) = spot.login_tab_args(tab) else {
            let why = format!("{} has no workspace of this project for the login tab", spot.machine);
            refused(&why, cx);
            return;
        };
        let target = match Store::get(cx).read(cx).target_of(&spot.machine) {
            Ok(target) => target,
            Err(why) => {
                refused(&why, cx);
                return;
            }
        };
        let starting = Starting::LoginTab { agent: note.agent.clone() };
        let start = StartThere { spot, starting, command, target, select: true };
        Store::get(cx).update(cx, |s, cx| s.start_there(start, cx));
    }

    pub(crate) fn where_line(&self, how: &str, cx: &App) -> Option<Div> {
        let machine = self.chosen.as_deref()?;
        let line = |text: String| div().text_size(px(12.5)).text_color(colours::soft()).child(text);
        Some(match self.there(cx)? {
            There::Ready(spot) => {
                let home = Store::get(cx).read(cx).machine_home(&spot.boot);
                let folder = home.map_or_else(|| spot.path.clone(), |home| paths::home_relative(&spot.path, home));
                line(format!("{how} on {machine}, in {folder}"))
            }
            There::NotCloned => line(format!("Not on {machine} yet: it is cloned there first")),
            There::Cloning => ui::busy("run-on-cloning", format!("Cloning on {machine}…")),
            There::Listing => ui::busy("run-on-listing", format!("Cloned: waiting for {machine} to list it…")),
            There::NoRemote => return None,
            There::Refused(why) => ui::error_line(why),
        })
    }

    fn blocked(&self, cx: &App) -> bool {
        Store::get(cx).read(cx).project_here(self.project).is_some_and(|project| project.repo.is_none())
    }

    fn here_chip(&self, cx: &Context<Self>) -> Stateful<Div> {
        let label = cli::host().unwrap_or_else(|| THIS_COMPUTER.to_string());
        ui::icon_segment("run-on-here", Icon::Laptop, label, self.chosen.is_none())
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.choose(None, cx)))
    }

    fn offer_chip(&self, at: usize, offer: &Offer, blocked: bool, cx: &Context<Self>) -> Stateful<Div> {
        let label = match &offer.why {
            Some(why) => format!("{} · {why}", offer.name),
            None => offer.name.clone(),
        };
        let id = ("run-on", at);
        let chosen = self.chosen.as_deref() == Some(offer.name.as_str());
        if !blocked && offer.why.is_none() {
            let name = offer.name.clone();
            return ui::icon_segment(id, offer.icon.icon(), label, chosen)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.choose(Some(name.clone()), cx)));
        }
        let chip = ui::resting_icon_segment(id, offer.icon.icon(), label, chosen);
        match offer.said.clone() {
            Some(said) => chip.tooltip(tip(said)),
            None => chip,
        }
    }

    fn chips(&self, blocked: bool, cx: &Context<Self>) -> Div {
        let offers = Store::get(cx).read(cx).offers();
        let others = offers.iter().enumerate().map(|(at, offer)| self.offer_chip(at, offer, blocked, cx));
        let chips = std::iter::once(self.here_chip(cx)).chain(others).map(IntoElement::into_any_element);
        ui::segments(chips.collect()).flex_wrap()
    }

    fn asking_lines(&self, machine: &str) -> Vec<Div> {
        let line = |(at, check): (usize, Check)| {
            let id = SharedString::from(format!("run-on-asking-{at}"));
            ui::busy(id, check.doing(machine))
        };
        self.checks.asking().into_iter().enumerate().map(line).collect()
    }

    fn checks_lines(&self, machine: &str, cx: &App) -> Vec<Div> {
        let failed = self.checks.failures().into_iter().map(|(check, why)| check.failed(machine, why));
        let spot = match self.there(cx) {
            Some(There::Ready(spot)) => Some(spot),
            _ => None,
        };
        let lacks = spot.and_then(|spot| self.checks.tracker_note(&self.wanted, machine, &spot.name));
        failed.chain(lacks).map(ui::error_line).collect()
    }

    fn notes(&self, blocked: bool, cx: &App) -> Vec<Div> {
        let mut notes = Vec::new();
        if blocked {
            notes.push(div().text_size(px(12.)).text_color(colours::muted()).child(NO_REMOTE));
        }
        if let Some(machine) = self.chosen.as_deref() {
            notes.extend(self.asking_lines(machine));
            notes.extend(self.checks_lines(machine, cx));
        }
        notes
    }
}

impl Render for RunOn {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let blocked = self.blocked(cx);
        ui::field(
            "Run on",
            div().flex().flex_col().gap(px(6.)).child(self.chips(blocked, cx)).children(self.notes(blocked, cx)),
        )
    }
}

pub(crate) fn follow_field(
    field: &Entity<TextField>,
    hint: String,
    (was, now): (&str, String),
    cx: &mut App,
) -> String {
    let fill = follow(&field.read(cx).text(), was, &now);
    field.update(cx, |field, cx| {
        field.set_placeholder(hint, cx);
        if let Some(text) = fill {
            field.set_text(text, cx);
        }
    });
    now
}

pub(crate) fn accounts_of(run_on: &Entity<RunOn>, fields: Option<Div>, cx: &App) -> Option<Div> {
    let fields = fields?;
    Some(match run_on.read(cx).machine() {
        Some(machine) => {
            div().flex().flex_col().gap(px(4.)).child(fields).child(ui::dim(format!("{machine}'s accounts")))
        }
        None => fields,
    })
}

pub(crate) fn agent_line(run_on: &Entity<RunOn>, agents: &[String], picks: &[AccountPick], cx: &App) -> Option<Div> {
    let this = run_on.read(cx);
    let machine = this.chosen.as_deref()?;
    let picked = |agent: &str| picks.iter().find(|pick| pick.agent == agent).map(|pick| pick.folder.clone());
    let note = this.checks.accounts.read()?.agent_note(agents, picked)?;
    let text = note.text(machine);
    let ready = matches!(this.there(cx), Some(There::Ready(_)));
    let open = ready.then(|| login_button(run_on.clone(), note, cx));
    Some(
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(div().flex_1().min_w_0().child(ui::warning_line(text)))
            .children(open),
    )
}

fn login_button(run_on: Entity<RunOn>, note: AgentNote, cx: &App) -> Stateful<Div> {
    if Store::get(cx).read(cx).shown_dialog_starts() {
        return ui::busy_button("run-on-login", "Opening…", Tone::Plain);
    }
    ui::button("run-on-login", note.button())
        .on_click(move |_: &ClickEvent, _, cx| run_on.update(cx, |run_on, cx| run_on.open_login(&note, cx)))
}
