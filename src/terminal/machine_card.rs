use std::time::Instant;

use gpui::{
    AnyElement, Context, Div, IntoElement as _, ParentElement as _, Stateful, Styled as _, div,
    prelude::FluentBuilder as _, px,
};

use super::no_tab::{heading, lead_text, scrolled};
use crate::app::Root;
use crate::app::machine::Standing;
use crate::desktop;
use crate::model::machines::{LinkState, NextTry};
use crate::model::{MachinePlace, MachineSeen, Tab, ThereId};
use crate::panels::on_machine::update_button;
use crate::panels::{PanelKind, queue};
use crate::platform::paths;
use crate::store::{UpdateOffer, restart_note};
use crate::theme::{colours, radius};
use crate::ui;
use crate::ui::counting::Counting;

fn counting(root: &mut Root) -> &mut Counting {
    &mut root.counting
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct CardText {
    pub(super) title: String,
    pub(super) line: String,
    pub(super) folder: String,
    pub(super) state: Option<CardState>,
    pub(super) why: Option<String>,
    pub(super) update: UpdateOffer,
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
    pub(super) update: UpdateOffer,
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
        Standing::Waiting(why) => Some(CardState::Running(format!("{why}…"))),
        Standing::Older(note) => Some(CardState::Said(note.line.clone())),
        Standing::Restarted => Some(CardState::Said(restart_note(&seen.name))),
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
    let why = match standing {
        Standing::RetryIn(_, why) => why.clone(),
        Standing::Older(note) => Some(note.tip.clone()),
        _ => None,
    };
    let state = state_of(standing, seen, told).filter(|_| seen.state != LinkState::Older);
    let update = if matches!(standing, Standing::Older(_)) { told.update.clone() } else { UpdateOffer::Hidden };
    CardText { title, line: line_of(seen, agent, standing), folder, state, why, update }
}

fn card_counts(standing: &Standing, seen: &MachineSeen, now: i64) -> bool {
    match standing {
        Standing::RetryIn(..) => true,
        Standing::LinkDown => seen.next_try(now).is_some_and(NextTry::counts),
        _ => false,
    }
}

fn machine_card(text: &CardText, seen: &MachineSeen) -> Stateful<Div> {
    let faint =
        |size: f32, text: String| div().text_center().text_size(px(size)).text_color(colours::faint()).child(text);
    let card = ui::glass::card(radius::sheet())
        .w_full()
        .max_w(px(460.))
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(8.))
        .px(px(28.))
        .py(px(24.))
        .child(div().flex().justify_center().child(ui::emblem(seen.icon.icon()).flex_none()))
        .child(heading(&text.title))
        .child(lead_text(text.line.clone()))
        .child(faint(12., text.folder.clone()))
        .when_some(text.state.as_ref(), |d, state| d.child(card_state(state)))
        .when_some(text.why.clone(), |d, why| d.child(faint(11., why)))
        .children(update_button(&text.update).map(|button| div().flex().justify_center().min_w_0().child(button)));
    scrolled("machine-card", div().flex().justify_center().p(px(24.)).child(card))
}

fn card_state(state: &CardState) -> Div {
    let line = div().text_center().text_size(px(12.)).text_color(colours::muted());
    match state {
        CardState::Running(text) => {
            line.flex().justify_center().child(ui::busy("machine-opening", text.clone()).min_w_0())
        }
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
        let update = match &standing {
            Standing::Older(note) => store.update_offer(Some(note)),
            _ => UpdateOffer::Hidden,
        };
        let told = Told { home: store.machine_home(&place.machine, &place.boot), error, now: queue::now(), update };
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
        self.counting.keep(counting, cx);
    }
}

#[cfg(test)]
mod tests;
