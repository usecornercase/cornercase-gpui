use gpui::{
    App, ClickEvent, Context, Div, FocusHandle, IntoElement, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, Window, div, px,
};

use super::{BotForm, Mode, revealed};
use crate::model::bots::{FollowRow, follow_note, follow_sent};
use crate::store::Store;
use crate::theme::radius;
use crate::ui::account_picker::reveals;
use crate::ui::{self, press};

const LABEL: &str = "Follow agents";
const STOP: &str = "bot-follow";
const TICK: &str = "Tell it when an agent it starts finishes";

pub(super) struct FollowBox {
    on: bool,
    stop: FocusHandle,
    keyed: bool,
    due: bool,
}

impl FollowBox {
    pub(super) fn new(mode: &Mode, cx: &mut App) -> Self {
        let on = match mode {
            Mode::New => true,
            Mode::Edit(editing) => editing.before.follow.unwrap_or(true),
        };
        Self { on, stop: cx.focus_handle(), keyed: false, due: false }
    }
}

impl BotForm {
    pub(super) fn follow_reveal(&mut self, window: &Window) {
        let now = self.follow.stop.is_focused(window);
        self.follow.due = reveals(now, self.follow.keyed, false);
        self.follow.keyed = now;
    }

    pub(super) fn follow_sent(&self, cx: &App) -> Option<bool> {
        follow_sent(&Store::get(cx).read(cx).follow_row(), self.follow.on)
    }

    fn follow_pressed(&mut self, event: &ClickEvent, cx: &mut Context<Self>) {
        if press::pressed(event, STOP) {
            self.follow.on = !self.follow.on;
            self.changed(cx);
        }
    }

    pub(super) fn follow_field(&self, cx: &mut Context<Self>) -> Option<Div> {
        let body = match Store::get(cx).read(cx).follow_row() {
            FollowRow::Hidden => return None,
            FollowRow::Lacks(words) => ui::dim(words),
            FollowRow::Box => self.follow_box(cx),
        };
        let field = ui::field(LABEL, body);
        Some(if self.follow.due { revealed(field, &self.scroller) } else { field })
    }

    fn follow_box(&self, cx: &mut Context<Self>) -> Div {
        let tick = ui::checkbox(STOP, self.follow.on, TICK)
            .px(px(4.))
            .py(px(2.))
            .ml(px(-4.))
            .rounded(px(radius::control()))
            .text_size(px(12.));
        let tick = press::stop_at(tick, &self.follow.stop, STOP, true)
            .on_click(cx.listener(|this, event: &ClickEvent, _, cx| this.follow_pressed(event, cx)));
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(div().flex().child(tick.into_any_element()))
            .child(ui::dim(follow_note(self.follow.on)))
    }
}
