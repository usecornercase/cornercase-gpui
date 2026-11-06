use std::collections::BTreeMap;

use gpui::App;

use super::data::Place;
use super::names;
use crate::model::Session;
use crate::model::link::is_web;
use crate::store::{Store, ToastKind};

const MOVED_ISSUE: &str = "usecornercase/cornercase-terminal#664";

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
        .and_then(|session| live_pane(place, session).or_else(|| where_it_was(place, session)))
        .unwrap_or(Go::Nowhere)
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

pub(super) fn left_its_pane(name: &str, what: &str) -> String {
    format!("{name} is not in its pane any more: showing its {what}. Finding a pane it moved to needs {MOVED_ISSUE}")
}

pub(super) fn out_of_reach(name: &str, place: &Place) -> String {
    let why = if place.is_pull() { "it has no web address" } else { "it is closed" };
    format!("{name} cannot be reached: {why}")
}

pub(super) fn follow(link: &str, places: &BTreeMap<String, Place>, cx: &mut App) {
    match places.get(link) {
        Some(place) => reach(link, place, cx),
        None => open(link, cx),
    }
}

fn reach(id: &str, place: &Place, cx: &mut App) {
    let store = Store::get(cx);
    let name = names::label(id, place);
    let (kind, shown, note) = match go(place, store.read(cx).session.as_ref()) {
        Go::Address(url) => return cx.open_url(&url),
        Go::Nowhere => return tell(out_of_reach(&name, place), cx),
        Go::Pane(pane) => ("pane", pane, None),
        Go::Workspace(workspace) => ("workspace", workspace, Some(left_its_pane(&name, "workspace"))),
        Go::Project(project) => ("project", project, Some(left_its_pane(&name, "project"))),
    };
    store.update(cx, |s, cx| s.show_in_terminal(kind, shown, cx));
    if let Some(note) = note {
        tell(note, cx);
    }
}

fn open(address: &str, cx: &mut App) {
    if is_web(address) {
        cx.open_url(address);
    } else {
        tell(format!("{address} is not a web address, so it cannot be opened"), cx);
    }
}

fn tell(text: String, cx: &mut App) {
    Store::get(cx).update(cx, |s, cx| s.toast(text, ToastKind::Info, cx));
}

#[cfg(test)]
mod tests;
