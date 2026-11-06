mod data;
mod links;
mod names;
mod view;

use std::time::{Duration, Instant};

use gpui::{
    AnyView, App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _, Subscription,
    Task, Window, div,
};

use self::data::{Answer, Choice, Slot};
use crate::cli;
use crate::panels::PanelKind;
use crate::panels::flows::model::now;
use crate::store::{self, Store, ToastKind};
use crate::theme::colours;

const TICK: Duration = Duration::from_secs(20);
const AGAIN: Duration = Duration::from_secs(2);

pub(crate) struct RecapPanel {
    choice: Choice,
    slots: [Slot; 3],
    open: bool,
    _store: Subscription,
    _ticker: Task<()>,
}

impl RecapPanel {
    fn new(cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&Store::get(cx), |this, store, cx| this.store_changed(&store, cx));
        let ticker = cx.spawn(async move |this, cx| {
            while this.update(cx, Self::tick).is_ok() {
                cx.background_executor().timer(TICK).await;
            }
        });
        Self::ask(Choice::Now, false, cx);
        Self { choice: Choice::Now, slots: Default::default(), open: true, _store: subscription, _ticker: ticker }
    }

    fn slot(&self) -> &Slot {
        &self.slots[self.choice.index()]
    }

    fn store_changed(&mut self, store: &Entity<Store>, cx: &mut Context<Self>) {
        let open = store.read(cx).shown_dock() == Some(PanelKind::Recap);
        if open && !self.open {
            Self::ask(self.choice, false, cx);
        }
        self.open = open;
        if open {
            cx.notify();
        }
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        if self.open {
            cx.notify();
        }
    }

    fn choose(&mut self, choice: Choice, cx: &mut Context<Self>) {
        self.choice = choice;
        Self::ask(choice, false, cx);
        cx.notify();
    }

    fn write_again(&mut self, cx: &mut Context<Self>) {
        Self::ask(self.choice, true, cx);
    }

    fn ask(choice: Choice, fresh: bool, cx: &mut Context<Self>) {
        if Store::get(cx).read(cx).busy(&choice.key()) {
            return;
        }
        store::run_partial_busy(cx, choice.busy(), choice.command(fresh), move |this, out, cx| {
            this.answered(choice, out, cx);
        });
    }

    fn answered(&mut self, choice: Choice, out: anyhow::Result<String>, cx: &mut Context<Self>) {
        let answer = out.and_then(|out| Ok(serde_json::from_str::<Answer>(&out)?)).map_err(|e| cli::reason(&e));
        let shown = self.open && self.choice == choice;
        let slot = &mut self.slots[choice.index()];
        let runs = slot.answered(answer);
        if let Some(error) = slot.error.clone().filter(|_| !shown) {
            Store::get(cx).update(cx, |s, cx| s.toast(format!("Recap: {error}"), ToastKind::Error, cx));
        }
        if runs {
            Self::ask_again(choice, cx);
        }
        cx.notify();
    }

    fn ask_again(choice: Choice, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(AGAIN).await;
            let _ = this.update(cx, |_, cx| Self::ask(choice, false, cx));
        })
        .detach();
    }
}

impl Render for RecapPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let running = Store::get(cx).read(cx).jobs.shown(&self.choice.key(), Instant::now());
        div()
            .size_full()
            .flex()
            .flex_col()
            .text_color(colours::text())
            .child(self.toolbar(running, cx))
            .child(self.notes(running, now(), cx))
            .child(self.body(running))
    }
}

pub(crate) fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(RecapPanel::new).into()
}
