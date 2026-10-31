use std::path::PathBuf;

use gpui::{AppContext as _, Context};

use crate::model::{Project, Session, Workspace};

use super::served::Listed;
use super::source::Source;
use super::{FilesPanel, Place, tree};

pub(super) fn shown_workspace(session: Option<&Session>) -> Option<(&Project, &Workspace)> {
    let session = session?;
    session.workspace(session.shown().workspace?)
}

impl Place {
    fn apply_listing(&mut self, rel: PathBuf, listing: std::io::Result<tree::Listing>) -> bool {
        let root = rel.as_os_str().is_empty();
        match listing {
            Ok(listing) => {
                let cleared = root && self.error.take().is_some();
                if self.listings.get(&rel) == Some(&listing) {
                    return cleared;
                }
                self.listings.insert(rel, listing);
                true
            }
            Err(e) if root => {
                self.error = Some(e.to_string());
                true
            }
            Err(_) => self.listings.remove(&rel).is_some() | self.expanded.remove(&rel),
        }
    }
}

impl FilesPanel {
    pub(super) fn follow_workspace(&mut self, cx: &mut Context<Self>) {
        let shown =
            shown_workspace(self.store.read(cx).session.as_ref()).map(|(_, ws)| (ws.id, PathBuf::from(&ws.path)));
        let Some((id, root)) = shown else {
            if self.current.take().is_some() {
                cx.notify();
            }
            return;
        };
        if self.current == Some(id) && self.places.get(&id).is_some_and(|p| p.root == root) {
            return;
        }
        self.switch_to(id, root, cx);
    }

    fn switch_to(&mut self, id: u64, root: PathBuf, cx: &mut Context<Self>) {
        let old_query = self.query.read(cx).text();
        if let Some(place) = self.current.and_then(|id| self.places.get_mut(&id)) {
            place.query.clone_from(&old_query);
        }
        if self.places.get(&id).is_some_and(|p| p.root != root) {
            self.places.remove(&id);
        }
        self.current = Some(id);
        self.menu = None;
        let source = Source::new(id, root.clone(), self.remote);
        let place = self.places.entry(id).or_insert_with(|| Place::new(root, source));
        let query = place.query.clone();
        self.search.index = None;
        self.search.clear();
        if query != old_query {
            self.query.update(cx, |f, cx| f.set_text(query, cx));
        } else if !query.trim().is_empty() {
            self.run_search(cx);
        }
        self.relist(cx);
        cx.notify();
    }

    pub(super) fn tick(&mut self, list: bool, cx: &mut Context<Self>) {
        if !self.showing(cx) {
            return;
        }
        if list {
            self.relist(cx);
        }
        self.check_file(cx);
    }

    pub(super) fn relist(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.current else { return };
        let Some(place) = self.places.get_mut(&id) else { return };
        if place.listing {
            return;
        }
        place.listing = true;
        let folders = tree::shown_folders(&place.expanded);
        self.list(id, folders, cx);
    }

    pub(super) fn list(&mut self, id: u64, folders: Vec<PathBuf>, cx: &mut Context<Self>) {
        let Some(place) = self.places.get(&id) else { return };
        let source = place.source.clone();
        let job = cx.background_spawn(async move { source.list(folders) });
        cx.spawn(async move |this, cx| {
            let listed = job.await;
            let _ = this.update(cx, |this, cx| this.apply_listings(id, listed, cx));
        })
        .detach();
    }

    fn apply_listings(&mut self, id: u64, listed: Listed, cx: &mut Context<Self>) {
        let Some(place) = self.places.get_mut(&id) else { return };
        place.listing = false;
        let mut changed = false;
        for (rel, listing) in listed {
            changed |= place.apply_listing(rel, listing);
        }
        if changed {
            cx.notify();
        }
    }

    pub(super) fn click_row(&mut self, rel: PathBuf, dir: bool, cx: &mut Context<Self>) {
        self.menu = None;
        if !dir {
            self.open_file(rel, super::viewer::OpenAt::default(), cx);
            return;
        }
        let Some(id) = self.current else { return };
        let Some(place) = self.places.get_mut(&id) else { return };
        if !place.expanded.remove(&rel) {
            place.expanded.insert(rel.clone());
            self.list(id, vec![rel], cx);
        }
        cx.notify();
    }

    pub(super) fn collapse_all(&mut self, cx: &mut Context<Self>) {
        if let Some(place) = self.place_mut() {
            place.expanded.clear();
        }
        cx.notify();
    }
}
