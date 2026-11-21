use std::time::{Duration, Instant};

use gpui::{ClipboardItem, Context};

use super::{Store, ToastGo};

pub(crate) const COPY_COMMAND: &str = "Copy command";

const KEPT: usize = 5;
const PAST_END: Duration = Duration::from_millis(100);
const MEANT: Duration = Duration::from_millis(300);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Shown {
    banked: Duration,
    since: Option<Instant>,
    once: bool,
}

impl Shown {
    pub(crate) fn so_far(self, now: Instant) -> Duration {
        self.banked + self.since.map_or(Duration::ZERO, |since| now.saturating_duration_since(since))
    }

    pub(crate) fn drawn(self) -> bool {
        self.since.is_some()
    }

    pub(crate) fn ever(self) -> bool {
        self.once
    }

    pub(crate) fn resume(&mut self, now: Instant) -> bool {
        let starts = self.since.is_none();
        if starts {
            self.since = Some(now);
            self.once = true;
        }
        starts
    }

    pub(crate) fn pause(&mut self, now: Instant) {
        if self.since.is_some() {
            self.banked = self.so_far(now);
            self.since = None;
        }
    }
}

pub(crate) type ToastAction = Option<(String, ToastGo)>;

#[derive(Clone)]
pub(crate) struct Toast {
    pub id: u64,
    pub text: String,
    pub kind: ToastKind,
    pub action: ToastAction,
    pub shown: Shown,
    pub count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToastKind {
    Info,
    Error,
    NeedsYou,
    Done,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Clip {
    pub text: String,
    pub said: String,
}

impl Clip {
    pub(crate) fn command(command: String, place: Option<&str>) -> Self {
        let said = match place {
            Some(place) => format!("Copied: run it on {place}"),
            None => "Copied: run it in a terminal".to_string(),
        };
        Self { text: command, said }
    }
}

impl Toast {
    pub(crate) fn lasts(&self) -> Duration {
        Duration::from_secs(match self.kind {
            ToastKind::Info => 4,
            ToastKind::Error => 8,
            ToastKind::Done | ToastKind::NeedsYou => 15,
        })
    }

    pub(crate) fn expired(&self) -> bool {
        self.expired_at(Instant::now())
    }

    pub(crate) fn expired_at(&self, now: Instant) -> bool {
        self.shown.so_far(now) > self.lasts()
    }

    pub(crate) fn left(&self) -> Duration {
        self.lasts().saturating_sub(self.shown.banked)
    }

    fn repeats(&self, other: &Toast) -> bool {
        self.kind == other.kind && self.text == other.text && self.action == other.action && !self.expired()
    }
}

pub(crate) fn drawn(toasts: &mut [Toast], ids: &[u64], now: Instant) -> Vec<Duration> {
    let mut starting = Vec::new();
    for toast in toasts {
        if !ids.contains(&toast.id) {
            toast.shown.pause(now);
        } else if toast.shown.resume(now) {
            starting.push(toast.left());
        }
    }
    starting
}

fn making_way(older: &[Toast]) -> usize {
    older
        .iter()
        .position(|t| t.shown.ever())
        .or_else(|| older.iter().position(|t| t.kind == ToastKind::Info))
        .unwrap_or(0)
}

pub(crate) fn shows_bot(toast: &Toast, id: &str) -> bool {
    matches!(&toast.action, Some((_, ToastGo::Bot(bot, _))) if bot == id)
}

pub(crate) fn newest_meant(toasts: &[Toast], now: Instant) -> Option<u64> {
    toasts.iter().rev().find(|t| t.shown.drawn() && !t.expired_at(now) && t.shown.so_far(now) >= MEANT).map(|t| t.id)
}

pub(crate) fn push_toast(toasts: &mut Vec<Toast>, mut toast: Toast) {
    if let Some(at) = toasts.iter().position(|t| t.repeats(&toast)) {
        toast.count += toasts.remove(at).count;
    }
    toasts.push(toast);
    if toasts.len() > KEPT {
        let gone = making_way(&toasts[..toasts.len() - 1]);
        toasts.remove(gone);
    }
}

impl Store {
    pub(crate) fn drop_bot_toasts(&mut self, id: &str, cx: &mut Context<Self>) {
        let before = self.toasts.len();
        self.toasts.retain(|toast| !shows_bot(toast, id));
        if self.toasts.len() != before {
            cx.notify();
        }
    }

    pub(crate) fn copy(&mut self, clip: &Clip, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(clip.text.clone()));
        self.toast(clip.said.clone(), ToastKind::Info, cx);
    }

    pub(crate) fn toast_with(
        &mut self,
        text: impl Into<String>,
        kind: ToastKind,
        action: ToastAction,
        cx: &mut Context<Self>,
    ) {
        let id = self.next_toast;
        self.next_toast += 1;
        let shown = Shown::default();
        push_toast(&mut self.toasts, Toast { id, text: text.into(), kind, action, shown, count: 1 });
        cx.notify();
    }

    pub(crate) fn take_newest_toast(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = newest_meant(&self.toasts, Instant::now()) {
            self.dismiss_toast(id, cx);
        }
    }

    pub(crate) fn toasts_drawn(&mut self, ids: &[u64], cx: &mut Context<Self>) {
        for left in drawn(&mut self.toasts, ids, Instant::now()) {
            Self::sweep_after(left + PAST_END, cx);
        }
    }

    fn sweep_after(wait: Duration, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let _ = this.update(cx, |this, cx| this.swept().then(|| cx.notify()));
        })
        .detach();
    }

    fn swept(&mut self) -> bool {
        let before = self.toasts.len();
        self.toasts.retain(|t| !t.expired());
        self.toasts.len() != before
    }
}

#[cfg(test)]
mod tests;
