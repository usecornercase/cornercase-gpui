use std::time::{Duration, Instant};

use gpui::Context;

use super::{Store, ToastGo};

const KEPT: usize = 5;
const PAST_END: Duration = Duration::from_millis(100);

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

#[derive(Clone)]
pub(crate) struct Toast {
    pub id: u64,
    pub text: String,
    pub kind: ToastKind,
    pub action: Option<(String, ToastGo)>,
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
    pub(crate) fn toast_with(
        &mut self,
        text: impl Into<String>,
        kind: ToastKind,
        action: Option<(String, ToastGo)>,
        cx: &mut Context<Self>,
    ) {
        let id = self.next_toast;
        self.next_toast += 1;
        let shown = Shown::default();
        push_toast(&mut self.toasts, Toast { id, text: text.into(), kind, action, shown, count: 1 });
        cx.notify();
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
