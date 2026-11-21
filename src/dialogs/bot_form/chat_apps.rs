use gpui::{
    App, ClickEvent, Context, Div, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _, ScrollHandle,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use super::{BotForm, Mode};
use crate::model::bots::{AWAY, CHOICES, Row, UNPAIRED, note, sent};
use crate::notify;
use crate::store::Store;
use crate::theme::{colours, radius};
use crate::ui::account_picker::reveals;
use crate::ui::{self, press, text_field};

const LABEL: &str = "Chat apps";
const STOPS: [&str; 3] = ["bot-chat-apps-away", "bot-chat-apps-always", "bot-chat-apps-off"];
const PAIR_STOP: &str = "bot-chat-apps-pair";
const PAIR_TIP: &str = "Closes this form and opens Settings › Remote.";
const REMOTE: &str = "Remote";

pub(super) struct ChatRow {
    chosen: Option<String>,
    stops: [FocusHandle; 4],
    keyed: bool,
    due: bool,
}

impl ChatRow {
    pub(super) fn new(mode: &Mode, cx: &mut App) -> Self {
        let chosen = match mode {
            Mode::New => Some(AWAY.to_string()),
            Mode::Edit(editing) => editing.before.chat_apps.clone(),
        };
        Self { chosen, stops: std::array::from_fn(|_| cx.focus_handle()), keyed: false, due: false }
    }
}

impl BotForm {
    pub(super) fn chat_reveal(&mut self, window: &Window) {
        let now = self.chat.stops.iter().any(|handle| handle.is_focused(window));
        self.chat.due = reveals(now, self.chat.keyed, false);
        self.chat.keyed = now;
    }

    pub(super) fn chat_sent(&self, cx: &App) -> Option<String> {
        sent(&Store::get(cx).read(cx).chat_apps_row(), self.chat.chosen.as_deref())
    }

    fn chat_pressed(&mut self, event: &ClickEvent, at: usize, cx: &mut Context<Self>) {
        if press::pressed(event, STOPS[at]) {
            self.chat.chosen = Some(CHOICES[at].0.to_string());
            self.changed(cx);
        }
    }

    pub(super) fn chat_field(&self, cx: &mut Context<Self>) -> Option<Div> {
        let body = match Store::get(cx).read(cx).chat_apps_row() {
            Row::Hidden => return None,
            Row::Lacks(words) => ui::dim(words),
            Row::Unpaired => self.pair_line(),
            Row::Choices => self.chat_choices(cx),
        };
        let field = ui::field(LABEL, body);
        Some(if self.chat.due { revealed(field, &self.scroller) } else { field })
    }

    fn pair_line(&self) -> Div {
        let line = div()
            .id(PAIR_STOP)
            .px(px(4.))
            .py(px(2.))
            .ml(px(-4.))
            .rounded(px(radius::control()))
            .text_size(px(11.))
            .text_color(colours::faint())
            .cursor_pointer()
            .hover(|s| s.text_color(colours::soft()))
            .child(UNPAIRED);
        let stop = press::stop_at(line, &self.chat.stops[3], PAIR_STOP, true).tooltip(notify::tip(PAIR_TIP));
        div().flex().child(stop.on_click(pair_pressed))
    }

    fn chat_choices(&self, cx: &mut Context<Self>) -> Div {
        let chosen = self.chat.chosen.as_deref();
        let items = CHOICES
            .iter()
            .zip(STOPS)
            .zip(&self.chat.stops)
            .enumerate()
            .map(|(at, ((&(id, label), name), handle))| {
                press::stop_at(ui::segment(name, label, chosen == Some(id)), handle, name, true)
                    .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| this.chat_pressed(event, at, cx)))
                    .into_any_element()
            })
            .collect();
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(div().flex().child(ui::segments(items)))
            .children(note(chosen).map(ui::dim))
    }
}

fn pair_pressed(event: &ClickEvent, _: &mut Window, cx: &mut App) {
    if press::pressed(event, PAIR_STOP) {
        Store::get(cx).update(cx, open_remote);
    }
}

fn open_remote(store: &mut Store, cx: &mut Context<Store>) {
    store.close_dialog(cx);
    store.open_settings_page(REMOTE, cx);
}

fn revealed(field: Div, scroller: &ScrollHandle) -> Div {
    let scroller = scroller.clone();
    div().child(field).on_children_prepainted(move |bounds, window, _| {
        if let Some(found) = bounds.first() {
            text_field::show_box(&scroller, (found.top(), found.size.height), window);
        }
    })
}
