use gpui::{Context, Window};

use super::TextField;
use crate::platform::Os;

#[cfg(target_os = "macos")]
mod carbon;
#[cfg(not(target_os = "macos"))]
#[path = "secure/no_carbon.rs"]
mod carbon;

fn guards_keys(os: Os, masked: bool) -> bool {
    os == Os::Mac && masked
}

fn guarded(os: Os, masked: bool, focused: bool, front: bool) -> bool {
    guards_keys(os, masked) && focused && front
}

struct Guard(());

impl Guard {
    fn take() -> Option<Self> {
        carbon::enable().then_some(Self(()))
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        carbon::disable();
    }
}

#[derive(Default)]
pub(super) struct Secure {
    watched: bool,
    focused: bool,
    guard: Option<Guard>,
}

impl TextField {
    pub(super) fn watch_keys(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !guards_keys(Os::CURRENT, self.options.masked) || self.secure.watched {
            return;
        }
        self.secure.watched = true;
        let handle = self.focus_handle.clone();
        cx.on_focus(&handle, window, |field, window, _| field.keys_focused(true, window)).detach();
        cx.on_blur(&handle, window, |field, window, _| field.keys_focused(false, window)).detach();
        cx.observe_window_activation(window, |field, window, _| field.guard_keys(window)).detach();
        self.keys_focused(handle.is_focused(window), window);
    }

    fn keys_focused(&mut self, focused: bool, window: &Window) {
        self.secure.focused = focused;
        self.guard_keys(window);
    }

    fn guard_keys(&mut self, window: &Window) {
        let wanted = guarded(Os::CURRENT, self.options.masked, self.secure.focused, window.is_window_active());
        if wanted != self.secure.guard.is_some() {
            self.secure.guard = if wanted { Guard::take() } else { None };
        }
    }
}

#[cfg(test)]
mod tests;
