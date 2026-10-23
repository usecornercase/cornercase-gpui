use gpui::Context;

use crate::panels::PanelKind;
use crate::store::{self, Store, args};

use super::pieces::find;
use super::wire::{self, State};
use super::{ChangesPanel, Follow, Scope};

fn list_args(follow: Follow) -> Vec<String> {
    let mut a = args(&["changes", "list", "--follow"]);
    match follow.workspace {
        Some(w) => {
            a.extend(["--workspace", &w.to_string(), "--mode", follow.scope.mode().name()].map(String::from));
        }
        None => a.push("--every".into()),
    }
    a
}

fn ended_reason(err: &str) -> String {
    let err = err.trim();
    if err.is_empty() { "the changes stopped".to_string() } else { err.to_string() }
}

impl ChangesPanel {
    pub(super) fn sync(&mut self, cx: &mut Context<Self>) {
        let store = Store::get(cx);
        let s = store.read(cx);
        let docked = s.dock == Some(PanelKind::Changes);
        let shown = s.session.as_ref().and_then(|s| s.shown().workspace);
        if shown != self.shown {
            self.shown = shown;
            self.base = None;
            self.base_open = false;
            if matches!(self.scope, Scope::Project(_)) {
                self.select(None, cx);
            }
        }
        let want = if docked { self.wanted_follow(shown) } else { None };
        if want != self.follow {
            self.restart(want, cx);
        }
    }

    fn wanted_follow(&self, shown: Option<u64>) -> Option<Follow> {
        match self.scope {
            Scope::Project(_) => shown.map(|w| Follow { scope: self.scope, workspace: Some(w) }),
            Scope::Every => Some(Follow { scope: Scope::Every, workspace: None }),
        }
    }

    fn restart(&mut self, follow: Option<Follow>, cx: &mut Context<Self>) {
        self.follow = follow;
        self.stream = None;
        self.summary = None;
        self.ended = None;
        self.stream_generation += 1;
        let generation = self.stream_generation;
        let Some(follow) = follow else {
            cx.notify();
            return;
        };
        self.stream = store::follow_until(
            cx,
            list_args(follow),
            move |this, line, cx| {
                if this.stream_generation == generation {
                    this.on_summary(&line, cx);
                }
            },
            move |this, err, cx| this.on_end(generation, &err, cx),
        );
        cx.notify();
    }

    fn on_end(&mut self, generation: u64, err: &str, cx: &mut Context<Self>) {
        if self.stream_generation != generation {
            return;
        }
        self.stream = None;
        self.ended = Some(ended_reason(err));
        cx.notify();
    }

    fn on_summary(&mut self, line: &str, cx: &mut Context<Self>) {
        let Ok(summary) = wire::summary(line) else { return };
        self.summary = Some(summary);
        self.refresh_open(cx);
        cx.notify();
    }

    fn refresh_open(&mut self, cx: &mut Context<Self>) {
        let Some(selected) = self.selected.clone() else { return };
        let Some(summary) = &self.summary else { return };
        if summary.state != State::Ready {
            return;
        }
        match find(summary, selected.workspace, &selected.path) {
            Some(file) if file.digest != self.open.digest => {
                let digest = file.digest.clone();
                self.load_patch(digest, cx);
            }
            Some(_) => {}
            None => self.select(None, cx),
        }
    }
}
