use gpui::{
    AnyElement, ClickEvent, Context, Div, Focusable as _, IntoElement, Rgba, SharedString, Stateful, Window, div,
    prelude::*, px,
};

use super::data::{self, Connection, Described, RowStatus, Tone};
use super::forum::{self, Forum, Server, Stage};
use super::pages::Slot;
use super::{Role, SettingsPanel, widgets};
use crate::cli;
use crate::dialogs::{self, PairKey};
use crate::store::{self, Busy, Store, ToastKind};
use crate::theme::colours;
use crate::ui::{self, Icon};

const TOKEN: f32 = 230.;
const OFFER: &str = "Pair a phone app";
const OFFER_NOTE: &str = "Shows a code to scan with the cornercase app on your phone";

fn tone_dot(tone: Tone) -> (Option<Rgba>, Rgba) {
    match tone {
        Tone::Good => (Some(colours::done()), colours::done()),
        Tone::Bad => (Some(colours::failed()), colours::failed()),
        Tone::Quiet => (None, colours::muted()),
    }
}

fn connection_label(c: &Connection, status: RowStatus) -> Div {
    let (dot, ink) = tone_dot(status.tone);
    let line = if status.named { status.text } else { widgets::capitalised(&status.text) };
    let title = div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(widgets::capitalised(&c.label))
        .child(widgets::status(line, dot, ink));
    widgets::labelled(title, widgets::capitalised(&c.note_here()))
}

fn forum_status(forum: &Forum) -> Div {
    let text = forum.words();
    match forum.stage() {
        Stage::Off => widgets::status(text, None, colours::muted()),
        Stage::Ready => widgets::status(text, Some(colours::done()), colours::done()),
        Stage::Waiting => widgets::status(text, Some(colours::waiting()), colours::waiting()),
    }
}

fn pair_control(c: &Connection) -> Div {
    let offer = PairKey::of(&c.key).zip(c.pairing_offer()).map(|(key, label)| {
        ui::button(SharedString::from(format!("pair-{}", c.key)), label)
            .on_click(move |_: &ClickEvent, window, cx| dialogs::pairing(key, window, cx))
    });
    div().children(offer)
}

fn app_offer(button: &'static str) -> Div {
    let pair = ui::button("pair-app", button)
        .on_click(|_: &ClickEvent, window, cx| dialogs::pairing(PairKey::App, window, cx));
    widgets::row(false, widgets::label(OFFER, OFFER_NOTE), pair, div())
}

fn server_row(at: usize, server: &Server, forum: &Forum, cx: &Context<SettingsPanel>) -> Stateful<Div> {
    let store = Store::get(cx);
    let store = store.read(cx);
    let id = SharedString::from(format!("forum-server-{at}"));
    let row = ui::row(id.clone(), false)
        .child(ui::icon(Icon::Server, 13.).text_color(colours::muted()))
        .child(div().flex_1().min_w_0().child(ui::one_line(server.name.clone())));
    if store.busy(&server.picking()) {
        return row.child(ui::spinner(id, 12., colours::muted()));
    }
    let picking = forum.picking(store);
    let server = server.clone();
    row.on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
        if !picking {
            pick_server(&server, cx);
        }
    }))
}

fn pick_server(server: &Server, cx: &mut Context<SettingsPanel>) {
    let busy = Busy::new(server.picking(), format!("Picking {} for the forum…", server.name));
    store::run_busy(cx, busy, server.pick_args(), |_, result, cx| {
        let (words, kind) = match result.and_then(|out| forum::picked(&out)) {
            Ok(name) => (format!("The forum goes in {name}"), ToastKind::Done),
            Err(e) => (widgets::capitalised(&cli::reason(&e)), ToastKind::Error),
        };
        Store::get(cx).update(cx, |s, cx| s.toast(words, kind, cx));
    });
}

fn forum_lines(forum: &Forum, cx: &Context<SettingsPanel>) -> Div {
    let opening = forum.address().map(|address| {
        ui::row_action("forum-open", Icon::External, "Open the forum in Discord", false)
            .on_click(move |_, _, cx| cx.open_url(&address))
    });
    let line = div().flex().items_center().gap(px(6.)).child(forum_status(forum)).children(opening);
    let servers = forum.servers.iter().enumerate().map(|(at, server)| server_row(at, server, forum, cx));
    widgets::under(div().flex().flex_col().gap(px(4.)).child(line).children(servers))
}

impl SettingsPanel {
    pub(super) fn forum_under(&self, key: &str, cx: &Context<Self>) -> Option<Div> {
        let place = if self.setting(forum::SETTING).is_some() { forum::SETTING } else { forum::BOT };
        let forum = forum::of_app(Store::get(cx).read(cx).app.as_ref()).filter(|_| key == place)?;
        Some(forum_lines(&forum, cx))
    }

    pub(super) fn app_offer_under(&self, key: &str) -> Option<Div> {
        let offer = self.described.as_ref().and_then(data::app_offer).filter(|_| key == data::PHONE_APP)?;
        Some(app_offer(offer))
    }

    pub(super) fn render_connection(
        &self,
        d: &Described,
        at: usize,
        slot: Slot<'_>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &d.connections[at];
        let role = Role::Secret(c.key.clone());
        let control = if c.takes_secret() { self.token_control(c, slot.window, cx) } else { pair_control(c) };
        div()
            .flex()
            .flex_col()
            .child(widgets::row(
                slot.first,
                connection_label(c, data::row_status(&d.connections, at)),
                control,
                self.connection_slot(c, cx),
            ))
            .children(self.hint_of(&role, c.hint(), slot.window, cx))
            .children(self.error_of(&role))
            .children(self.forum_under(&c.key, cx))
            .into_any_element()
    }

    fn token_control(&self, c: &Connection, window: &Window, cx: &mut Context<Self>) -> Div {
        let Some(field) = self.fields.get(&Role::Secret(c.key.clone())) else { return div() };
        let focused = field.read(cx).focus_handle(cx).is_focused(window);
        let typed = !field.read(cx).text().trim().is_empty();
        let key_ink = if focused { colours::accent() } else { colours::faint() };
        let inner = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(ui::icon(Icon::Key, 13.).text_color(key_ink))
            .child(ui::field_slot(field.clone()));
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(ui::input_box(inner, focused).w(px(TOKEN)).py(px(5.)).overflow_hidden())
            .child(self.connect_button(c, typed, cx))
    }

    fn connect_button(&self, c: &Connection, typed: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let key = c.key.clone();
        let id = SharedString::from(format!("connect-{key}"));
        if self.busy.contains(&Role::Secret(key.clone())) {
            return ui::busy_button(id, "Checking…", ui::Tone::Primary);
        }
        let label = if c.state == data::State::Missing { "Connect" } else { "Replace" };
        if !typed {
            return ui::resting_button(id, label, ui::Tone::Primary);
        }
        ui::primary_button(id, label).on_click(cx.listener(move |this, _, _, cx| this.connect(&key, cx)))
    }

    fn connection_slot(&self, c: &Connection, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.busy.contains(&Role::Secret(c.key.clone()));
        if busy && !c.takes_secret() {
            let id = SharedString::from(format!("saving-secret-{}", c.key));
            return ui::spinner(id, 13., colours::muted()).into_any_element();
        }
        if !c.removable || busy {
            return div().into_any_element();
        }
        let key = c.key.clone();
        ui::row_action(SharedString::from(format!("remove-{}", c.key)), Icon::Trash, c.removal_hint(), true)
            .on_click(cx.listener(move |this, _, window, cx| this.ask_remove(&key, window, cx)))
            .into_any_element()
    }
}
