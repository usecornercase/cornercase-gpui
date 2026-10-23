use gpui::{
    App, AppContext as _, ClickEvent, Context, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use crate::dialogs;
use crate::store::Store;
use crate::theme::colours;
use crate::ui;

pub(super) struct Question {
    pub(super) title: String,
    pub(super) message: String,
    pub(super) button: &'static str,
    pub(super) danger: bool,
}

impl Question {
    pub(super) fn ask(self, window: &mut Window, cx: &mut App, on_yes: impl Fn(&mut App) + 'static) {
        let view = cx.new(|cx| Ask { question: self, on_yes: Box::new(on_yes), focus: cx.focus_handle() });
        let focus = view.read(cx).focus.clone();
        Store::get(cx).update(cx, |s, cx| s.show_dialog(view.into(), cx));
        window.focus(&focus);
    }
}

struct Ask {
    question: Question,
    on_yes: Box<dyn Fn(&mut App)>,
    focus: FocusHandle,
}

fn close(cx: &mut App) {
    Store::get(cx).update(cx, Store::close_dialog);
}

impl Ask {
    fn yes(&mut self, cx: &mut Context<Self>) {
        close(cx);
        (self.on_yes)(cx);
    }

    fn key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "enter" => self.yes(cx),
            "escape" => close(cx),
            _ => {}
        }
    }
}

impl Render for Ask {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let question = &self.question;
        let button = if question.danger {
            ui::danger_button("yes", question.button)
        } else {
            ui::primary_button("yes", question.button)
        };
        div().track_focus(&self.focus).on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| this.key(e, cx))).child(
            dialogs::frame(
                question.title.clone(),
                div().text_color(colours::muted()).child(question.message.clone()),
                div()
                    .flex()
                    .gap(px(8.))
                    .child(ui::button("cancel", "Cancel").on_click(|_: &ClickEvent, _, cx| close(cx)))
                    .child(button.on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.yes(cx)))),
            ),
        )
    }
}
