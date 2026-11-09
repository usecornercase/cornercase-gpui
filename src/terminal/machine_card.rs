use std::cell::Cell;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, AsyncApp, Context, Div, IntoElement as _, ParentElement as _, Styled as _, Task, WeakEntity, div,
    prelude::FluentBuilder as _, px,
};

use super::no_tab::{heading, lead_text};
use crate::app::Root;
use crate::app::machine::Standing;
use crate::desktop;
use crate::model::machines::{LinkState, NextTry};
use crate::model::{MachinePlace, MachineSeen, Tab, ThereId};
use crate::panels::{PanelKind, queue};
use crate::platform::paths;
use crate::theme::{colours, radius};
use crate::ui;

const COUNTS_EVERY: Duration = Duration::from_secs(1);

#[derive(Default)]
pub(crate) struct Counting {
    drawn: Cell<bool>,
    next: Option<Task<()>>,
}

impl Counting {
    pub(crate) fn drawn(&self) {
        self.drawn.set(true);
    }
}

async fn next_count(this: WeakEntity<Root>, cx: &mut AsyncApp) {
    cx.background_executor().timer(COUNTS_EVERY).await;
    let _ = this.update(cx, |this, cx| {
        this.counting.next = None;
        cx.notify();
    });
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct CardText {
    pub(super) title: String,
    pub(super) line: String,
    pub(super) folder: String,
    pub(super) state: Option<CardState>,
    pub(super) why: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum CardState {
    Running(String),
    Said(String),
}

pub(super) struct Told<'a> {
    pub(super) home: Option<&'a str>,
    pub(super) error: Option<&'a str>,
    pub(super) now: i64,
}

fn link_state(seen: &MachineSeen, told: &Told<'_>) -> Option<CardState> {
    match seen.next_try(told.now) {
        Some(next) if next.counts() => Some(CardState::Said(next.words())),
        Some(next) => Some(CardState::Running(next.words())),
        None if seen.state == LinkState::Connecting => Some(CardState::Running(NextTry::UnderWay.words())),
        None => told.error.map(|why| CardState::Said(why.to_string())),
    }
}

fn state_of(standing: &Standing, seen: &MachineSeen, told: &Told<'_>) -> Option<CardState> {
    match standing {
        Standing::Connecting => Some(CardState::Running("connecting…".to_string())),
        Standing::Reconnecting => Some(CardState::Running(NextTry::UnderWay.words())),
        Standing::LinkDown => link_state(seen, told),
        Standing::RetryIn(seconds, _) => Some(CardState::Said(format!("reconnecting in {seconds} s"))),
        Standing::Elsewhere(why) | Standing::NoWay(why) => Some(CardState::Said(why.clone())),
        Standing::NoTab | Standing::Shown => None,
    }
}

fn line_of(seen: &MachineSeen, agent: Option<(&str, Option<&str>)>, standing: &Standing) -> String {
    match standing {
        Standing::NoTab => return "no tab is open there".to_string(),
        Standing::Elsewhere(_) => return format!("its tab needs this window on this {}", desktop::here()),
        _ => {}
    }
    if seen.linked() {
        return agent
            .map_or_else(|| "no agent there".to_string(), |(agent, status)| format!("{agent} {}", doing(status)));
    }
    agent.map_or_else(
        || "its shells keep running there".to_string(),
        |(agent, _)| format!("{agent} keeps working there"),
    )
}

fn agent_of(tab: &Tab<ThereId>) -> Option<(&str, Option<&str>)> {
    let agents = || tab.panes.iter().filter(|pane| pane.agent.is_some());
    let pane = agents().find(|pane| pane.active).or_else(|| agents().next())?;
    Some((pane.agent.as_deref()?, pane.status.as_deref()))
}

fn doing(status: Option<&str>) -> String {
    match status {
        Some("waiting") => "is waiting for you".to_string(),
        Some("failed") => "failed".to_string(),
        Some(status) => format!("is {status}"),
        None => "is idle".to_string(),
    }
}

pub(super) fn card_text(place: &MachinePlace, seen: &MachineSeen, told: &Told<'_>, standing: &Standing) -> CardText {
    let tab = seen.workspace(place.workspace).and_then(|ws| ws.tab(place.tab));
    let agent = tab.and_then(agent_of);
    let folder = told.home.map_or_else(|| place.path().to_string(), |home| paths::home_relative(place.path(), home));
    let title = if seen.linked() { format!("runs on {}", seen.name) } else { seen.link_note() };
    let why = if let Standing::RetryIn(_, why) = standing { why.clone() } else { None };
    let state = state_of(standing, seen, told).filter(|_| seen.state != LinkState::Older);
    CardText { title, line: line_of(seen, agent, standing), folder, state, why }
}

fn card_counts(standing: &Standing, seen: &MachineSeen, now: i64) -> bool {
    match standing {
        Standing::RetryIn(..) => true,
        Standing::LinkDown => seen.next_try(now).is_some_and(NextTry::counts),
        _ => false,
    }
}

fn machine_card(text: &CardText, seen: &MachineSeen) -> Div {
    let card = ui::glass::card(radius::sheet())
        .max_w(px(460.))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(8.))
        .px(px(28.))
        .py(px(24.))
        .child(ui::emblem(seen.icon.icon()).flex_none())
        .child(heading(&text.title))
        .child(lead_text(text.line.clone()))
        .child(div().text_center().text_size(px(12.)).text_color(colours::faint()).child(text.folder.clone()))
        .when_some(text.state.as_ref(), |d, state| d.child(card_state(state)))
        .when_some(text.why.clone(), |d, why| {
            d.child(div().text_center().text_size(px(11.)).text_color(colours::faint()).child(why))
        });
    div().absolute().inset_0().flex().items_center().justify_center().p(px(24.)).child(card)
}

fn card_state(state: &CardState) -> Div {
    let line = div().text_center().text_size(px(12.)).text_color(colours::muted());
    match state {
        CardState::Running(text) => line.child(ui::busy("machine-opening", text.clone())),
        CardState::Said(text) => line.child(text.clone()),
    }
}

impl Root {
    pub(super) fn machine_tab_area(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let standing = self.machine.as_ref()?.standing(Instant::now());
        if standing == Standing::Shown {
            return None;
        }
        let place = self.side.machine_place.as_ref()?;
        let seen = self.session.as_ref()?.machine(&place.machine)?;
        let store = self.store.read(cx);
        let error = store.machines.iter().find(|machine| machine.name == seen.name).and_then(|m| m.error.as_deref());
        let told = Told { home: store.machine_home(&place.boot), error, now: queue::now() };
        let text = card_text(place, seen, &told, &standing);
        if card_counts(&standing, seen, told.now) {
            self.counting.drawn();
        }
        Some(machine_card(&text, seen).into_any_element())
    }

    pub(crate) fn keep_counting(&mut self, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let listed = store.sheet == Some(PanelKind::Machines) || store.dialog.is_some();
        let now = queue::now();
        if listed && store.machines.iter().any(|machine| machine.counts(now)) {
            self.counting.drawn();
        }
        if !self.counting.drawn.take() {
            self.counting.next = None;
        } else if self.counting.next.is_none() {
            self.counting.next = Some(cx.spawn(async move |this, cx| next_count(this, cx).await));
        }
    }
}

#[cfg(test)]
mod tests;
