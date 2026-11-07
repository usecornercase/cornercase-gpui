use std::time::{Duration, Instant};

use futures::channel::mpsc::UnboundedSender;
use gpui::{ClipboardItem, Context, Window};
use serde_json::Value;

use super::connection::Connection;
use super::{Heard, Msg, Root};
use crate::cli::{self, Stream, Target};
use crate::desktop;
use crate::model::{Frame, Grid, MachinePlace, MachineSeen, Session, ThereId};
use crate::notify::{Event, Notifier, parse_event};
use crate::store::{ToastKind, args};
use crate::terminal::cleaned;
use crate::theme;

const MOST_HELD: usize = 1024;
const HELD_FOR: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, PartialEq, Eq)]
enum Lost {
    Link,
    Stream { next: Instant, try_number: u32, why: Option<String> },
    NoWay(String),
    Elsewhere(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Standing {
    Shown,
    Connecting,
    Reconnecting,
    LinkDown,
    RetryIn(u64, Option<String>),
    NoTab,
    NoWay(String),
    Elsewhere(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TabState {
    Opening,
    Live,
    Down(Lost),
}

#[derive(Debug, PartialEq, Eq)]
enum Plan {
    Nothing,
    Back,
    Open,
    Steer,
}

#[derive(Debug, PartialEq, Eq)]
enum LinkStep {
    Stay,
    Fall,
    Reopen,
}

#[derive(Debug, PartialEq, Eq)]
enum KeysGo {
    Pane(u64),
    Hold,
    Refuse,
    Nowhere,
}

#[derive(Debug, Default)]
struct Arriving {
    keys: Vec<Value>,
    since: Option<Instant>,
}

impl Arriving {
    fn too_long(&self, now: Instant) -> bool {
        self.since.is_some_and(|since| now.duration_since(since) > HELD_FOR)
    }

    fn hold(&mut self, event: Value, now: Instant) -> bool {
        if self.too_long(now) {
            *self = Self::default();
            return false;
        }
        self.since.get_or_insert(now);
        if self.keys.len() >= MOST_HELD {
            return false;
        }
        self.keys.push(event);
        true
    }
}

pub(crate) struct MachineTab {
    pub(super) conn: Connection,
    session: Option<Session>,
    state: TabState,
    pub(crate) machine: String,
    boot: String,
    aimed: MachinePlace,
    arriving: Option<Arriving>,
    attempts: u32,
    tries: u32,
    themed: bool,
    told_visible: bool,
    opened_visible: bool,
    host_events: Option<Stream>,
}

fn plan(tab: Option<&MachineTab>, place: Option<&MachinePlace>) -> Plan {
    match (tab, place) {
        (None, None) => Plan::Nothing,
        (Some(_), None) => Plan::Back,
        (Some(tab), Some(place)) if tab.machine == place.machine && tab.boot == place.boot => Plan::Steer,
        (_, Some(_)) => Plan::Open,
    }
}

fn link_step(state: &TabState, linked: bool) -> LinkStep {
    match (state, linked) {
        (TabState::Opening | TabState::Live | TabState::Down(Lost::Stream { .. }), false) => LinkStep::Fall,
        (TabState::Down(Lost::Link | Lost::NoWay(_)), true) => LinkStep::Reopen,
        _ => LinkStep::Stay,
    }
}

fn keys_go(tab: Option<&MachineTab>, pane: Option<u64>) -> KeysGo {
    match (tab, pane) {
        (_, Some(pane)) => KeysGo::Pane(pane),
        (None, None) => KeysGo::Nowhere,
        (Some(tab), None) if tab.state == TabState::Live && tab.arriving.is_some() => KeysGo::Hold,
        (Some(_), None) => KeysGo::Refuse,
    }
}

fn reason_of(err: &str) -> Option<String> {
    err.lines().map(str::trim).rfind(|line| !line.is_empty()).map(str::to_string)
}

fn copies_here(grid: &Grid, pane: Option<u64>) -> bool {
    pane.is_some_and(|pane| grid.pane(pane).is_some())
}

fn host_heard(event: Event) -> Option<Heard> {
    match event {
        Event::Clipboard { text, pane } => Some(Heard::Copied(text, pane)),
        Event::Bell { tab, .. } => Some(Heard::Bell(tab)),
        Event::Notice(_) | Event::Other => None,
    }
}

fn refusal(tab: &MachineTab, seen: Option<&MachineSeen>) -> String {
    let name = &tab.machine;
    match &tab.state {
        TabState::Opening | TabState::Live if tab.shows_no_tab() => format!("No tab is open on {name}"),
        TabState::Opening | TabState::Live => format!("The tab on {name} is still opening"),
        TabState::Down(Lost::NoWay(why)) => why.clone(),
        TabState::Down(Lost::Elsewhere(_)) => {
            format!("The tab on {name} needs this window on this {}", desktop::here())
        }
        TabState::Down(Lost::Link) => seen.map_or_else(|| format!("{name} unreachable"), MachineSeen::link_note),
        TabState::Down(Lost::Stream { .. }) => format!("{name} unreachable"),
    }
}

impl MachineTab {
    fn new(place: &MachinePlace, size: Option<(u16, u16)>) -> Self {
        Self {
            conn: Connection::unopened(size),
            session: None,
            state: TabState::Opening,
            machine: place.machine.clone(),
            boot: place.boot.clone(),
            aimed: place.clone(),
            arriving: None,
            attempts: 0,
            tries: 0,
            themed: false,
            told_visible: true,
            opened_visible: true,
            host_events: None,
        }
    }

    fn live(&self) -> bool {
        self.state == TabState::Live && self.arriving.is_none()
    }

    fn same_run(&self, boot: &str) -> bool {
        self.boot == boot
    }

    pub(crate) fn dimmed(&self) -> bool {
        self.state != TabState::Live
    }

    pub(crate) fn standing(&self, now: Instant) -> Standing {
        match &self.state {
            TabState::Down(Lost::Link) => Standing::LinkDown,
            TabState::Down(Lost::Stream { next, why, .. }) => {
                let left = next.saturating_duration_since(now);
                Standing::RetryIn(left.as_secs() + u64::from(left.subsec_nanos() > 0), why.clone())
            }
            TabState::Down(Lost::NoWay(why)) => Standing::NoWay(why.clone()),
            TabState::Down(Lost::Elsewhere(why)) => Standing::Elsewhere(why.clone()),
            _ if self.shows_no_tab() => Standing::NoTab,
            TabState::Live => Standing::Shown,
            TabState::Opening if self.conn.has_frame || self.attempts > 0 => Standing::Reconnecting,
            TabState::Opening => Standing::Connecting,
        }
    }

    fn shows_no_tab(&self) -> bool {
        let frame = self.conn.has_frame.then_some(self.conn.grid.projection).flatten();
        self.session.as_ref().is_some_and(|session| session.shows_no_tab(frame))
    }

    fn needs_steering(&self, place: &MachinePlace) -> bool {
        self.conn.view_id.is_some() && !self.aimed.shows_the_same(place)
    }

    fn steered(&mut self, place: &MachinePlace) -> bool {
        let dropped = self.drop_held();
        self.aimed = place.clone();
        self.arriving = place.tab.is_some().then(Arriving::default);
        dropped
    }

    fn hold(&mut self, event: Value, now: Instant) -> bool {
        self.arriving.as_mut().is_some_and(|arriving| arriving.hold(event, now))
    }

    fn drop_held(&mut self) -> bool {
        self.arriving.take().is_some_and(|arriving| !arriving.keys.is_empty())
    }

    fn fall(&mut self, lost: Lost) -> bool {
        self.conn.go_nowhere();
        self.host_events = None;
        self.state = TabState::Down(lost);
        self.drop_held()
    }

    fn next_wait(&mut self) -> (Duration, u32) {
        let wait = cli::backoff(self.attempts);
        self.attempts += 1;
        self.tries += 1;
        (wait, self.tries)
    }

    fn heard_frame(&mut self, tab: ThereId, now: Instant) -> bool {
        self.state = TabState::Live;
        self.attempts = 0;
        if self.aimed.tab != Some(tab) {
            return false;
        }
        let Some(arriving) = self.arriving.take() else { return false };
        if arriving.too_long(now) {
            return !arriving.keys.is_empty();
        }
        if let Some(pane) = self.conn.grid.active_pane().map(|pane| pane.pane) {
            for event in arriving.keys {
                self.conn.queue.input(pane, event);
            }
        }
        false
    }

    fn view_opened(&mut self, tx: &UnboundedSender<Msg>) {
        self.themed = true;
        self.conn.queue.cmd_with_stdin(args(&["theme", "-"]), theme::theme_json().into_bytes());
        if self.told_visible != self.opened_visible {
            self.conn.tell_visible(self.told_visible);
        }
        self.follow_host_events(tx);
    }

    fn follow_host_events(&mut self, tx: &UnboundedSender<Msg>) {
        let (serial, tx) = (self.conn.queue_serial(), tx.clone());
        let on_line = move |line: String| {
            if let Some(heard) = parse_event(&line).and_then(host_heard) {
                let _ = tx.unbounded_send(Msg::Stream(serial, heard));
            }
        };
        self.host_events =
            cli::stream_on(self.conn.target(), &args(&["events", "--host", "--json"]), on_line, |_| {}).ok();
    }

    fn reopen(&mut self, target: Target, place: &MachinePlace, tx: &UnboundedSender<Msg>) {
        self.conn.open_queue(target, tx);
        self.state = TabState::Opening;
        self.session = None;
        self.aimed = place.clone();
        self.arriving = None;
    }
}

impl Root {
    pub(crate) fn shown(&self) -> &Connection {
        self.machine.as_ref().map_or(&self.own, |tab| &tab.conn)
    }

    pub(super) fn shown_mut(&mut self) -> &mut Connection {
        match &mut self.machine {
            Some(tab) => &mut tab.conn,
            None => &mut self.own,
        }
    }

    pub(crate) fn shown_session(&self) -> Option<&Session> {
        match &self.machine {
            Some(tab) => tab.session.as_ref(),
            None => self.session.as_ref(),
        }
    }

    pub(crate) fn takes_input(&self) -> bool {
        self.frame_shown() && self.machine.as_ref().is_none_or(MachineTab::live)
    }

    pub(crate) fn input(&mut self, event: Value, cx: &mut Context<Self>) {
        let pane = self.active_pane();
        match keys_go(self.machine.as_ref(), pane) {
            KeysGo::Pane(pane) => self.shown().queue.input(pane, event),
            KeysGo::Hold => {
                if !self.machine.as_mut().is_some_and(|tab| tab.hold(event, Instant::now())) {
                    self.refuse_on_machine(cx);
                }
            }
            KeysGo::Refuse => self.refuse_on_machine(cx),
            KeysGo::Nowhere => {}
        }
    }

    pub(crate) fn refuse_on_machine(&mut self, cx: &mut Context<Self>) {
        let Some(tab) = self.machine.as_ref() else { return };
        let seen = self.session.as_ref().and_then(|session| session.machine(&tab.machine));
        let why = refusal(tab, seen);
        self.store.update(cx, |s, cx| s.not_sent(&why, cx));
    }

    pub(crate) fn machine_place_changed(
        &mut self,
        place: Option<MachinePlace>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        self.side.machine_place.clone_from(&place);
        match (plan(self.machine.as_ref(), place.as_ref()), place) {
            (Plan::Back, _) => self.back_from_machine(cx),
            (Plan::Open, Some(place)) => self.enter_machine(&place, cx),
            (Plan::Steer, Some(_)) => self.steer_machine(cx),
            _ => {}
        }
        self.activation_changed(window.is_window_active());
        cx.notify();
    }

    fn enter_machine(&mut self, place: &MachinePlace, cx: &mut Context<Self>) {
        self.drop_drags();
        self.own.pause_surface();
        if self.machine.as_ref().is_some_and(|tab| tab.machine != place.machine) {
            self.take_back_colours();
        }
        let size = self.shown().size;
        self.machine = Some(MachineTab::new(place, size));
        self.open_machine(cx);
    }

    fn back_from_machine(&mut self, cx: &mut Context<Self>) {
        self.drop_drags();
        self.take_back_colours();
        self.machine = None;
        let shown = self.session.as_ref().and_then(|session| session.shown().tab);
        self.own.has_frame = self.own.has_frame && shown == Some(self.own.grid.tab);
        self.resume_surface(shown.is_some(), cx);
    }

    pub(super) fn take_back_colours(&self) {
        if let Some(tab) = self.machine.as_ref().filter(|tab| tab.themed) {
            cli::leave(tab.conn.target());
        }
    }

    pub(super) fn take_back_colours_on_quit(cx: &mut Context<Self>) {
        cx.on_app_quit(|this, _| {
            this.take_back_colours();
            async {}
        })
        .detach();
    }

    fn machine_reach(&self, place: &MachinePlace, cx: &Context<Self>) -> Result<Target, Lost> {
        let Some(seen) = self.session.as_ref().and_then(|session| session.machine(&place.machine)) else {
            return Err(Lost::Link);
        };
        self.store.read(cx).machine_target(seen).map_err(|why| match (&self.host, seen.linked()) {
            (Some(_), _) => Lost::Elsewhere(why),
            (None, false) => Lost::Link,
            (None, true) => Lost::NoWay(why),
        })
    }

    fn open_machine(&mut self, cx: &mut Context<Self>) {
        let Some(place) = self.side.machine_place.clone() else { return };
        match self.machine_reach(&place, cx) {
            Ok(target) => self.connect_machine(target, &place, cx),
            Err(lost) => self.machine_down(lost, cx),
        }
    }

    fn connect_machine(&mut self, target: Target, place: &MachinePlace, cx: &mut Context<Self>) {
        let Some(tab) = self.machine.as_mut() else { return };
        let size = tab.conn.size.or(self.own.size).unwrap_or((120, 36));
        let seen = self.window_active && !self.had.full;
        tab.told_visible = seen;
        tab.opened_visible = seen;
        tab.reopen(target, place, &self.tx);
        if let Err(e) = tab.conn.open_view(&place.open_args(size, !seen), &self.tx) {
            self.machine_fell(&e.to_string(), cx);
        }
    }

    pub(super) fn machine_heard(&mut self, heard: Heard, cx: &mut Context<Self>) {
        match heard {
            Heard::Line(session) => self.machine_line(*session, cx),
            Heard::ViewEnded(err) => self.machine_fell(&err, cx),
            Heard::Frame(frame) => self.machine_frame(*frame, cx),
            Heard::SurfaceEnded(err) => self.machine_surface_ended(&err, cx),
            Heard::QueueError(why) => self.machine_error(&why, cx),
            Heard::FocusRefused(why) => self.machine_focus_refused(&why, cx),
            Heard::Copied(text, pane) => self.machine_copied(&text, pane, cx),
            Heard::Bell(tab) => self.machine_bell(tab, cx),
        }
    }

    fn machine_copied(&mut self, text: &str, pane: Option<u64>, cx: &mut Context<Self>) {
        let Some(tab) = self.machine.as_ref().filter(|tab| copies_here(&tab.conn.grid, pane)) else { return };
        let text = cleaned(text);
        if text.is_empty() {
            return;
        }
        let program =
            pane.and_then(|id| tab.session.as_ref()?.pane(id)).and_then(|p| p.agent.clone().or(p.program.clone()));
        let program = program.unwrap_or_else(|| "A program".to_string());
        let note = format!("{program} on {} copied to the clipboard", tab.machine);
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.store.update(cx, |s, cx| s.toast(note, ToastKind::Info, cx));
    }

    fn machine_bell(&mut self, tab: Option<u64>, cx: &mut Context<Self>) {
        if self.machine.as_ref().is_some_and(|shown| tab == Some(shown.conn.grid.tab)) {
            self.notifier.update(cx, Notifier::flash);
        }
    }

    fn machine_line(&mut self, session: Session, cx: &mut Context<Self>) {
        let Some(tab) = self.machine.as_mut() else { return };
        if !tab.same_run(&session.boot) {
            let why = format!("{} restarted", tab.machine);
            self.machine_fell(&why, cx);
            return;
        }
        let opened = tab.conn.saw_view(&session);
        let shows_tab = session.shown().tab.is_some();
        tab.session = Some(session);
        if opened {
            tab.view_opened(&self.tx);
        }
        if let Some(view) = tab.conn.surface_due(shows_tab)
            && let Err(e) = tab.conn.open_surface(view, &self.tx)
        {
            self.machine_fell(&e.to_string(), cx);
            return;
        }
        if opened {
            self.steer_machine(cx);
        }
        self.drop_drags_without_tab();
    }

    fn machine_frame(&mut self, frame: Frame, cx: &mut Context<Self>) {
        let Some(tab) = self.machine.as_mut().filter(|tab| tab.same_run(&frame.boot)) else { return };
        let shows = ThereId::heard(frame.tab);
        if tab.conn.frame_arrived(frame) && tab.heard_frame(shows, Instant::now()) {
            self.refuse_on_machine(cx);
        }
    }

    fn machine_surface_ended(&mut self, err: &str, cx: &mut Context<Self>) {
        let Some(tab) = self.machine.as_mut() else { return };
        if tab.session.as_ref().is_some_and(|session| session.shown().tab.is_none()) {
            tab.conn.pause_surface();
            return;
        }
        self.machine_fell(err, cx);
    }

    fn machine_error(&mut self, why: &str, cx: &mut Context<Self>) {
        let Some(machine) = self.machine.as_ref().map(|tab| tab.machine.clone()) else { return };
        self.store.update(cx, |s, cx| s.toast(format!("{machine}: {why}"), ToastKind::Error, cx));
    }

    fn machine_focus_refused(&mut self, why: &str, cx: &mut Context<Self>) {
        if let Some(tab) = self.machine.as_mut() {
            tab.drop_held();
        }
        self.machine_error(why, cx);
    }

    fn machine_linked(&self, machine: &str) -> bool {
        self.session.as_ref().and_then(|session| session.machine(machine)).is_some_and(MachineSeen::linked)
    }

    fn machine_fell(&mut self, err: &str, cx: &mut Context<Self>) {
        let Some(tab) = self.machine.as_mut() else { return };
        let (wait, try_number) = tab.next_wait();
        let machine = tab.machine.clone();
        if !self.machine_linked(&machine) {
            self.machine_down(Lost::Link, cx);
            return;
        }
        let lost = Lost::Stream { next: Instant::now() + wait, try_number, why: reason_of(err) };
        self.machine_down(lost, cx);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let _ = this.update(cx, |this, cx| this.machine_retry(try_number, cx));
        })
        .detach();
    }

    fn machine_retry(&mut self, try_number: u32, cx: &mut Context<Self>) {
        let due = self.machine.as_ref().is_some_and(
            |tab| matches!(&tab.state, TabState::Down(Lost::Stream { try_number: was, .. }) if *was == try_number),
        );
        if due {
            self.open_machine(cx);
            cx.notify();
        }
    }

    fn machine_down(&mut self, lost: Lost, cx: &mut Context<Self>) {
        self.drop_drags();
        let Some(tab) = self.machine.as_mut() else { return };
        if tab.fall(lost) {
            self.refuse_on_machine(cx);
        }
        cx.notify();
    }

    fn steer_machine(&mut self, cx: &mut Context<Self>) {
        let Some(place) = self.side.machine_place.clone() else { return };
        if !self.machine.as_ref().is_some_and(|tab| tab.needs_steering(&place)) {
            return;
        }
        self.drop_drags();
        let tx = self.tx.clone();
        let Some(tab) = self.machine.as_mut() else { return };
        let Some(view) = tab.conn.view_id else { return };
        let serial = tab.conn.queue_serial();
        let dropped = tab.steered(&place);
        tab.conn.queue.cmd_then(place.focus_args(view), move |answer| {
            if let Err(e) = answer {
                let _ = tx.unbounded_send(Msg::Stream(serial, Heard::FocusRefused(e.to_string())));
            }
        });
        if dropped {
            self.refuse_on_machine(cx);
        }
        cx.notify();
    }

    pub(super) fn tell_machine_visible(&mut self) {
        let seen = self.window_active && !self.had.full;
        if let Some(tab) = self.machine.as_mut().filter(|tab| tab.told_visible != seen) {
            tab.told_visible = seen;
            tab.conn.tell_visible(seen);
        }
    }

    pub(super) fn follow_link(&mut self, cx: &mut Context<Self>) {
        let Some(tab) = self.machine.as_ref() else { return };
        match link_step(&tab.state, self.machine_linked(&tab.machine)) {
            LinkStep::Stay => {}
            LinkStep::Fall => self.machine_down(Lost::Link, cx),
            LinkStep::Reopen => self.open_machine(cx),
        }
    }
}

#[cfg(test)]
mod tests;
