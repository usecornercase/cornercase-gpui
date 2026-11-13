use std::collections::BTreeMap;

use gpui::App;

use super::data::Place;
use super::names;
use crate::model::link::is_web;
use crate::model::{PaneInfo, Session};
use crate::store::{Store, ToastKind};
use crate::ui::markdown::text;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Go {
    Pane(u64),
    Workspace(u64),
    Project(u64),
    Address(String),
    Nowhere,
}

pub(super) fn go(place: &Place, session: Option<&Session>) -> Go {
    if place.is_pull() {
        return place.url.clone().filter(|url| is_web(url)).map_or(Go::Nowhere, Go::Address);
    }
    session
        .and_then(|session| {
            pane_in_conversation(place, session)
                .or_else(|| live_pane(place, session))
                .or_else(|| where_it_was(place, session))
        })
        .unwrap_or(Go::Nowhere)
}

fn pane_in_conversation(place: &Place, session: &Session) -> Option<Go> {
    let (agent, conversation) = (place.agent.as_deref()?, place.conversation.as_deref()?);
    let holds = |pane: &&PaneInfo| {
        pane.conversation.as_deref() == Some(conversation) && pane.agent.as_deref().is_none_or(|a| a == agent)
    };
    session.tabs().flat_map(|tab| &tab.panes).find(holds).map(|pane| Go::Pane(pane.id))
}

fn live_pane(place: &Place, session: &Session) -> Option<Go> {
    let pane = place.pane.filter(|_| place.boot.as_deref() == Some(session.boot.as_str()))?;
    session.pane(pane).map(|_| Go::Pane(pane))
}

fn where_it_was(place: &Place, session: &Session) -> Option<Go> {
    let path = &place.project.as_ref()?.path;
    let project = session.projects.iter().find(|project| project.path == *path)?;
    let named = place.workspace.as_ref();
    let workspace = named.and_then(|named| project.workspaces.iter().find(|w| w.path == named.path));
    Some(workspace.map_or(Go::Project(project.id), |workspace| Go::Workspace(workspace.id)))
}

pub(super) fn out_of_reach(name: &str, place: &Place) -> String {
    if place.is_pull() {
        format!("{name} cannot be reached: it has no web address")
    } else {
        format!("{name} is closed")
    }
}

pub(super) fn follow(link: &str, places: &BTreeMap<String, Place>, cx: &mut App) {
    match places.get(link) {
        Some(place) => reach(link, place, cx),
        None => text::open_web(link, cx),
    }
}

fn reach(id: &str, place: &Place, cx: &mut App) {
    let store = Store::get(cx);
    let (kind, shown) = match go(place, store.read(cx).session.as_ref()) {
        Go::Address(url) => return cx.open_url(&url),
        Go::Nowhere => return tell(out_of_reach(&names::label(id, place), place), cx),
        Go::Pane(pane) => ("pane", pane),
        Go::Workspace(workspace) => ("workspace", workspace),
        Go::Project(project) => ("project", project),
    };
    store.update(cx, |s, cx| s.show_in_terminal(kind, shown, cx));
}

fn tell(text: String, cx: &mut App) {
    Store::get(cx).update(cx, |s, cx| s.toast(text, ToastKind::Info, cx));
}

#[cfg(test)]
mod tests;
