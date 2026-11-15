use gpui::{IntoElement, ParentElement as _, StatefulInteractiveElement as _, Styled as _, div, px};

use super::entries::{self, Control, new_conversation_control, stop_control};
use super::words::{
    NEW_CONVERSATION, STARTING, STOP, STOPPING, answering, new_conversation_message, new_conversation_title, turn_label,
};
use crate::dialogs::{self, Question};
use crate::model::bots::{Bot, Chat};
use crate::notify;
use crate::store::Store;
use crate::theme::colours;
use crate::ui::{self, Tone};

pub(super) fn header(bot: &Bot, starting: bool) -> impl IntoElement + use<> {
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
        .child(
            div().flex_1().min_w_0().text_size(px(11.5)).text_color(colours::faint()).child(ui::one_line(bot.facts())),
        )
        .child(new_conversation(bot, starting))
}

fn new_conversation(bot: &Bot, starting: bool) -> impl IntoElement + use<> {
    let slot = div().flex_none();
    match new_conversation_control(bot.thinking(), starting) {
        Control::Running => slot.child(ui::busy_button("bot-new-conversation", STARTING, Tone::Plain)),
        Control::Resting => slot.child(
            ui::resting_button("bot-new-conversation", NEW_CONVERSATION, Tone::Plain)
                .tooltip(notify::tip(answering(&bot.name))),
        ),
        Control::Ready => {
            let (id, name) = (bot.id.clone(), bot.name.clone());
            slot.child(ui::button("bot-new-conversation", NEW_CONVERSATION).on_click(move |_, window, cx| {
                let id = id.clone();
                let question = Question::new(
                    new_conversation_title(&name),
                    new_conversation_message(&name),
                    NEW_CONVERSATION,
                    Vec::new(),
                )
                .on_yes(move |cx| Store::get(cx).update(cx, |s, cx| s.new_conversation(&id, cx)));
                dialogs::confirm(question, window, cx);
            }))
        }
    }
}

pub(super) fn turn_row(bot: &Bot, chat: &Chat, now: i64, stopping: bool) -> Option<impl IntoElement + use<>> {
    let turn = entries::turn(&chat.lines, bot.thinking())
        .or_else(|| bot.thinking().then_some(entries::Turn { since: now, steps: 0 }))?;
    let id = bot.id.clone();
    let stop = match stop_control(stopping) {
        Control::Running | Control::Resting => ui::busy_button("bot-stop", STOPPING, Tone::Plain).into_any_element(),
        Control::Ready => ui::button("bot-stop", STOP)
            .on_click(move |_, _, cx| Store::get(cx).update(cx, |s, cx| s.stop_bot(&id, cx)))
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
