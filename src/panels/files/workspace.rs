use std::path::PathBuf;

use gpui::{AppContext as _, Context};

use crate::model::{Project, Session, Workspace};
use crate::store::Store;

use super::served::Listed;
use super::source::Source;
use super::there::is_lost;
use super::{FilesPanel, Place, tree};

pub(super) fn shown_workspace(session: Option<&Session>) -> Option<(&Project, &Workspace)> {
    let session = session?;
    session.workspace(session.shown().workspace?)
}

impl Place {
    pub(super) fn apply_listing(&mut self, rel: PathBuf, listing: std::io::Result<tree::Listing>) -> bool {
        let root = rel.as_os_str().is_empty();
        match listing {
            Err(e) if self.source.is_there() && is_lost(&e) => false,
            Ok(listing) => {
                let cleared = root && self.error.take().is_some();
                if self.source.lists_marks() {
                    tree::keep_marks(&mut self.marks, &rel, &listing);
                }
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
    pub(super) fn store_changed(&mut self, cx: &mut Context<Self>) {
        self.follow_workspace(cx);
        if self.showing(cx) {
            self.take_run_file(cx);
            self.list_machine_on_opening(cx);
        } else {
            self.forget_flow();
            self.store.update(cx, Store::leave_file_full_screen);
        }
    }

    pub(super) fn follow_workspace(&mut self, cx: &mut Context<Self>) {
        let read = self.store.read(cx).machine_read();
        if read != self.machine {
            self.machine.clone_from(&read);
            cx.notify();
        }
        if let Some(read) = &read {
            self.follow_machine(read, cx);
            return;
        }
        self.leave_machine(cx);
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
        let source = Source::new(id, root.clone(), self.remote);
        self.switch_to(id, root, source, cx);
    }

    pub(super) fn switch_to(&mut self, id: u64, root: PathBuf, source: Source, cx: &mut Context<Self>) {
        let old_query = self.query.read(cx).text();
        if let Some(place) = self.current.and_then(|id| self.places.get_mut(&id)) {
            place.query.clone_from(&old_query);
        }
        if self.places.get(&id).is_some_and(|p| p.root != root) {
            self.places.remove(&id);
        }
        self.current = Some(id);
        self.menu = None;
        self.left_place = true;
        let place = self.places.entry(id).or_insert_with(|| Place::new(root, source));
        let query = place.query.clone();
        self.search.index = None;
        self.search.forget();
        if query != old_query {
            self.query.update(cx, |f, cx| f.set_text(query, cx));
        } else if !query.trim().is_empty() {
            self.run_search(cx);
        }
        self.relist(cx);
        cx.notify();
    }

    pub(super) fn tick(&mut self, list: bool, again: bool, cx: &mut Context<Self>) {
        if !self.showing(cx) {
            return;
        }
        if list {
            self.relist(cx);
        }
        if let Some(id) = self.current.filter(|_| again) {
            self.follow_changes(id, true, cx);
        }
        self.check_file(cx);
    }

    pub(super) fn relist(&mut self, cx: &mut Context<Self>) {
        if !self.may_poll(cx) {
            return;
        }
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
        let (source, marked) = (place.source.clone(), folders.clone());
        let job = cx.background_spawn(async move { source.list(folders) });
        cx.spawn(async move |this, cx| {
            let listed = job.await;
            let _ = this.update(cx, |this, cx| this.apply_listings(id, listed, cx));
        })
        .detach();
        self.read_marks(id, marked, cx);
    }

    fn apply_listings(&mut self, id: u64, listed: Listed, cx: &mut Context<Self>) {
        if self.current == Some(id) {
            self.machine_answered(&listed, cx);
        }
        let Some(place) = self.places.get_mut(&id) else { return };
        place.listing = false;
        let mut changed = false;
        for (rel, listing) in listed {
            changed |= place.apply_listing(rel, listing);
        }
        if changed {
            cx.notify();
        }
        self.follow_changes(id, false, cx);
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
            self.open_folder(id, rel, cx);
        }
        cx.notify();
    }

    fn open_folder(&mut self, id: u64, rel: PathBuf, cx: &mut Context<Self>) {
        if self.refuses_asking(cx).is_some() {
            return;
        }
        if let Some(place) = self.places.get_mut(&id) {
            place.expanded.insert(rel.clone());
        }
        self.list(id, vec![rel], cx);
    }

    pub(super) fn collapse_all(&mut self, cx: &mut Context<Self>) {
        if let Some(place) = self.place_mut() {
            place.expanded.clear();
        }
        cx.notify();
    }
}
