use gpui::{App, Context, Entity, Window};

use crate::app::Root;
use crate::escape::{Focus, Scene, Step, Top};
use crate::panels::PanelKind;
use crate::panels::browser::Browser;
use crate::panels::changes::ChangesPanel;
use crate::panels::files::FilesPanel;
use crate::panels::todo::TodoPanel;
use crate::store::Store;

enum Shown {
    Browser(Entity<Browser>),
    Files(Entity<FilesPanel>),
    Changes(Entity<ChangesPanel>),
    Todo(Entity<TodoPanel>),
}

impl Shown {
    fn tops(&self, cx: &App) -> Vec<Top> {
        match self {
            Self::Browser(browser) => browser.read(cx).tops(),
            Self::Files(files) => files.read(cx).tops(),
            Self::Changes(changes) => changes.read(cx).tops(),
            Self::Todo(todo) => todo.read(cx).tops(),
        }
    }

    fn close(&self, top: Top, cx: &mut App) {
        match self {
            Self::Browser(browser) => browser.update(cx, |browser, cx| browser.close_top(top, cx)),
            Self::Files(files) => files.update(cx, |files, cx| files.close_top(top, cx)),
            Self::Changes(changes) => changes.update(cx, |changes, cx| changes.close_top(top, cx)),
            Self::Todo(todo) => todo.update(cx, |todo, cx| todo.close_top(top, cx)),
        }
    }

    fn holds_the_card_focus(&self) -> bool {
        matches!(self, Self::Files(_) | Self::Changes(_))
    }
}

impl Root {
    pub(super) fn escape(&mut self, terminal: bool, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let step = self.escape_scene(terminal, window, cx).step();
        self.take_step(step, window, cx)
    }

    fn escape_scene(&self, terminal: bool, window: &Window, cx: &App) -> Scene {
        let store = self.store.read(cx);
        Scene {
            focus: self.escape_focus(terminal, window, cx),
            drag: self.moves.borrow().pressed().is_some(),
            dialog: store.dialog.is_some(),
            sheet: store.sheet.is_some(),
            tops: self.shown_panel(cx).map(|shown| shown.tops(cx)).unwrap_or_default(),
            full_screen: store.shows_file_full_screen(),
        }
    }

    fn escape_focus(&self, terminal: bool, window: &Window, cx: &App) -> Focus {
        if terminal {
            Focus::Terminal
        } else if self.browser().is_some_and(|browser| browser.read(cx).page_focused(window)) {
            Focus::Page
        } else if self.dock_focused(window, cx) {
            Focus::Dock
        } else {
            Focus::Elsewhere
        }
    }

    fn shown_panel(&self, cx: &App) -> Option<Shown> {
        match self.store.read(cx).shown_dock()? {
            PanelKind::Browser => self.browser().map(Shown::Browser),
            PanelKind::Files => self.files_panel().map(Shown::Files),
            PanelKind::Changes => self.changes_panel().map(Shown::Changes),
            PanelKind::Todo => self.todo_panel().map(Shown::Todo),
            _ => None,
        }
    }

    fn take_step(&mut self, step: Step, window: &mut Window, cx: &mut Context<Self>) -> bool {
        match step {
            Step::CancelDrag => {
                self.cancel_pane_drag(cx);
            }
            Step::CloseDialog | Step::CloseSheet => {
                self.store.update(cx, Store::close_overlay);
            }
            Step::Close(top) => self.close_top(top, window, cx),
            Step::LeaveFullScreen => {
                self.store.update(cx, Store::leave_file_full_screen);
                window.focus(&self.dock_focus);
            }
            Step::ToPage => {
                if let Some(browser) = self.browser() {
                    browser.update(cx, |browser, _| browser.escape());
                }
            }
            Step::ToProgram => return false,
            Step::KeysBack => window.focus(&self.focus),
        }
        true
    }

    fn close_top(&mut self, top: Top, window: &mut Window, cx: &mut Context<Self>) {
        let Some(shown) = self.shown_panel(cx) else { return };
        shown.close(top, cx);
        if shown.holds_the_card_focus() {
            window.focus(&self.dock_focus);
        }
    }
}
