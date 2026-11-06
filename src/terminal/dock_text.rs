use gpui::{App, Context, Entity, Window};

use crate::app::Root;
use crate::panels::PanelKind;
use crate::panels::changes::ChangesPanel;
use crate::panels::files::FilesPanel;
use crate::store::Store;

impl Root {
    fn files_panel(&self) -> Option<Entity<FilesPanel>> {
        self.panels.get(&PanelKind::Files)?.clone().downcast::<FilesPanel>().ok()
    }

    fn changes_panel(&self) -> Option<Entity<ChangesPanel>> {
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

    pub(super) fn dock_escape(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let answered = match self.focused_dock(window, cx) {
            Some(PanelKind::Files) => self.files_panel().is_some_and(|files| files.update(cx, FilesPanel::escape)),
            Some(PanelKind::Changes) => {
                self.changes_panel().is_some_and(|changes| changes.update(cx, ChangesPanel::escape))
            }
            _ => false,
        };
        let answered = answered || self.store.update(cx, Store::leave_file_full_screen);
        if answered {
            window.focus(&self.dock_focus);
        }
        answered
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
}
