use gpui::{App, ClickEvent, Context, Div, IntoElement, SharedString, StatefulInteractiveElement as _};

use super::{BotCard, listed, section};
use crate::cli::{Feature, Has};
use crate::model::bots::{StartedRow, started_rows};
use crate::store::Store;
use crate::ui::{self, press};

const TITLE: &str = "Agents it started";
const NONE: &str = "No agent it started is open.";
const SHOW: &str = "Show";
const PREFIX: &str = "bot-card-agent-";

fn stop_name(pane: u64) -> SharedString {
    format!("{PREFIX}{pane}").into()
}

pub(super) fn started(store: &Store, bot: &str) -> Option<Vec<StartedRow>> {
    let session = store.session.as_ref()?;
    (store.server_has(Feature::BotFollow) == Has::Yes).then(|| started_rows(session, bot))
}

pub(super) fn started_stops(rows: &[StartedRow]) -> Vec<SharedString> {
    rows.iter().map(|started| stop_name(started.row.id)).collect()
}

fn show_pressed(event: &ClickEvent, name: &str, pane: u64, cx: &mut App) {
    if press::pressed(event, name) {
        Store::get(cx).update(cx, |s, cx| {
            s.close_dialog(cx);
            s.show_in_terminal("pane", pane, cx);
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
                let (pane, name) = (started.row.id, stop_name(started.row.id));
                let pressed = name.clone();
                let show = ui::button(name.clone(), SHOW)
                    .on_click(move |event, _, cx| show_pressed(event, &pressed, pane, cx));
                let lead = ui::badge(started.status.as_deref()).into_any_element();
                listed(&started.row, Some(lead), Some(self.card_stop(show, &name).into_any_element()))
                    .into_any_element()
            })
            .collect();
        section(TITLE, lines, NONE, None)
    }
}
