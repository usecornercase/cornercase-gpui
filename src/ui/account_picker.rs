use std::collections::BTreeMap;

use gpui::{App, AppContext as _, Context, Div, Entity, ParentElement as _, ScrollHandle, Styled as _, div, px};

pub(crate) use super::picker::reveals;
use super::picker::{PickRow, Pickable, Picked, Picker};
use crate::model::accounts::pick::{AccountPick, AgentChoices, Picking};

type AccountPicker = Picker<Picking>;

impl Pickable for Picking {
    fn key(&self) -> String {
        format!("account-{}", self.agent())
    }

    fn rows(&self) -> Vec<PickRow> {
        Picking::rows(self)
    }

    fn shown(&self) -> PickRow {
        Picking::shown(self)
    }

    fn gone(&self) -> Option<String> {
        Picking::gone(self)
    }

    fn choose(&mut self, folder: &str) {
        Picking::choose(self, folder);
    }

    fn chosen_at(&self) -> Option<usize> {
        Picking::chosen_at(self)
    }
}

#[derive(Default)]
pub(crate) struct AccountPickers {
    saved: BTreeMap<String, String>,
    pickers: Vec<Entity<AccountPicker>>,
    scroller: Option<ScrollHandle>,
}

impl AccountPickers {
    pub(crate) fn keeping(saved: BTreeMap<String, String>) -> Self {
        Self { saved, pickers: Vec::new(), scroller: None }
    }

    pub(crate) fn inside(mut self, scroller: ScrollHandle) -> Self {
        self.scroller = Some(scroller);
        self
    }

    pub(crate) fn follow<T: 'static>(
        &mut self,
        list: Vec<AgentChoices>,
        on_pick: fn(&mut T, &mut Context<T>),
        cx: &mut Context<T>,
    ) {
        let old = std::mem::take(&mut self.pickers);
        self.pickers = list.into_iter().map(|choices| self.picker_for(choices, &old, on_pick, cx)).collect();
    }

    fn picker_for<T: 'static>(
        &self,
        choices: AgentChoices,
        old: &[Entity<AccountPicker>],
        on_pick: fn(&mut T, &mut Context<T>),
        cx: &mut Context<T>,
    ) -> Entity<AccountPicker> {
        if let Some(picker) = old.iter().find(|picker| picker.read(cx).picking.agent() == choices.agent) {
            picker.update(cx, |picker, cx| picker.follow(|picking| picking.follow(choices), cx));
            return picker.clone();
        }
        let saved = self.saved.get(&choices.agent).map(String::as_str);
        let picking = Picking::new(choices, saved);
        let scroller = self.scroller.clone();
        let picker = cx.new(|cx| AccountPicker::new(picking, cx).inside(scroller));
        cx.subscribe(&picker, move |this, _, _: &Picked, cx| on_pick(this, cx)).detach();
        picker
    }

    fn of<'a>(&'a self, agents: &'a [String], cx: &'a App) -> impl Iterator<Item = &'a Entity<AccountPicker>> {
        self.pickers.iter().filter(move |picker| agents.iter().any(|agent| agent == picker.read(cx).picking.agent()))
    }

    pub(crate) fn picks(&self, agents: &[String], cx: &App) -> Vec<AccountPick> {
        self.of(agents, cx).filter_map(|picker| picker.read(cx).picking.pick()).collect()
    }

    pub(crate) fn follow_in_use(&self, agents: &[String], cx: &App) -> bool {
        self.of(agents, cx).all(|picker| picker.read(cx).picking.follows_in_use())
    }

    pub(crate) fn fields(&self, agents: &[String], cx: &App) -> Option<Div> {
        let fields: Vec<Div> = self
            .of(agents, cx)
            .map(|picker| {
                let label = format!("{} account", picker.read(cx).picking.title());
                div().flex_1().min_w(px(200.)).child(super::field(label, picker.clone()))
            })
            .collect();
        (!fields.is_empty()).then(|| div().flex().flex_wrap().gap(px(10.)).children(fields))
    }
}
