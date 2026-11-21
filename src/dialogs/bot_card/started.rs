use gpui::{App, ClickEvent, Context, Div, IntoElement, SharedString, StatefulInteractiveElement as _};

use super::{BotCard, listed, section};
use crate::cli::{Feature, Has};
use crate::model::bots::{BotKey, ShowAt, StartedRow, started_rows, started_rows_there};
use crate::store::Store;
use crate::ui::{self, press};

const TITLE: &str = "Agents it started";
const NONE: &str = "No agent it started is open.";
const SHOW: &str = "Show";
const PREFIX: &str = "bot-card-agent-";

fn stop_name(stop: &str) -> SharedString {
    format!("{PREFIX}{stop}").into()
}

pub(super) fn started(store: &Store, key: &BotKey) -> Option<Vec<StartedRow>> {
    let session = store.session.as_ref()?;
    let follows = |has: Has| (has == Has::Yes).then_some(());
    follows(store.server_has(Feature::BotFollow))?;
    let Some(machine) = key.home.machine() else { return Some(started_rows(session, &key.id)) };
    follows(Feature::BotFollow.on(store.bot_releases(&key.home).server))?;
    session.machine(machine).map(|seen| started_rows_there(seen, &key.id))
}

pub(super) fn started_stops(rows: &[StartedRow]) -> Vec<SharedString> {
    rows.iter().map(|started| stop_name(&started.stop)).collect()
}

fn show_pressed(event: &ClickEvent, name: &str, show: &ShowAt, cx: &mut App) {
    if press::pressed(event, name) {
        Store::get(cx).update(cx, |s, cx| {
            s.close_dialog(cx);
            match show {
                ShowAt::Pane(pane) => s.show_in_terminal("pane", *pane, cx),
                ShowAt::There(place) => s.pick_machine_place((**place).clone(), cx),
            }
        });
    }
}

impl BotCard {
    pub(super) fn keep_started_stops(&mut self, rows: &[StartedRow], cx: &mut Context<Self>) {
        let names = started_stops(rows);
        self.stops.retain(|name, _| !name.starts_with(PREFIX) || names.contains(name));
        for name in names {
            self.stops.entry(name).or_insert_with(|| cx.focus_handle());
        }
    }

    pub(super) fn started_part(&self, rows: &[StartedRow]) -> Div {
        let lines = rows
            .iter()
            .map(|started| {
                let (name, show) = (stop_name(&started.stop), started.show.clone());
                let pressed = name.clone();
                let button = ui::button(name.clone(), SHOW)
                    .on_click(move |event, _, cx| show_pressed(event, &pressed, &show, cx));
                let lead = ui::badge(started.status.as_deref()).into_any_element();
                listed(&started.row, Some(lead), Some(self.card_stop(button, &name).into_any_element()))
                    .into_any_element()
            })
            .collect();
        section(TITLE, lines, NONE, None)
    }
}
