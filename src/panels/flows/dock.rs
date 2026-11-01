use gpui::{AnyView, AppContext as _, Context, Div, ParentElement as _, Styled as _, Window, div, px};

use super::model::Run;
use super::widgets::error_text;
use super::{FlowsPanel, Place, Selected};
use crate::store::Store;
use crate::ui;

pub(super) fn shown_run(runs: &[Run], workspace: Option<u64>) -> Option<u64> {
    let workspace = workspace?;
    runs.iter().find(|r| r.workspace == workspace).map(|r| r.id)
}

impl FlowsPanel {
    pub(super) fn follow_shown(&mut self, cx: &mut Context<Self>) {
        let workspace = Store::get(cx).read(cx).session.as_ref().and_then(|s| s.shown().workspace);
        match shown_run(&self.runs, workspace) {
            Some(id) => self.select(Selected::Run(id), cx),
            None => self.clear_selection(cx),
        }
    }

    fn clear_selection(&mut self, cx: &mut Context<Self>) {
        if self.selected.take().is_some() {
            self.files = None;
            self.sync_feed(cx);
            cx.notify();
        }
    }

    pub(super) fn render_dock(&mut self, cx: &mut Context<Self>) -> Div {
        self.sync_run_list();
        if self.selected_run().is_some() {
            return div().size_full().child(self.render_run(cx));
        }
        match &self.runs_error {
            Some(error) => div().p(px(20.)).child(error_text(error.clone())),
            None => ui::loading("Reading the run…"),
        }
    }
}

pub(crate) fn build_dock(_window: &mut Window, cx: &mut gpui::App) -> AnyView {
    cx.new(|cx| FlowsPanel::new(Place::Dock, cx)).into()
}

#[cfg(test)]
mod tests;
