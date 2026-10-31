use gpui::{
    App, AppContext as _, ClickEvent, Context, Div, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};

use crate::dialogs;
use crate::store::Store;
use crate::theme::{colours, radius};
use crate::ui;

pub(super) struct Question {
    pub(super) title: String,
    pub(super) message: String,
    pub(super) details: Vec<String>,
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
        if event.keystroke.key == "enter" {
            self.yes(cx);
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
                body(question),
                div()
                    .flex()
                    .gap(px(8.))
                    .child(ui::button("cancel", "Cancel").on_click(|_: &ClickEvent, _, cx| close(cx)))
                    .child(button.on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.yes(cx)))),
            ),
        )
    }
}

fn body(question: &Question) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(div().text_color(colours::muted()).child(question.message.clone()))
        .when(!question.details.is_empty(), |d| d.child(text_box("details".into(), &question.details, 300.)))
}

pub(super) fn text_box(id: SharedString, lines: &[String], height: f32) -> impl IntoElement + use<> {
    div()
        .id(id)
        .max_h(px(height))
        .overflow_y_scroll()
        .p(px(10.))
        .rounded(px(radius::CONTROL))
        .bg(colours::sidebar())
        .font_family("Menlo")
        .text_size(px(11.))
        .text_color(colours::muted())
        .children(lines.iter().map(|l| div().child(l.clone())).collect::<Vec<_>>())
}
