mod data;
mod draft;
mod form;
mod form_view;
mod time;

use std::time::Duration;

use gpui::{
    AnyElement, AnyView, App, AppContext as _, AsyncApp, ClickEvent, Context, Div, Entity, Focusable as _,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Rgba, StatefulInteractiveElement as _,
    Styled as _, Subscription, WeakEntity, Window, div, prelude::FluentBuilder as _, px,
};
use serde_json::Value;

use crate::dialogs;
use crate::model::Session;
use crate::store::{self, Store, ToastKind, args};
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent};
use data::{Entry, entries, status_label};
use form::{Form, Spot};
use time::{Civil, local, relative, when_label};
pub(crate) use time::{days, now};

const UNDO_ISSUE: &str = "usecornercase/cornercase-terminal#463";

fn status_colour(state: &str) -> Rgba {
    match state {
        "failed" => colours::failed(),
        "missed" | "starting" => colours::working(),
        "started" => colours::done(),
        _ => colours::accent(),
    }
}

pub(crate) struct QueuePanel {
    filter: Entity<TextField>,
    form: Option<Form>,
    config: Option<Value>,
    _subscriptions: Vec<Subscription>,
}

struct RowContext {
    session: Option<Session>,
    boot: Option<String>,
    today: Civil,
    now: i64,
}

impl RowContext {
    fn new(app: Option<&Value>, session: Option<Session>) -> Self {
        let now = now();
        Self {
            session,
            boot: app.and_then(|a| a.get("boot")).and_then(Value::as_str).map(str::to_string),
            today: local(now).unwrap_or(Civil::EPOCH),
            now,
        }
    }
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

fn remove_entries(ids: &[u64], cx: &mut App) {
    let store = Store::get(cx);
    for id in ids {
        store.read(cx).cmd(args(&["queue", "remove", &id.to_string()]));
    }
}

fn notice(app_loaded: bool, entries: usize, shown: usize) -> Option<&'static str> {
    if !app_loaded {
        Some("Loading…")
    } else if entries == 0 {
        Some("Nothing is scheduled. + New queues a prompt or an issue to start an agent later, even unattended.")
    } else if shown == 0 {
        Some("No entry matches the filter.")
    } else {
        None
    }
}

impl QueuePanel {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| TextField::new("Filter by label, prompt, agent or place", cx));
        let subscriptions = vec![
            cx.observe(&Store::get(cx), |_, _, cx| cx.notify()),
            cx.subscribe_in(&filter, window, |_, field, event, _, cx| match event {
                TextFieldEvent::Cancel => field.update(cx, TextField::clear),
                _ => cx.notify(),
            }),
        ];
        cx.spawn(async move |this, cx| tick(this, cx).await).detach();
        Self { filter, form: None, config: None, _subscriptions: subscriptions }
    }

    fn run_now(id: u64, cx: &mut Context<Self>) {
        store::run(cx, args(&["queue", "run", &id.to_string()]), |_: &mut Self, result, cx| {
            toast_outcome(result, |_| ("Starting the queued entry".to_string(), ToastKind::Info), cx);
        });
    }

    fn remove(&mut self, entry: &Entry, window: &mut Window, cx: &mut Context<Self>) {
        let command = args(&["queue", "remove", &entry.id.to_string()]);
        if self.form.as_ref().is_some_and(|f| f.edit == Some(entry.id)) {
            self.form = None;
        }
        if entry.state == "started" {
            Store::get(cx).read(cx).cmd(command);
            return;
        }
        let message = format!(
            "“{}” will not start, and its prompt and choices are gone: undoing a removal needs {UNDO_ISSUE}.",
            entry.label
        );
        dialogs::confirm(dialogs::Question::new("Remove this queue entry?", message, "Remove", command), window, cx);
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

    fn entry_buttons(entry: &Entry, status: &str, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let id = entry.id;
        let pane = entry.pane.filter(|_| status.starts_with("started · pane ") && !status.ends_with("closed"));
        let edited = entry.clone();
        let removed = entry.clone();
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.))
            .when(entry.runnable(), |d| {
                d.child(ui::button(("queue-edit", id), "Edit").on_click(
                    cx.listener(move |this, _: &ClickEvent, window, cx| this.open_form(Some(&edited), window, cx)),
                ))
                .child(
                    ui::primary_button(("queue-run", id), "Start now")
                        .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| Self::run_now(id, cx))),
                )
            })
            .when_some(pane, |d, pane| {
                d.child(ui::primary_button(("queue-open", id), "Open").on_click(move |_, _, cx| open_pane(pane, cx)))
            })
            .when(entry.state != "starting", |d| {
                d.child(
                    ui::icon_button(("queue-rm", id), "✕").on_click(
                        cx.listener(move |this, _: &ClickEvent, window, cx| this.remove(&removed, window, cx)),
                    ),
                )
            })
    }

    fn entry_row(&self, entry: &Entry, ctx: &RowContext, cx: &mut Context<Self>) -> AnyElement {
        let editing = self.form.as_ref().is_some_and(|f| f.edit == Some(entry.id));
        let when = local(entry.at).map_or_else(|| entry.at.to_string(), |at| when_label(at, ctx.today));
        let status = status_label(entry, ctx.boot.as_deref(), ctx.session.as_ref());
        let buttons = Self::entry_buttons(entry, &status, cx);
        div()
            .id(("queue", entry.id))
            .flex()
            .items_start()
            .gap(px(14.))
            .px(px(12.))
            .py(px(9.))
            .rounded(px(6.))
            .border_1()
            .border_color(if editing { colours::accent() } else { gpui::transparent_black().into() })
            .hover(|s| s.bg(colours::hover()))
            .child(
                div()
                    .w(px(130.))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(div().text_size(px(13.)).text_color(colours::text()).child(when))
                    .child(ui::dim(relative(entry.at, ctx.now))),
            )
            .child(entry_text(entry, status, ctx.session.as_ref()))
            .child(buttons)
            .into_any_element()
    }

    fn toolbar(&self, entries: &[Entry], window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let started: Vec<u64> = entries.iter().filter(|e| e.state == "started").map(|e| e.id).collect();
        let pending = entries.iter().filter(|e| e.state == "pending").count();
        let focused = self.filter.read(cx).focus_handle(cx).is_focused(window);
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(16.))
            .py(px(10.))
            .border_b_1()
            .border_color(colours::border())
            .child(div().flex_1().child(ui::input_box(self.filter.clone(), focused)))
            .child(ui::dim(format!("{pending} pending · {} in all", entries.len())))
            .when(!started.is_empty(), |d| {
                d.child(
                    ui::button("queue-clear-started", "Clear started")
                        .on_click(move |_: &ClickEvent, _, cx| remove_entries(&started, cx)),
                )
            })
            .child(
                ui::primary_button("queue-new", "+ New")
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_form(None, window, cx))),
            )
    }

    fn list(&self, app_loaded: bool, entries: &[Entry], ctx: &RowContext, cx: &mut Context<Self>) -> Div {
        let filter = self.filter.read(cx).text().to_lowercase();
        let shown: Vec<&Entry> = entries.iter().filter(|e| e.matches(&filter, ctx.session.as_ref())).collect();
        let rows: Vec<AnyElement> = shown.iter().map(|entry| self.entry_row(entry, ctx, cx)).collect();
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .p(px(10.))
            .children(notice(app_loaded, entries.len(), shown.len()).map(ui::note))
            .children(rows)
            .when(!entries.is_empty(), |d| {
                d.child(div().px(px(12.)).pt(px(8.)).child(ui::dim(format!(
                    "Removing an entry cannot be undone yet ({UNDO_ISSUE}); removing a started one does not stop its agent."
                ))))
            })
    }
}

fn entry_text(entry: &Entry, status: String, session: Option<&Session>) -> Div {
    let first_line = entry.first_prompt_line();
    div()
        .flex_1()
        .min_w(px(0.))
        .flex()
        .flex_col()
        .gap(px(3.))
        .child(div().text_size(px(13.)).truncate().child(entry.label.clone()))
        .when(!first_line.is_empty() && first_line != entry.label, |d| {
            d.child(div().text_size(px(12.)).text_color(colours::muted()).truncate().child(first_line))
        })
        .child(
            div()
                .flex()
                .gap(px(6.))
                .text_size(px(11.))
                .child(div().flex_none().text_color(status_colour(&entry.state)).child(status))
                .child(div().text_color(colours::faint()).truncate().child(format!("· {}", entry.meta(session)))),
        )
}

impl Render for QueuePanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (app, session) = {
            let s = Store::get(cx).read(cx);
            (s.app.clone(), s.session.clone())
        };
        let entries = entries(app.as_ref());
        self.sync_form(&entries, session.as_ref());
        let ctx = RowContext::new(app.as_ref(), session);
        let toolbar = self.toolbar(&entries, window, cx);
        let list = self.list(app.is_some(), &entries, &ctx, cx);
        div()
            .size_full()
            .flex()
            .text_color(colours::text())
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(toolbar)
                    .child(div().id("queue-list").flex_1().overflow_y_scroll().child(list)),
            )
            .children(self.form_view(window, cx))
    }
}

pub(crate) fn build(window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| QueuePanel::new(window, cx)).into()
}

#[cfg(test)]
mod tests;
