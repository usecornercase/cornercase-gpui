use std::time::{Duration, Instant};

use gpui::{
    AnyElement, Div, InteractiveElement as _, IntoElement, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, div, prelude::FluentBuilder as _, px,
};

use crate::cli::{self, Target};
use crate::notify::tip;
use crate::store::{MachineRead, Reach};
use crate::theme::colours;
use crate::ui;

#[derive(Debug, Default)]
pub(crate) struct Retry {
    failures: u32,
    next: Option<Instant>,
    why: Option<String>,
}

impl Retry {
    pub(crate) fn failed(&mut self, why: String, now: Instant) -> Duration {
        self.why = Some(why);
        if let Some(next) = self.next.filter(|next| now < *next) {
            return next - now;
        }
        let wait = cli::backoff(self.failures);
        self.failures += 1;
        self.next = Some(now + wait);
        wait
    }

    pub(crate) fn heard(&mut self) -> bool {
        let failed = self.why.is_some();
        *self = Self::default();
        failed
    }

    pub(crate) fn due(&self, now: Instant) -> bool {
        self.next.is_none_or(|next| now >= next)
    }

    fn waiting(&self) -> bool {
        self.why.is_some()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Note {
    Reconnecting(String),
    Refused(String),
}

fn reconnecting(why: &str) -> Note {
    Note::Reconnecting(format!("{why} · reconnecting…"))
}

pub(crate) fn note(read: &MachineRead, retry: &Retry) -> Option<Note> {
    match &read.reach {
        Reach::Up(_) => retry.waiting().then(|| reconnecting(&format!("{} did not answer", read.aim.machine))),
        Reach::Down(why) => Some(reconnecting(why)),
        Reach::Refused(why) => Some(Note::Refused(why.clone())),
    }
}

pub(crate) fn may_ask(read: &MachineRead, retry: &Retry) -> Result<Target, String> {
    match &read.reach {
        Reach::Up(_) if retry.waiting() => Err(format!("{} did not answer", read.aim.machine)),
        Reach::Up(target) => Ok(target.clone()),
        Reach::Down(why) | Reach::Refused(why) => Err(why.clone()),
    }
}

pub(crate) fn nothing_read(read: &MachineRead) -> String {
    format!("Nothing read from {}", read.aim.machine)
}

fn where_line(read: &MachineRead) -> impl IntoElement + use<> {
    div()
        .id("on-machine")
        .flex()
        .items_center()
        .gap(px(6.))
        .tooltip(tip(read.folder.clone()))
        .child(ui::icon(read.icon.icon(), 12.).flex_none().text_color(colours::muted()))
        .child(div().flex_none().text_color(colours::text()).child(format!("on {}", read.aim.machine)))
        .child(div().flex_1().min_w_0().text_color(colours::faint()).child(ui::one_line(read.place.clone())))
        .child(div().flex_none().text_color(colours::faint()).child("read only"))
}

fn note_line(note: &Note, why: Option<&str>) -> AnyElement {
    let line = match note {
        Note::Reconnecting(text) => ui::busy("on-machine-reconnecting", text.clone()).text_size(px(11.)),
        Note::Refused(text) => div().text_color(colours::working()).child(ui::one_line(text.clone())),
    };
    let why = why.map(str::to_string);
    div().id("on-machine-note").when_some(why, |d, why| d.tooltip(tip(why))).child(line).into_any_element()
}

pub(crate) fn strip(read: &MachineRead, retry: &Retry) -> Div {
    div()
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(3.))
        .px(px(12.))
        .py(px(7.))
        .border_b_1()
        .border_color(colours::border())
        .text_size(px(11.))
        .child(where_line(read))
        .children(note(read, retry).map(|note| note_line(&note, retry.why.as_deref())))
}

#[cfg(test)]
mod tests;
