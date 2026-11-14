use super::feed::{Message, Notice};
use crate::model::Session;
use crate::panels::PanelKind;
use crate::store::{Busy, NoticeThere, ToastGo, ToastKind, args, setup_key};

pub(super) const SHOW: &str = "Show";
pub(super) const OPEN_QUEUE: &str = "Open Queue";
pub(super) const REOPEN: &str = "Reopen agents";
pub(super) const GO: &str = "Go";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Notify,
    QueueStarted,
    QueueFailed,
    QueueMissed,
    QueueKept,
    SetupFailed,
    FlowConsent,
    FlowMerged,
    Other,
}

impl Kind {
    fn of(id: &str) -> Kind {
        match id {
            "notify" => Kind::Notify,
            "queue-started" => Kind::QueueStarted,
            "queue-failed" => Kind::QueueFailed,
            "queue-missed" => Kind::QueueMissed,
            "queue-kept" => Kind::QueueKept,
            "setup-failed" => Kind::SetupFailed,
            "flow-consent" => Kind::FlowConsent,
            "flow-merged" => Kind::FlowMerged,
            _ => Kind::Other,
        }
    }

    fn of_queue(self) -> bool {
        matches!(self, Kind::QueueStarted | Kind::QueueFailed | Kind::QueueMissed | Kind::QueueKept)
    }

    fn toast(self, failed: bool) -> ToastKind {
        if failed {
            return ToastKind::NeedsYou;
        }
        match self {
            Kind::QueueFailed | Kind::QueueMissed | Kind::QueueKept | Kind::SetupFailed | Kind::FlowConsent => {
                ToastKind::NeedsYou
            }
            Kind::Notify | Kind::QueueStarted | Kind::FlowMerged | Kind::Other => ToastKind::Done,
        }
    }

    fn headline(self) -> Option<&'static str> {
        match self {
            Kind::Notify | Kind::Other => Some("Message"),
            Kind::QueueStarted | Kind::QueueFailed | Kind::QueueMissed | Kind::QueueKept => Some("Queue"),
            Kind::SetupFailed => Some("Setup"),
            Kind::FlowConsent => Some("Flow"),
            Kind::FlowMerged => None,
        }
    }
}

type Button = Option<(String, ToastGo)>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Telling {
    pub text: String,
    pub kind: ToastKind,
    pub button: Button,
    pub headline: Option<String>,
    pub sound: Option<String>,
}

impl Telling {
    pub(super) fn of_notice(notice: &Notice, there: Option<NoticeThere>) -> Telling {
        Telling {
            text: notice.message.clone(),
            kind: notice.toast_kind(),
            button: Some((GO.to_string(), notice.go(there))),
            headline: Some(notice.headline()),
            sound: notice.sound.clone(),
        }
    }

    pub(super) fn of_message(message: &Message, session: Option<&Session>) -> Telling {
        let kind = Kind::of(&message.kind);
        Telling {
            text: message.text.clone(),
            kind: kind.toast(message.failed),
            button: button(message, kind, session),
            headline: kind.headline().map(str::to_string),
            sound: None,
        }
    }

    pub(super) fn waits(&self, active: bool) -> bool {
        !active && self.headline.is_none()
    }

    pub(super) fn needs_config(&self, behind: bool) -> bool {
        self.sound.is_some() || (behind && self.headline.is_some())
    }

    pub(super) fn posts(&self, behind: bool, desktop: bool) -> bool {
        behind && desktop && self.headline.is_some()
    }
}

#[derive(Default)]
pub(super) struct Waiting(Vec<Telling>);

impl Waiting {
    pub(super) fn hold_or_pass(&mut self, telling: Telling, active: bool) -> Option<Telling> {
        if telling.waits(active) {
            self.0.push(telling);
            return None;
        }
        Some(telling)
    }

    pub(super) fn release(&mut self, active: bool) -> Vec<Telling> {
        if active { std::mem::take(&mut self.0) } else { Vec::new() }
    }

    pub(super) fn forget(&mut self) {
        self.0.clear();
    }
}

fn button(message: &Message, kind: Kind, session: Option<&Session>) -> Button {
    if kind == Kind::FlowMerged {
        return message.workspace.map(|workspace| (REOPEN.to_string(), reopen(workspace, session)));
    }
    match show(message) {
        Some(go) => Some((SHOW.to_string(), go)),
        None if kind.of_queue() => Some((OPEN_QUEUE.to_string(), ToastGo::Open(PanelKind::Queue))),
        None => None,
    }
}

fn show(message: &Message) -> Option<ToastGo> {
    if let Some(pane) = message.panes.first() {
        return Some(ToastGo::Run(args(&["focus", "--pane", &pane.to_string()])));
    }
    let workspace = message.workspace?;
    Some(ToastGo::Run(args(&["focus", "--workspace", &workspace.to_string()])))
}

fn reopen(workspace: u64, session: Option<&Session>) -> ToastGo {
    let name = session
        .and_then(|session| session.workspace(workspace))
        .map_or_else(|| format!("workspace {workspace}"), |(_, ws)| ws.name.clone());
    let (busy, args): (Busy, Vec<String>) = crate::store::reopening(workspace, &name);
    ToastGo::Job(busy, args)
}

pub(super) struct Looking {
    pub active: bool,
    pub queue_open: bool,
}

impl Looking {
    pub(super) fn of(active: bool, sheet: Option<PanelKind>) -> Looking {
        Looking { active, queue_open: sheet == Some(PanelKind::Queue) }
    }
}

pub(super) fn dropped(message: &Message, looking: &Looking, busy: impl Fn(&str) -> bool) -> bool {
    match Kind::of(&message.kind) {
        kind if kind.of_queue() => looking.active && looking.queue_open,
        Kind::SetupFailed => message.workspace.is_some_and(|workspace| busy(&setup_key(workspace))),
        _ => false,
    }
}

#[cfg(test)]
mod tests;
