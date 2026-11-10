use gpui::{AnyElement, Context, InteractiveElement as _, IntoElement as _, MouseButton, Styled as _, Window, div};

use super::{Conn, Root, frame};
use crate::appearance::{self, Appearance};
use crate::store::{ToastKind, args};
use crate::theme;

#[derive(Default)]
pub(super) struct Given(Option<String>);

impl Given {
    pub(super) fn of(json: String) -> Given {
        Given(Some(json))
    }

    pub(super) fn needs(&self, json: &str) -> bool {
        self.0.as_deref() != Some(json)
    }

    pub(super) fn gave(&mut self, json: String) {
        self.0 = Some(json);
    }

    pub(super) fn forget(&mut self) {
        self.0 = None;
    }
}

impl Root {
    pub(super) fn follow_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let appearance = Appearance::get(cx);
        cx.observe(&appearance, |this, appearance, cx| this.appearance_changed(&appearance, cx)).detach();
        cx.observe_window_appearance(window, |_, window, cx| {
            let dark = appearance::dark(window.appearance());
            Appearance::get(cx).update(cx, |this, cx| this.system_changed(dark, cx));
        })
        .detach();
        let dark = appearance::dark(window.appearance());
        appearance.update(cx, |this, cx| this.system_changed(dark, cx));
        self.appearance_changed(&appearance, cx);
    }

    fn appearance_changed(&mut self, appearance: &gpui::Entity<Appearance>, cx: &mut Context<Self>) {
        let (notices, settled) = appearance.update(cx, |this, _| (this.take_notices(), this.settled()));
        for notice in notices {
            self.store.update(cx, |store, cx| store.toast(notice, ToastKind::Error, cx));
        }
        if settled != self.palette_settled {
            self.palette_settled = settled;
            self.give_palette();
        }
        cx.notify();
    }

    pub(super) fn give_palette(&mut self) {
        let json = theme::terminal_json();
        if !self.given.needs(&json) {
            return;
        }
        let tab = self.machine.as_ref().filter(|tab| tab.themed);
        let own = takes_palette(&self.conn);
        if !own && tab.is_none() {
            return;
        }
        if own {
            self.own.queue.cmd_with_stdin(args(&["theme", "-"]), json.clone().into_bytes());
        }
        if let Some(tab) = tab {
            tab.conn.queue.cmd_with_stdin(args(&["theme", "-"]), json.clone().into_bytes());
        }
        self.given.gave(json);
    }

    pub(super) fn sheet_or_peek(
        &mut self,
        kind: crate::panels::PanelKind,
        window_frame: frame::Frame,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !self.store.read(cx).peeking {
            return self.render_sheet(kind, window_frame, window, cx).into_any_element();
        }
        let layer = div().absolute().inset_0().occlude().on_mouse_up(MouseButton::Left, |_, _, cx| {
            crate::store::Store::get(cx).update(cx, |store, cx| store.peek(false, cx));
        });
        frame::round(layer, window_frame).into_any_element()
    }
}

fn takes_palette(conn: &Conn) -> bool {
    matches!(conn, Conn::Connected)
}

#[cfg(test)]
mod tests;
