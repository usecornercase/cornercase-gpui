mod data;
mod draft;
mod empty;
mod form;
mod form_view;
mod list;
mod rows;
mod time;
mod undo;

use std::time::Duration;

use gpui::{
    AnyElement, AnyView, App, AppContext as _, AsyncApp, ClickEvent, Context, Div, Entity, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity,
    Window, div, px,
};
use serde_json::Value;

use crate::model::Session;
use crate::store::{self, Busy, Store, ToastKind, args};
use crate::theme::colours;
use crate::ui::split::{self, Edge, Split};
use crate::ui::{self, TextField, TextFieldEvent};
use data::{Body, Entry, entries};
use form::{Form, Spot};
use rows::Snapshot;
pub(crate) use time::{days, now};
use undo::{Removal, UNDO_FOR};

const FORM: Split = Split::new(420., 360., 760.);
const LIST_KEEP: f32 = 320.;
const NO_MATCH: &str = "No entry matches the filter.";

struct FormGrip;

pub(crate) struct QueuePanel {
    filter: Entity<TextField>,
    form: Option<Form>,
    config: Option<Value>,
    form_width: Split,
    removal: Option<Removal>,
    _subscriptions: Vec<Subscription>,
}

async fn tick(this: WeakEntity<QueuePanel>, cx: &mut AsyncApp) {
    loop {
        cx.background_executor().timer(Duration::from_secs(30)).await;
        if this.update(cx, |_, cx| cx.notify()).is_err() {
            return;
        }
    }
}

pub(crate) fn open_pane(pane: u64, cx: &mut App) {
    Store::get(cx).update(cx, |s, cx| {
        s.focus("pane", pane);
        s.close_sheet(cx);
    });
}

pub(crate) fn toast_outcome<T>(result: anyhow::Result<T>, done: impl FnOnce(T) -> (String, ToastKind), cx: &mut App) {
    let (text, kind) = match result {
        Ok(value) => done(value),
        Err(e) => (e.to_string(), ToastKind::Error),
    };
    Store::get(cx).update(cx, |s, cx| s.toast(text, kind, cx));
}

fn starting(entry: u64) -> String {
    format!("queue-run:{entry}")
}

fn scrolled(content: impl IntoElement) -> AnyElement {
    div().id("queue-list").flex_1().min_h_0().overflow_y_scroll().child(content).into_any_element()
}

impl QueuePanel {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| TextField::new("Filter by label, prompt, agent or place", cx).clears_on_escape());
        let subscriptions = vec![
            cx.observe(&Store::get(cx), |_, _, cx| cx.notify()),
            cx.subscribe_in(&filter, window, |_, _, _: &TextFieldEvent, _, cx| cx.notify()),
        ];
        cx.spawn(async move |this, cx| tick(this, cx).await).detach();
        Self { filter, form: None, config: None, form_width: FORM, removal: None, _subscriptions: subscriptions }
    }

    fn filter_text(&self, cx: &App) -> String {
        self.filter.read(cx).text().trim().to_lowercase()
    }

    fn run_now(entry: &Entry, cx: &mut Context<Self>) {
        let busy = Busy::new(starting(entry.id), format!("Starting {} on {}…", entry.agent, entry.label));
        store::run_busy(cx, busy, args(&["queue", "run", &entry.id.to_string()]), |_: &mut Self, result, cx| {
            toast_outcome(result, |_| ("Starting the queued entry".to_string(), ToastKind::Info), cx);
        });
    }

    fn remove(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.form.as_ref().is_some_and(|f| f.edit == Some(id)) {
            self.form = None;
        }
        Self::take_out(args(&["queue", "remove", &id.to_string(), "--json"]), cx);
    }

    fn clear_started(cx: &mut Context<Self>) {
        Self::take_out(args(&["queue", "clear-started", "--json"]), cx);
    }

    fn take_out(command: Vec<String>, cx: &mut Context<Self>) {
        store::run(cx, command, |this: &mut Self, result, cx| match result {
            Ok(out) => this.offer_undo(Removal::from_answer(&out), cx),
            Err(e) => Store::get(cx).update(cx, |s, cx| s.toast(e.to_string(), ToastKind::Error, cx)),
        });
    }

    fn offer_undo(&mut self, removal: Option<Removal>, cx: &mut Context<Self>) {
        let Some(number) = removal.as_ref().map(|r| r.undo) else { return };
        self.removal = removal;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(UNDO_FOR).await;
            let _ = this.update(cx, |this, cx| this.expire_undo(number, cx));
        })
        .detach();
    }

    fn expire_undo(&mut self, number: u64, cx: &mut Context<Self>) {
        if self.removal.as_ref().is_some_and(|r| r.undo == number) {
            self.removal = None;
            cx.notify();
        }
    }

    fn restore(&mut self, cx: &mut Context<Self>) {
        let Some(removal) = self.removal.take() else { return };
        cx.notify();
        store::run(cx, args(&["queue", "restore", &removal.undo.to_string()]), |_: &mut Self, result, cx| {
            if let Err(e) = result {
                Store::get(cx).update(cx, |s, cx| s.toast(e.to_string(), ToastKind::Error, cx));
            }
        });
    }

    fn undo_bar(&self, cx: &mut Context<Self>) -> Option<Div> {
        let removal = self.removal.as_ref()?;
        let button = ui::primary_button("queue-undo", "Undo")
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.restore(cx)));
        Some(ui::undo_bar(removal.text.clone(), button).mx(px(16.)).mt(px(12.)))
    }

    fn sync_form(&mut self, entries: &[Entry], session: Option<&Session>) {
        if let Some(edit) = self.form.as_ref().and_then(|f| f.edit)
            && !entries.iter().any(|e| e.id == edit && e.runnable())
        {
            self.form = None;
        }
        if let Some(form) = self.form.as_mut()
            && form.edit.is_none()
            && form.project.is_none()
            && let Some(shown) = session.map(Session::shown)
        {
            form.project = shown.project;
            form.spot = Spot::from(shown.workspace);
        }
    }

    fn main(&self, entries: &[Entry], snapshot: &Snapshot, window: &Window, cx: &mut Context<Self>) -> Div {
        let filter = self.filter_text(cx);
        let shown = entries.iter().filter(|e| e.matches(&filter, snapshot.session.as_ref())).count();
        let body = Body::of(snapshot.loaded, entries.len(), shown);
        let toolbar = matches!(body, Body::NoMatch | Body::List).then(|| self.toolbar(window, cx));
        let content = match body {
            Body::Loading => ui::loading("Loading…").into_any_element(),
            Body::Empty => self.empty_page(cx),
            Body::NoMatch => scrolled(ui::note(NO_MATCH)),
            Body::List => scrolled(self.list(entries, &filter, snapshot, cx)),
        };
        div()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .flex()
            .flex_col()
            .children(toolbar)
            .children(self.undo_bar(cx))
            .child(content)
    }
}

impl Render for QueuePanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (app, session) = {
            let s = Store::get(cx).read(cx);
            (s.app.clone(), s.session.clone())
        };
        let entries = entries(app.as_ref());
        self.sync_form(&entries, session.as_ref());
        let snapshot = Snapshot::new(app.as_ref(), session);
        let main = self.main(&entries, &snapshot, window, cx);
        let at = (Edge::Left, self.form_width.size());
        let handle = self.form.as_ref().map(|_| split::handle("queue-split", FormGrip, at, form_width, cx));
        div()
            .size_full()
            .relative()
            .flex()
            .text_color(colours::text())
            .on_drag_move(split::follow::<_, FormGrip>(Edge::Left, LIST_KEEP, form_width, cx))
            .child(main)
            .children(self.form_view(window, cx))
            .children(handle)
    }
}

fn form_width(panel: &mut QueuePanel) -> &mut Split {
    &mut panel.form_width
}

pub(crate) fn build(window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| QueuePanel::new(window, cx)).into()
}

#[cfg(test)]
mod tests;
