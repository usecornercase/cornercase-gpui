use gpui::{App, Context, Entity, Window};

use crate::app::Root;
use crate::keymap::Command;
use crate::panels::PanelKind;
use crate::panels::browser::Browser;

impl Root {
    fn browser(&self) -> Option<Entity<Browser>> {
        self.panels.get(&PanelKind::Browser)?.clone().downcast::<Browser>().ok()
    }

    pub(super) fn browser_has_focus(&self, window: &Window, cx: &App) -> bool {
        self.browser().is_some_and(|browser| browser.read(cx).has_focus(window, cx))
    }

    pub(super) fn browser_cancel_pick(&mut self, cx: &mut Context<Self>) -> bool {
        self.browser().is_some_and(|browser| browser.update(cx, Browser::cancel_pick))
    }

    pub(super) fn browser_escape(&mut self, window: &Window, cx: &mut Context<Self>) -> bool {
        let Some(browser) = self.browser().filter(|browser| browser.read(cx).page_focused(window)) else {
            return false;
        };
        browser.update(cx, Browser::escape);
        true
    }

    pub(super) fn browser_command(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(browser) = self.browser() else { return false };
        browser.update(cx, |browser, cx| browser.command(command, window, cx))
    }
}
