use std::time::Instant;

use gpui::{
    AnyElement, ClickEvent, Context, Div, InteractiveElement as _, IntoElement, ParentElement as _, Rgba,
    StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use super::part::{self, Drawn};
use super::{Conn, GAP, Root};
use crate::cli;
use crate::dialogs;
use crate::keymap::{Command, with_keys};
use crate::model::tunnels::Way;
use crate::notify;
use crate::platform::Os;
use crate::platform::paths;
use crate::self_update;
use crate::terminal;
use crate::theme::{colours, radius};
use crate::ui::{self, motion};

pub(super) mod key;
mod tunnels;

const HEIGHT: f32 = 28.;
const HOST_MAX: f32 = 150.;

impl Root {
    pub(super) fn status_part(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> Drawn {
        Drawn { element: self.render_status(cx).into_any_element(), counted: false }
    }

    pub(super) fn status_slot(&self) -> impl IntoElement + use<> {
        div().h(px(HEIGHT)).flex_none().child(part::cached(&self.parts.status))
    }

    fn render_status(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let items = self.notifier.read(cx).status_items(&self.notifier, cx);
        let jobs = self.store.read(cx).jobs.summary(Instant::now());
        let separator = || div().w(px(1.)).h(px(12.)).flex_none().bg(colours::edge());
        div()
            .size_full()
            .flex()
            .items_center()
            .gap(px(10.))
            .pl(px(6.))
            .pr(px(GAP + 4.))
            .text_size(px(11.))
            .text_color(colours::faint())
            .child(self.connection_status(cx))
            .child(div().flex_1().text_color(colours::muted()).child(ui::one_line(self.active_pane_summary())))
            .when_some(jobs, |d, jobs| {
                d.child(ui::busy("jobs", jobs).flex_none().max_w(px(360.)).text_size(px(11.))).child(separator())
            })
            .when(!items.is_empty(), |d| {
                d.child(div().flex_none().flex().items_center().gap(px(2.)).children(items)).child(separator())
            })
            .child(div().flex_none().child(self.shown().size.map(|(c, r)| format!("{c}×{r}")).unwrap_or_default()))
    }

    fn connection_status(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let (dot, label, why) = self.connection_label();
        let store = self.store.read(cx);
        let menu_open = self.side.menu.is_some();
        let held = store.tunnel_way() == Way::Cornercase && self.host.is_some();
        let tip = connection_tip(why, self.host.as_deref().filter(|_| held));
        let parts = label_parts(self.host.as_deref(), if held { store.tunnel_count() } else { 0 }, &label);
        div()
            .id("conn")
            .flex_none()
            .max_w(px(320.))
            .flex()
            .items_center()
            .gap(px(6.))
            .px(px(6.))
            .py(px(2.))
            .rounded(px(radius::chip()))
            .cursor_pointer()
            .hover(|s| s.bg(colours::hover()))
            .child(connection_dot(dot, self.conn == Conn::Starting))
            .child(label_element(parts))
            .when(!menu_open, |d| d.tooltip(notify::tip(tip)))
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                if held {
                    this.open_machine_menu(e.position(), window, cx);
                } else {
                    dialogs::connect(window, cx);
                }
            }))
    }

    fn connection_label(&self) -> (Rgba, String, Option<String>) {
        let version = self.session.as_ref().map(|s| format!("cornercase {}", s.version)).unwrap_or_default();
        let (state, why) = conn_words(&self.conn, self.came_up, &version);
        let dot = match &self.conn {
            Conn::Starting => colours::working(),
            Conn::Connected => colours::done(),
            Conn::Older(_) | Conn::Behind(_) => colours::waiting(),
            Conn::Lost(_) | Conn::Missing(_) => colours::failed(),
        };
        (dot, state, why)
    }

    fn active_pane_summary(&self) -> String {
        let session = self.shown_session();
        let pane = self.active_pane().and_then(|id| session?.pane(id).cloned());
        let there = self.machine.as_ref().map(|tab| format!("{} · ", tab.machine)).unwrap_or_default();
        let folder = |path: &str| match session.and_then(|s| s.home.as_deref()).filter(|_| self.machine.is_some()) {
            Some(home) => paths::home_relative(path, home),
            None => ui::home_relative(path),
        };
        pane.map(|p| {
            let program = p.agent.or(p.program).unwrap_or_else(|| "shell".into());
            match p.path {
                Some(path) => format!("{there}{program} · {}", folder(&path)),
                None => format!("{there}{program}"),
            }
        })
        .unwrap_or_default()
    }
}

#[derive(Debug, PartialEq, Eq)]
struct LabelParts {
    host: Option<String>,
    count: Option<String>,
    state: String,
}

fn label_parts(host: Option<&str>, tunnels: usize, state: &str) -> LabelParts {
    let count = (host.is_some() && tunnels > 0).then(|| format!("⇄{tunnels}"));
    LabelParts { host: host.map(str::to_string), count, state: state.to_string() }
}

fn label_element(parts: LabelParts) -> Div {
    let row = div().min_w_0().flex().items_center().gap(px(4.));
    let Some(host) = parts.host else { return row.child(ui::one_line(parts.state)) };
    row.child(div().min_w_0().max_w(px(HOST_MAX)).child(ui::one_line(host)))
        .children(parts.count.map(|count| div().flex_none().text_color(colours::accent()).child(count)))
        .child(div().flex_none().child("·"))
        .child(div().min_w_0().child(ui::one_line(parts.state)))
}

fn connection_tip(why: Option<String>, tunnels_to: Option<&str>) -> String {
    let connect = with_keys("connecting to another server", Command::Connect);
    let lead = why.unwrap_or_else(|| match tunnels_to {
        Some(host) => format!("Tunnels to {host}, and {connect}"),
        None => with_keys("Connect to another server", Command::Connect),
    });
    format!("{lead}\nThis window is {}", self_update::version_line())
}

fn conn_words(conn: &Conn, came_up: bool, version: &str) -> (String, Option<String>) {
    match conn {
        Conn::Starting => ("connecting…".to_string(), None),
        Conn::Connected => (version.to_string(), None),
        Conn::Lost(why) => {
            let state = if came_up { "disconnected" } else { "not connected" };
            (state.to_string(), Some(cli::reason_of(why)))
        }
        Conn::Missing(missing) => (format!("cornercase {}", missing.label()), None),
        Conn::Older(older) => {
            let state = older.there.as_ref().map_or_else(|| "older".to_string(), |there| format!("older · {there}"));
            (state, Some(terminal::older_lead(older, Os::CURRENT)))
        }
        Conn::Behind(behind) => {
            (format!("older server · {}", behind.server), Some(terminal::behind_lead(behind, Os::CURRENT)))
        }
    }
}

fn connection_dot(colour: Rgba, waiting: bool) -> AnyElement {
    let dot = div().size(px(7.)).rounded_full().bg(colour).shadow(ui::glass::glow(colour, 0.9, 6.));
    if waiting { motion::pulse(dot, "conn-pulse").into_any_element() } else { dot.into_any_element() }
}

#[cfg(test)]
mod tests;
