use gpui::{
    App, ClickEvent, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use super::entries::{self, Control, new_conversation_control, stop_control};
use super::words::{
    NEW_CONVERSATION, ROUTINES, STARTING, STOP, STOPPING, answering, menu_tip, new_conversation_message,
    new_conversation_title, routines_tip, secrets_label, turn_label,
};
use crate::dialogs::{self, Question};
use crate::model::bots::{Bot, Chat};
use crate::model::bots::{Reach, reach_tip};
use crate::notify;
use crate::store::Store;
use crate::theme::colours;
use crate::ui::Icon;
use crate::ui::{self, Tone, press};

const NEW_CONVERSATION_STOP: &str = "bot-new-conversation";
const MENU_STOP: &str = "bot-menu";
const STOP_STOP: &str = "bot-stop";
const ROUTINES_STOP: &str = "bot-routines";

pub(super) fn header(
    bot: &Bot,
    starting: bool,
    (stops, routines): (bool, bool),
    reach: Option<&Reach>,
) -> impl IntoElement + use<> {
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(10.))
        .px(px(16.))
        .py(px(10.))
        .border_b_1()
        .border_color(colours::border())
        .child(
            div()
                .min_w_0()
                .max_w(gpui::relative(0.6))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(colours::text())
                .child(ui::one_line(bot.name.clone())),
        )
        .child(facts(bot))
        .children(reach.map(reach_icon))
        .children(routines.then(|| routines_button(bot, stops)))
        .child(new_conversation(bot, starting, stops))
        .child(menu_button(bot, stops))
}

fn facts(bot: &Bot) -> impl IntoElement + use<> {
    let facts = bot.facts();
    let secrets = secrets_label(bot.secrets, bot.secrets_unreadable).map(|(words, unreadable)| {
        let colour = if unreadable { colours::failed() } else { colours::faint() };
        let words = if facts.is_empty() { words } else { format!("· {words}") };
        div().flex_none().text_color(colour).child(words)
    });
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .gap(px(4.))
        .text_size(px(11.5))
        .text_color(colours::faint())
        .child(div().min_w_0().child(ui::one_line(facts)))
        .children(secrets)
}

fn reach_icon(reach: &Reach) -> impl IntoElement + use<> {
    div()
        .id("bot-chat-apps")
        .flex_none()
        .child(ui::icon(Icon::Phone, 13.).text_color(colours::faint()))
        .tooltip(notify::tip(reach_tip(reach)))
}

fn routines_button(bot: &Bot, stops: bool) -> impl IntoElement + use<> {
    let id = bot.id.clone();
    div().flex_none().child(
        press::stop(ui::button(ROUTINES_STOP, ROUTINES), ROUTINES_STOP, stops)
            .tooltip(notify::tip(routines_tip(&bot.name)))
            .on_click(move |event: &ClickEvent, _, cx| {
                if press::pressed(event, ROUTINES_STOP) {
                    Store::get(cx).update(cx, |s, cx| s.open_routines(&id, cx));
                }
            }),
    )
}

fn menu_button(bot: &Bot, stops: bool) -> impl IntoElement + use<> {
    let (id, keyed) = (bot.id.clone(), bot.id.clone());
    div().flex_none().child(
        press::stop(ui::icon_button(MENU_STOP, "⋯").text_color(colours::muted()), MENU_STOP, stops)
            .tooltip(notify::tip(menu_tip(&bot.name)))
            .on_mouse_down(MouseButton::Left, move |e: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
                Store::get(cx).update(cx, |s, cx| s.ask_bot_menu(&id, e.position, false, cx));
            })
            .on_click(move |event: &ClickEvent, _, cx| {
                if event.is_keyboard() && press::pressed(event, MENU_STOP) {
                    Store::get(cx).update(cx, |s, cx| s.ask_bot_menu(&keyed, event.position(), true, cx));
                }
            }),
    )
}

fn new_conversation(bot: &Bot, starting: bool, stops: bool) -> impl IntoElement + use<> {
    let slot = div().flex_none();
    match new_conversation_control(bot.thinking(), starting) {
        Control::Running => slot.child(ui::busy_button("bot-new-conversation", STARTING, Tone::Plain)),
        Control::Resting => slot.child(
            ui::resting_button("bot-new-conversation", NEW_CONVERSATION, Tone::Plain)
                .tooltip(notify::tip(answering(&bot.name))),
        ),
        Control::Ready => {
            let (id, name) = (bot.id.clone(), bot.name.clone());
            slot.child(
                press::stop(ui::button(NEW_CONVERSATION_STOP, NEW_CONVERSATION), NEW_CONVERSATION_STOP, stops)
                    .on_click(move |event, window, cx| {
                        if press::pressed(event, NEW_CONVERSATION_STOP) {
                            ask_new_conversation(&id, &name, window, cx);
                        }
                    }),
            )
        }
    }
}

pub(crate) fn ask_new_conversation(id: &str, name: &str, window: &mut Window, cx: &mut App) {
    let bot = id.to_string();
    let question =
        Question::new(new_conversation_title(name), new_conversation_message(name), NEW_CONVERSATION, Vec::new())
            .on_yes({
                let bot = bot.clone();
                move |cx| Store::get(cx).update(cx, |s, cx| s.new_conversation(&bot, cx))
            });
    dialogs::confirm_naming(&bot, question, window, cx);
}

pub(super) fn turn_row(
    bot: &Bot,
    chat: &Chat,
    now: i64,
    (stopping, stops): (bool, bool),
) -> Option<impl IntoElement + use<>> {
    let turn = entries::turn(&chat.lines, bot.thinking())
        .or_else(|| bot.thinking().then_some(entries::Turn { since: now, steps: 0 }))?;
    let id = bot.id.clone();
    let stop = match stop_control(stopping) {
        Control::Running | Control::Resting => ui::busy_button("bot-stop", STOPPING, Tone::Plain).into_any_element(),
        Control::Ready => press::stop(ui::button(STOP_STOP, STOP), STOP_STOP, stops)
            .on_click(move |event, _, cx| {
                if press::pressed(event, STOP_STOP) {
                    Store::get(cx).update(cx, |s, cx| s.stop_bot(&id, cx));
                }
            })
            .into_any_element(),
    };
    Some(
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(16.))
            .py(px(6.))
            .text_size(px(12.))
            .text_color(colours::muted())
            .child(ui::spinner("bot-thinking", 12., colours::working()))
            .child(div().flex_1().min_w_0().child(ui::one_line(turn_label(turn, now))))
            .child(div().flex_none().child(stop)),
    )
}
