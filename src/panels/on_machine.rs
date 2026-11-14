use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, Div, InteractiveElement as _, IntoElement, ParentElement as _, Stateful,
    StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use crate::cli::{self, Target};
use crate::dialogs::{self, Question};
use crate::model::machines::NextTry;
use crate::notify::tip;
use crate::panels::queue;
use crate::platform::Os;
use crate::store::{
    AfterUpdate, MachineRead, OlderNote, Reach, Store, UpdateOffer, UpdateThere, restart_note, update_question,
};
use crate::theme::colours;
use crate::ui;
use crate::ui::counting::Counting;

#[derive(Debug, Default)]
pub(crate) struct Retry {
    failures: u32,
    next: Option<Instant>,
    why: Option<String>,
    restarted: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Missed {
    Restarted,
    Failed(String),
}

impl Retry {
    pub(crate) fn restarted(&mut self) {
        self.restarted = true;
        self.next = None;
        self.why = None;
    }

    pub(crate) fn missed(&mut self, missed: Missed, now: Instant) {
        match missed {
            Missed::Restarted => self.restarted(),
            Missed::Failed(why) => {
                self.failed(why, now);
            }
        }
    }

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
        if self.restarted {
            return false;
        }
        let failed = self.why.is_some();
        *self = Self::default();
        failed
    }

    pub(crate) fn forget_while_older(&mut self, reach: &Reach) {
        if matches!(reach, Reach::Older(_)) {
            *self = Self::default();
        }
    }

    pub(crate) fn due(&self, now: Instant) -> bool {
        !self.restarted && self.next.is_none_or(|next| now >= next)
    }

    pub(crate) fn refused(&self) -> bool {
        self.restarted
    }

    fn waiting(&self) -> bool {
        self.why.is_some()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Note {
    Reconnecting { why: String, next: NextTry },
    Waiting(String),
    Older(OlderNote),
    Refused(String),
    Restarted(String),
}

impl Note {
    fn text(&self) -> String {
        match self {
            Self::Reconnecting { why, next } => format!("{why} · {}", next.words()),
            Self::Waiting(why) => format!("{why}…"),
            Self::Older(note) => note.line.clone(),
            Self::Refused(why) | Self::Restarted(why) => why.clone(),
        }
    }

    fn counts(&self) -> bool {
        matches!(self, Self::Reconnecting { next, .. } if next.counts())
    }
}

fn did_not_answer(read: &MachineRead) -> String {
    format!("{} did not answer", read.aim.machine)
}

fn note_at(read: &MachineRead, retry: &Retry, now: i64) -> Option<Note> {
    match &read.reach {
        Reach::Up(_) if retry.restarted => Some(Note::Restarted(restart_note(&read.aim.machine))),
        Reach::Up(_) => {
            retry.waiting().then(|| Note::Reconnecting { why: did_not_answer(read), next: NextTry::UnderWay })
        }
        Reach::Down(why) => {
            Some(Note::Reconnecting { why: why.clone(), next: read.next_try(now).unwrap_or(NextTry::UnderWay) })
        }
        Reach::Waiting(why) => Some(Note::Waiting(why.clone())),
        Reach::Older(note) => Some(Note::Older(note.clone())),
        Reach::Refused(why) => Some(Note::Refused(why.clone())),
    }
}

pub(crate) fn note(read: &MachineRead, retry: &Retry) -> Option<Note> {
    note_at(read, retry, queue::now())
}

pub(crate) fn may_ask(read: &MachineRead, retry: &Retry) -> Result<Target, String> {
    match &read.reach {
        Reach::Up(_) if retry.restarted => Err(restart_note(&read.aim.machine)),
        Reach::Up(_) if retry.waiting() => Err(did_not_answer(read)),
        Reach::Up(target) => Ok(target.clone()),
        Reach::Older(note) => Err(note.line.clone()),
        Reach::Waiting(why) | Reach::Down(why) | Reach::Refused(why) => Err(why.clone()),
    }
}

pub(crate) fn nothing_read(read: &MachineRead) -> String {
    format!("Nothing read from {}", read.aim.machine)
}

fn where_line(read: &MachineRead, read_only: bool) -> impl IntoElement + use<> {
    div()
        .id("on-machine")
        .flex()
        .items_center()
        .gap(px(6.))
        .tooltip(tip(read.folder.clone()))
        .child(ui::icon(read.icon.icon(), 12.).flex_none().text_color(colours::muted()))
        .child(div().flex_none().text_color(colours::text()).child(format!("on {}", read.aim.machine)))
        .child(div().flex_1().min_w_0().text_color(colours::faint()).child(ui::one_line(read.place.clone())))
        .when(read_only, |d| d.child(div().flex_none().text_color(colours::faint()).child("read only")))
}

fn note_line(note: &Note, why: Option<&str>) -> AnyElement {
    let text = note.text();
    let line = match note {
        Note::Refused(_) | Note::Older(_) | Note::Restarted(_) => {
            div().text_color(colours::working()).child(ui::one_line(text))
        }
        _ if note.counts() => div().text_color(colours::muted()).child(ui::one_line(text)),
        Note::Reconnecting { .. } | Note::Waiting(_) => ui::busy("on-machine-reconnecting", text).text_size(px(11.)),
    };
    let why = match note {
        Note::Older(older) => Some(older.tip.clone()),
        _ => why.map(str::to_string),
    };
    div().id("on-machine-note").when_some(why, |d, why| d.tooltip(tip(why))).child(line).into_any_element()
}

pub(crate) fn ask_update_there(update: &UpdateThere, window: &mut Window, cx: &mut App) {
    let (title, message, button) = update_question(&update.machine, AfterUpdate::Reads, Os::CURRENT);
    let update = update.clone();
    let question = Question::new(title, message, button, Vec::new())
        .on_yes(move |cx| Store::get(cx).update(cx, |store, cx| store.update_on_machine(update.clone(), cx)));
    dialogs::confirm(question, window, cx);
}

pub(crate) fn update_button(offer: &UpdateOffer) -> Option<Stateful<Div>> {
    match offer {
        UpdateOffer::Hidden => None,
        UpdateOffer::Running(label) => Some(ui::busy_cut_button("update-there", label.clone())),
        UpdateOffer::Ask(update) => {
            let update = update.clone();
            Some(
                ui::cut_button("update-there", update.label())
                    .on_click(move |_, window, cx| ask_update_there(&update, window, cx)),
            )
        }
    }
}

pub(crate) fn strip(
    read: &MachineRead,
    retry: &Retry,
    counting: &Counting,
    (read_only, update): (bool, &UpdateOffer),
) -> Div {
    let note = note(read, retry);
    if note.as_ref().is_some_and(Note::counts) {
        counting.drawn();
    }
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
        .child(where_line(read, read_only))
        .children(note.map(|note| note_line(&note, retry.why.as_deref())))
        .children(update_button(update).map(|button| div().flex().min_w_0().child(button)))
}

#[cfg(test)]
mod tests;
