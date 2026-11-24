use gpui::{AppContext as _, Context, Entity, ScrollHandle};

use super::FlowEditor;
use crate::panels::flows::library::Library;
use crate::ui::picker::{PickRow, Pickable, Picked, Picker};

pub(in crate::panels::flows) const GONE: &str = "not among your personas";
const NONE: &str = "none";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(in crate::panels::flows) struct PersonaPicking {
    key: String,
    known: Option<Vec<(String, String)>>,
    chosen: String,
}

impl PersonaPicking {
    pub(in crate::panels::flows) fn new(key: String, chosen: &str, library: Option<&Library>) -> Self {
        Self { key, known: library.map(known), chosen: chosen.trim().to_string() }
    }

    pub(in crate::panels::flows) fn chosen(&self) -> &str {
        &self.chosen
    }

    pub(in crate::panels::flows) fn follow(&mut self, library: &Library) -> bool {
        let known = Some(known(library));
        let changed = self.known != known;
        self.known = known;
        changed
    }

    fn row(&self, id: &str, name: String) -> PickRow {
        PickRow { folder: id.to_string(), name, note: id.to_string(), chosen: self.chosen == id }
    }
}

fn known(library: &Library) -> Vec<(String, String)> {
    library.personas.iter().map(|persona| (persona.id.clone(), persona.label())).collect()
}

impl Pickable for PersonaPicking {
    fn key(&self) -> String {
        format!("persona-{}", self.key)
    }

    fn rows(&self) -> Vec<PickRow> {
        let none =
            PickRow { folder: String::new(), name: NONE.into(), note: String::new(), chosen: self.chosen.is_empty() };
        let known = self.known.iter().flatten().map(|(id, label)| self.row(id, label.clone()));
        std::iter::once(none).chain(known).collect()
    }

    fn shown(&self) -> PickRow {
        self.rows().into_iter().find(|row| row.chosen).unwrap_or_else(|| self.row(&self.chosen, self.chosen.clone()))
    }

    fn gone(&self) -> Option<String> {
        let listed = self.known.as_ref()?.iter().any(|(id, _)| *id == self.chosen);
        (!self.chosen.is_empty() && !listed).then(|| format!("{}: {GONE}", self.chosen))
    }

    fn choose(&mut self, folder: &str) {
        self.chosen = folder.to_string();
    }

    fn chosen_at(&self) -> Option<usize> {
        self.rows().iter().position(|row| row.chosen)
    }
}

pub(in crate::panels::flows) type PersonaPicker = Picker<PersonaPicking>;

pub(super) fn picker(
    key: String,
    chosen: &str,
    library: Option<&Library>,
    scroller: &ScrollHandle,
    cx: &mut Context<FlowEditor>,
) -> Entity<PersonaPicker> {
    let picking = PersonaPicking::new(key, chosen, library);
    let scroller = Some(scroller.clone());
    let picker = cx.new(|cx| Picker::new(picking, cx).inside(scroller));
    cx.subscribe(&picker, |this: &mut FlowEditor, _, _: &Picked, cx| this.changed(cx)).detach();
    picker
}

#[cfg(test)]
mod tests;
