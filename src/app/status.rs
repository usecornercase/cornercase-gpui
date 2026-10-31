use std::rc::Rc;
use std::time::Instant;

use gpui::{
    AnyElement, ClickEvent, Context, EntityId, InteractiveElement as _, IntoElement, ParentElement as _, Rgba,
    StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use super::{Conn, GAP, Root, tilde};
use crate::dialogs;
use crate::keymap::{Command, with_keys};
use crate::model::Session;
use crate::notify;
use crate::theme::{colours, radius};
use crate::ui::{self, motion};

impl Root {
    pub(super) fn render_status(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let items = self.notifier.read(cx).status_items(&self.notifier);
        let jobs = self.store.read(cx).jobs.summary(Instant::now());
        let separator = || div().w(px(1.)).h(px(12.)).flex_none().bg(colours::edge());
        div()
            .h(px(28.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(10.))
            .pl(px(6.))
            .pr(px(GAP + 4.))
            .text_size(px(11.))
            .text_color(colours::faint())
            .child(self.connection_status())
            .child(div().flex_1().text_color(colours::muted()).child(ui::one_line(self.active_pane_summary())))
            .when_some(jobs, |d, jobs| {
                d.child(ui::busy("jobs", jobs).flex_none().max_w(px(360.)).text_size(px(11.))).child(separator())
            })
            .when(!items.is_empty(), |d| {
                d.child(div().flex_none().flex().items_center().gap(px(2.)).children(items)).child(separator())
            })
            .child(div().flex_none().child(self.size.map(|(c, r)| format!("{c}×{r}")).unwrap_or_default()))
    }

    fn connection_status(&self) -> impl IntoElement + use<> {
        let (dot, label, why) = self.connection_label();
        let tip = why.unwrap_or_else(|| with_keys("Connect to another server", Command::Connect));
        div()
            .id("conn")
            .flex_none()
            .max_w(px(320.))
            .flex()
            .items_center()
            .gap(px(6.))
            .px(px(6.))
            .py(px(2.))
            .rounded(px(radius::CHIP))
            .cursor_pointer()
            .hover(|s| s.bg(colours::hover()))
            .child(connection_dot(dot, self.conn == Conn::Starting))
            .child(div().min_w_0().child(ui::one_line(label)))
            .tooltip(notify::tip(tip))
            .on_click(|_: &ClickEvent, window, cx| dialogs::connect(window, cx))
    }

    fn connection_label(&self) -> (Rgba, String, Option<String>) {
        let version = self.session.as_ref().map(|s| format!("cornercase {}", s.version)).unwrap_or_default();
        let (dot, state, why) = match &self.conn {
            Conn::Starting => (colours::working(), "connecting…".to_string(), None),
            Conn::Connected => (colours::done(), version, None),
            Conn::Lost(why) => (colours::failed(), "disconnected".to_string(), Some(why.clone())),
        };
        let label = match &self.host {
            Some(host) => format!("{host} · {state}"),
            None => state,
        };
        (dot, label, why)
    }

    pub(super) fn keep_beating(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.beat.drawing(Instant::now());
        if self.session.as_ref().is_some_and(Session::anyone_working) && self.beat.start() {
            follow_frames(window, cx.entity_id(), self.beat.clone());
        }
    }

    fn active_pane_summary(&self) -> String {
        let pane = self.active_pane().and_then(|id| self.session.as_ref()?.pane(id).cloned());
        pane.map(|p| {
            let program = p.agent.or(p.program).unwrap_or_else(|| "shell".into());
            match p.path {
                Some(path) => format!("{program} · {}", tilde(&path)),
                None => program,
            }
        })
        .unwrap_or_default()
    }
}

fn follow_frames(window: &Window, root: EntityId, beat: Rc<motion::Beat>) {
    window.on_next_frame(move |window, cx| match beat.tick(Instant::now(), motion::orbits_drawn()) {
        motion::Next::Wait => follow_frames(window, root, beat),
        motion::Next::Draw => cx.notify(root),
        motion::Next::Rest => {}
    });
}

fn connection_dot(colour: Rgba, waiting: bool) -> AnyElement {
    let dot = div().size(px(7.)).rounded_full().bg(colour).shadow(ui::glass::glow(colour, 0.9, 6.));
    if waiting { motion::pulse(dot, "conn-pulse").into_any_element() } else { dot.into_any_element() }
}
