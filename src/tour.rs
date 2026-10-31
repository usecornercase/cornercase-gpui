mod card;
mod catalogue;
pub(crate) mod model;

use gpui::{App, AppContext as _, Context, Entity, Global};

use crate::panels::PanelKind;
use crate::store::{self, Store, args};
use model::{Catalogue, State, state_of};

pub(crate) use catalogue::open as open_catalogue;

pub(crate) struct Guide {
    store: Entity<Store>,
    catalogue: Catalogue,
    opened: Option<PanelKind>,
    reported: Option<String>,
}

struct GlobalGuide(Entity<Guide>);
impl Global for GlobalGuide {}

fn is_open(store: &Store, kind: PanelKind) -> bool {
    store.dock == Some(kind) || store.sheet == Some(kind)
}

impl Guide {
    pub(crate) fn get(cx: &mut App) -> Entity<Guide> {
        let store = Store::get(cx);
        if let Some(GlobalGuide(guide)) = cx.try_global::<GlobalGuide>()
            && guide.read(cx).store == store
        {
            return guide.clone();
        }
        let guide = cx.new(|cx| Guide::new(store, cx));
        cx.set_global(GlobalGuide(guide.clone()));
        guide
    }

    fn new(store: Entity<Store>, cx: &mut Context<Self>) -> Self {
        cx.observe(&store, |this, _, cx| this.changed(cx)).detach();
        Self::load(cx);
        Self { store, catalogue: Catalogue::default(), opened: None, reported: None }
    }

    pub(crate) fn pointed(cx: &mut App) -> Option<PanelKind> {
        let guide = Self::get(cx);
        guide.read(cx).catalogue.current().and_then(|(_, _, step)| step.points_at())
    }

    fn load(cx: &mut Context<Self>) {
        store::run_json(cx, args(&["tour", "list", "--json"]), |this, result, cx| {
            if let Ok(catalogue) = serde_json::from_value::<Catalogue>(result.unwrap_or_default()) {
                this.catalogue.tours = catalogue.tours;
                this.step_changed(cx);
            }
            cx.notify();
        });
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        let state: State = state_of(self.store.read(cx).app.as_ref());
        if state != self.catalogue.state {
            let started = state.active.is_some() && self.catalogue.state.active.is_none();
            self.catalogue.state = state;
            if started && self.catalogue.tours.is_empty() {
                Self::load(cx);
            }
            self.step_changed(cx);
            cx.notify();
        }
        self.report_done(cx);
    }

    fn step_changed(&mut self, cx: &mut Context<Self>) {
        let wanted = self.catalogue.current().and_then(|(_, _, step)| step.shows());
        if wanted == self.opened {
            return;
        }
        let (opened, store) = (self.opened.take(), self.store.clone());
        store.update(cx, |s, cx| {
            if let Some(kind) = opened.filter(|k| is_open(s, *k)) {
                s.toggle(kind, cx);
            }
        });
        let Some(kind) = wanted else { return };
        let free = {
            let s = store.read(cx);
            if kind.docked() { s.dock.is_none() } else { s.sheet.is_none() && s.dialog.is_none() }
        };
        if free {
            store.update(cx, |s, cx| s.open(kind, cx));
            self.opened = Some(kind);
        }
    }

    fn report_done(&mut self, cx: &mut Context<Self>) {
        let Some((tour, _, step)) = self.catalogue.current() else { return };
        let key = format!("{}/{}", tour.id, step.id);
        let open = step.completed_by().is_some_and(|kind| is_open(self.store.read(cx), kind));
        if open && self.reported.as_ref() != Some(&key) {
            let command = args(&["tour", "complete", &tour.id, &step.id]);
            self.reported = Some(key);
            self.store.read(cx).cmd(command);
        }
    }

    fn command(&self, words: &[&str], cx: &App) {
        self.store.read(cx).cmd(args(words));
    }

    pub(crate) fn start(&mut self, tour: &str, cx: &mut Context<Self>) {
        self.command(&["tour", "start", tour, "--no-card"], cx);
        if self.catalogue.tours.is_empty() {
            Self::load(cx);
        }
    }
}

#[cfg(test)]
mod tests;
