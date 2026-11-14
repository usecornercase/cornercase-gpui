use std::time::Duration;

use gpui::Context;

use super::{Root, first_line};
use crate::model::Session;

impl Root {
    fn open_own_surface(&mut self, view: u64, cx: &mut Context<Self>) {
        if let Err(e) = self.own.open_surface(view, None, &self.tx) {
            self.say(&e.to_string(), cx);
        }
    }

    pub(super) fn follow_surface(&mut self, session: &Session, cx: &mut Context<Self>) {
        self.own.saw_view(session);
        self.resume_surface(session.shown().tab.is_some(), cx);
    }

    pub(super) fn resume_surface(&mut self, shows_tab: bool, cx: &mut Context<Self>) {
        if self.machine.is_none()
            && let Some(view) = self.own.surface_due(shows_tab)
        {
            self.open_own_surface(view, cx);
        }
    }

    pub(super) fn surface_ended(&mut self, serial: u64, err: &str, cx: &mut Context<Self>) {
        self.own.has_frame = false;
        if self.session.as_ref().is_some_and(|s| s.shown().tab.is_none()) {
            self.own.pause_surface();
            return;
        }
        let err = first_line(err.trim());
        let repeated = !err.is_empty() && self.own.surface_error.as_deref() == Some(err);
        if !err.is_empty() && !repeated {
            self.say(err, cx);
        }
        self.own.surface_error = (!err.is_empty()).then(|| err.to_string());
        let wait = if repeated { 5000 } else { 400 };
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(wait)).await;
            let _ = this.update(cx, |this, cx| this.reopen_surface(serial, cx));
        })
        .detach();
    }

    fn reopen_surface(&mut self, serial: u64, cx: &mut Context<Self>) {
        if self.own.owns(serial)
            && self.own.surface_open()
            && let Some(id) = self.own.view_id
        {
            self.open_own_surface(id, cx);
            cx.notify();
        }
    }
}
