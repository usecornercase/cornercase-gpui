mod bot;
mod data;
mod detail;
mod detail_text;
mod detail_view;
mod draft;
mod empty;
mod form;
mod form_view;
mod keys;
mod lines;
mod list;
mod read;
mod repeat;
mod result;
mod rows;
mod runs;
mod time;
mod undo;

use std::time::Duration;

use gpui::{
    AnyElement, AnyView, App, AppContext as _, AsyncApp, ClickEvent, Context, Div, Entity, FocusHandle, Focusable as _,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _,
    Subscription, WeakEntity, Window, div, px,
};
use serde_json::Value;

use crate::cli::{self, Releases};
use crate::model::Session;
use crate::panels::PanelKind;
use crate::store::{self, Busy, Store, ToastKind, args};
use crate::theme::colours;
use crate::ui::split::{self, Edge, Split};
use crate::ui::{self, TextField, TextFieldEvent};
use data::{Body, scoped};
pub(crate) use data::{Entry, entries};
use detail::Detail;
use form::{FlowChoices, Form, Spot};
use keys::{KeysTo, follows, keys_go};
use rows::{Around, Snapshot};
pub(crate) use time::{days, local, now, when_label};
use undo::{Removal, UNDO_FOR, undo_shown};

const SIDE: Split = Split::new(420., 360., 760.);
const LIST_KEEP: f32 = 320.;
const NO_MATCH: &str = "No entry matches the filter.";

struct SideGrip;

pub(crate) struct QueuePanel {
    filter: Entity<TextField>,
    form: Option<Form>,
    config: Option<Value>,
    flows: FlowChoices,
    side_width: Split,
    detail: Option<Detail>,
    focus: FocusHandle,
    seqs: read::Counter,
    removal: Option<Removal>,
    scope: Option<String>,
    first: FocusHandle,
    wants_keys: bool,
    was_open: bool,
    had_form: bool,
    went: KeysTo,
    _subscriptions: Vec<Subscription>,
}

struct Seen {
    app: Option<Value>,
    session: Option<Session>,
    releases: Releases<'static>,
    scope: Option<(String, String)>,
    bots: Vec<String>,
    stops: bool,
}

impl Seen {
    fn read(cx: &App) -> Self {
        let s = Store::get(cx).read(cx);
        Self {
            app: s.app.clone(),
            session: s.session.clone(),
            releases: s.releases(),
            scope: s.routines_of().map(|bot| (bot.id.clone(), bot.name.clone())),
            bots: s.bots.list.iter().map(|bot| bot.id.clone()).collect(),
            stops: s.sheet == Some(PanelKind::Queue) && s.dialog.is_none(),
        }
    }
}

fn queue_shows(cx: &App) -> bool {
    Store::get(cx).read(cx).sheet == Some(PanelKind::Queue)
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

fn toast_error(error: &anyhow::Error, cx: &mut App) {
    Store::get(cx).update(cx, |s, cx| s.toast(cli::reason(error), ToastKind::Error, cx));
}

pub(crate) fn repeat_words(entry: &Entry) -> String {
    repeat::words(entry.repeat.as_deref().unwrap_or_default())
}

fn starting(entry: u64) -> String {
    format!("queue-run:{entry}")
}

fn pausing(entry: u64) -> String {
    format!("queue-pause:{entry}")
}

fn scrolled(content: impl IntoElement) -> AnyElement {
    div().id("queue-list").flex_1().min_h_0().overflow_y_scroll().child(content).into_any_element()
}

impl QueuePanel {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| TextField::new("Filter by label, prompt, agent or place", cx).clears_on_escape());
        let open = queue_shows(cx);
        let subscriptions = vec![
            cx.observe(&Store::get(cx), |this: &mut Self, _, cx| {
                this.sheet_shown(queue_shows(cx));
                cx.notify();
            }),
            cx.subscribe_in(&filter, window, |_, _, _: &TextFieldEvent, _, cx| cx.notify()),
        ];
        cx.spawn(async move |this, cx| tick(this, cx).await).detach();
        Self {
            filter,
            form: None,
            config: None,
            flows: FlowChoices::default(),
            side_width: SIDE,
            detail: None,
            focus: cx.focus_handle(),
            seqs: read::Counter::default(),
            removal: None,
            scope: None,
            first: cx.focus_handle(),
            wants_keys: open,
            was_open: open,
            had_form: false,
            went: KeysTo::Wait,
            _subscriptions: subscriptions,
        }
    }

    fn sheet_shown(&mut self, open: bool) {
        if open && !self.was_open {
            self.wants_keys = true;
        }
        self.was_open = open;
    }

    fn take_keys(
        &mut self,
        (body, first_drawn, on_top): (Body, bool, bool),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.had_form && self.form.is_none() {
            self.wants_keys = true;
        }
        self.had_form = self.form.is_some();
        let to = keys_go(on_top, self.form.is_some(), body);
        let before = std::mem::replace(&mut self.went, to);
        if follows(before, to) && self.handle_of(before, true, cx).is_some_and(|had| had.is_focused(window)) {
            self.wants_keys = true;
        }
        if !self.wants_keys || to == KeysTo::Wait {
            return;
        }
        self.wants_keys = false;
        let Some(handle) = self.handle_of(to, first_drawn, cx) else { return };
        cx.defer_in(window, move |_, window, _| window.focus(&handle));
    }

    fn handle_of(&self, to: KeysTo, first_drawn: bool, cx: &App) -> Option<FocusHandle> {
        match (to, self.form.as_ref()) {
            (KeysTo::Prompt, Some(form)) => Some(form.prompt.read(cx).focus_handle(cx)),
            (KeysTo::Filter, _) => Some(self.filter.read(cx).focus_handle(cx)),
            (KeysTo::First, _) if first_drawn => Some(self.first.clone()),
            _ => None,
        }
    }

    fn filter_text(&self, cx: &App) -> String {
        self.filter.read(cx).text().trim().to_lowercase()
    }

    fn run_now(entry: &Entry, cx: &mut Context<Self>) {
        let busy = Busy::new(starting(entry.id), format!("Starting {} on {}…", entry.who(), entry.label));
        let text = if entry.is_routine() {
            format!("Starting a run of “{}”", entry.label)
        } else {
            "Starting the queued entry".to_string()
        };
        store::run_busy(cx, busy, args(&["queue", "run", &entry.id.to_string()]), |_: &mut Self, result, cx| {
            toast_outcome(result, |_| (text, ToastKind::Info), cx);
        });
    }

    fn set_paused(entry: &Entry, paused: bool, cx: &mut Context<Self>) {
        let (command, doing) = if paused { ("pause", "Pausing") } else { ("resume", "Resuming") };
        let busy = Busy::new(pausing(entry.id), format!("{doing} {}…", entry.label));
        store::run_busy(cx, busy, args(&["queue", command, &entry.id.to_string()]), |_: &mut Self, result, cx| {
            if let Err(e) = result {
                toast_error(&e, cx);
            }
        });
    }

    fn remove(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.form.as_ref().is_some_and(|f| f.edit == Some(id)) {
            self.form = None;
        }
        self.take_out(args(&["queue", "remove", &id.to_string(), "--json"]), cx);
    }

    fn clear_started(&self, cx: &mut Context<Self>) {
        self.take_out(args(&["queue", "clear-started", "--json"]), cx);
    }

    fn take_out(&self, command: Vec<String>, cx: &mut Context<Self>) {
        let asked_in = self.scope.clone();
        store::run(cx, command, move |this: &mut Self, result, cx| match result {
            Ok(out) if undo_shown(asked_in.as_deref(), this.scope.as_deref()) => {
                this.offer_undo(Removal::from_answer(&out), cx);
            }
            Ok(_) => {}
            Err(e) => toast_error(&e, cx),
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
                toast_error(&e, cx);
            }
        });
    }

    fn undo_bar(&self, cx: &mut Context<Self>) -> Option<Div> {
        let removal = self.removal.as_ref()?;
        let button = ui::primary_button("queue-undo", "Undo")
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.restore(cx)));
        Some(ui::undo_bar(removal.text.clone(), button).mx(px(16.)).mt(px(12.)))
    }

    fn sync_scope(&mut self, bot: Option<&str>, cx: &mut Context<Self>) {
        if self.scope.as_deref() == bot {
            return;
        }
        self.scope = bot.map(str::to_string);
        self.wants_keys = true;
        self.form = None;
        self.detail = None;
        self.removal = None;
        self.filter.update(cx, TextField::clear);
    }

    fn sync_form(&mut self, entries: &[Entry], session: Option<&Session>) {
        if let Some(edit) = self.form.as_ref().and_then(|f| f.edit)
            && !entries.iter().any(|e| e.id == edit && e.runnable())
        {
            self.form = None;
        }
        if let Some(form) = self.form.as_mut()
            && form.edit.is_none()
            && form.bot.is_none()
            && form.project.is_none()
            && let Some(shown) = session.map(Session::shown)
        {
            form.project = shown.project;
            form.spot = Spot::from(shown.workspace);
        }
        if let (Some(form), Some(session)) = (self.form.as_mut(), session) {
            form.fit_repository(session);
        }
    }

    fn body(&self, entries: &[Entry], snapshot: &Snapshot, cx: &App) -> Body {
        let filter = self.filter_text(cx);
        let shown = entries.iter().filter(|e| e.matches(&filter, snapshot.session.as_ref())).count();
        Body::of(snapshot.loaded, entries.len(), shown)
    }

    fn main(
        &self,
        (entries, body): (&[Entry], Body),
        snapshot: &Snapshot,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let filter = self.filter_text(cx);
        let toolbar = matches!(body, Body::NoMatch | Body::List).then(|| self.toolbar(snapshot, window, cx));
        let content = match (body, snapshot.scope.as_ref()) {
            (Body::Loading, _) => ui::loading("Loading…").into_any_element(),
            (Body::Empty, Some((_, name))) => self.no_routines(name, snapshot, cx),
            (Body::Empty, None) => self.empty_page(snapshot, cx),
            (Body::NoMatch, _) => scrolled(ui::note(NO_MATCH)),
            (Body::List, _) => scrolled(self.list(entries, &filter, snapshot, cx)),
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
        let Seen { app, session, releases, scope, bots, stops } = Seen::read(cx);
        self.sync_scope(scope.as_ref().map(|(id, _)| id.as_str()), cx);
        let all = entries(app.as_ref());
        let around = Around { elsewhere: bot::elsewhere(&all, &bots), clears_bots: bot::cleared_too(&all), stops };
        let entries = scoped(all, self.scope.as_deref());
        self.sync_form(&entries, session.as_ref());
        self.sync_flows(cx);
        self.sync_accounts(cx);
        self.filter.update(cx, |field, _| field.set_in_tab_order(stops));
        let snapshot = Snapshot::new(app.as_ref(), session, &releases).scoped(scope, around);
        self.sync_detail(&entries, snapshot.results, window, cx);
        let body = self.body(&entries, &snapshot, cx);
        self.take_keys((body, snapshot.bot_add_note().is_none(), snapshot.stops), window, cx);
        let main = self.main((&entries, body), &snapshot, window, cx);
        let at = (Edge::Left, self.side_width.size());
        let side = self.form_view(window, cx).or_else(|| self.detail_view(&entries, &snapshot, cx));
        let handle = side.as_ref().map(|_| split::handle("queue-split", SideGrip, at, side_width, cx));
        div()
            .size_full()
            .relative()
            .flex()
            .text_color(colours::text())
            .on_drag_move(split::follow::<_, SideGrip>(Edge::Left, LIST_KEEP, side_width, cx))
            .child(main)
            .children(side)
            .children(handle)
    }
}

fn side_width(panel: &mut QueuePanel) -> &mut Split {
    &mut panel.side_width
}

pub(crate) fn build(window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| QueuePanel::new(window, cx)).into()
}

#[cfg(test)]
mod tests;
