use gpui::{App, Context, Entity, Keystroke, Window};

use crate::app::Root;
use crate::panels::PanelKind;
use crate::panels::changes::ChangesPanel;
use crate::panels::files::FilesPanel;
use crate::panels::queue::QueuePanel;
use crate::panels::todo::TodoPanel;
use crate::ui::selection::Step;

impl Root {
    pub(super) fn files_panel(&self) -> Option<Entity<FilesPanel>> {
        self.panels.get(&PanelKind::Files)?.clone().downcast::<FilesPanel>().ok()
    }

    pub(super) fn todo_panel(&self) -> Option<Entity<TodoPanel>> {
        self.panels.get(&PanelKind::Todo)?.clone().downcast::<TodoPanel>().ok()
    }

    fn queue_panel(&self) -> Option<Entity<QueuePanel>> {
        self.panels.get(&PanelKind::Queue)?.clone().downcast::<QueuePanel>().ok()
    }

    pub(super) fn changes_panel(&self) -> Option<Entity<ChangesPanel>> {
        self.panels.get(&PanelKind::Changes)?.clone().downcast::<ChangesPanel>().ok()
    }

    fn focused_dock(&self, window: &Window, cx: &App) -> Option<PanelKind> {
        self.store.read(cx).shown_dock().filter(|_| !self.overlay_open(cx) && self.dock_focused(window, cx))
    }

    pub(super) fn viewer_has_focus(&self, window: &Window, cx: &App) -> bool {
        self.focused_dock(window, cx) == Some(PanelKind::Files)
            && self.files_panel().is_some_and(|files| files.read(cx).can_find())
    }

    pub(super) fn find_in_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(files) = self.files_panel() {
            files.update(cx, |files, cx| files.open_find(window, cx));
        }
    }

    pub(super) fn dock_select(&mut self, ks: &Keystroke, window: &Window, cx: &mut Context<Self>) -> bool {
        let Some(step) = Step::of(ks) else { return false };
        let card = self.dock_focus.is_focused(window);
        match self.focused_dock(window, cx) {
            Some(PanelKind::Files) => self.files_panel().is_some_and(|files| {
                (card || files.read(cx).has_focus_itself(window))
                    && files.update(cx, |files, cx| files.extend_selection(step, cx))
            }),
            Some(PanelKind::Changes) => {
                card && self
                    .changes_panel()
                    .is_some_and(|changes| changes.update(cx, |changes, cx| changes.extend_selection(step, cx)))
            }
            _ => false,
        }
    }

    pub(super) fn dock_copy(&mut self, window: &Window, cx: &mut Context<Self>) -> bool {
        match self.focused_dock(window, cx) {
            Some(PanelKind::Files) => {
                self.files_panel().is_some_and(|files| files.update(cx, FilesPanel::copy_selection))
            }
            Some(PanelKind::Changes) => {
                self.changes_panel().is_some_and(|changes| changes.update(cx, ChangesPanel::copy_selection))
            }
            _ => false,
        }
    }

    pub(super) fn sheet_copy(&mut self, cx: &mut Context<Self>) -> bool {
        let store = self.store.read(cx);
        if store.dialog.is_some() || store.sheet != Some(PanelKind::Queue) {
            return false;
        }
        self.queue_panel().is_some_and(|queue| queue.update(cx, QueuePanel::copy_selection))
    }
}
