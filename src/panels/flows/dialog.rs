use gpui::{
    App, AppContext as _, ClickEvent, Context, Div, Entity, Focusable as _, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use crate::store::Store;
use crate::ui::{self, TextField, TextFieldEvent};

pub(super) trait Submit: Sized + 'static {
    fn submit(&mut self, cx: &mut Context<Self>);
}

pub(super) fn close_dialog(cx: &mut App) {
    Store::get(cx).update(cx, Store::close_dialog);
}

pub(super) fn field<T: Submit>(placeholder: &str, cx: &mut Context<T>, window: &mut Window) -> Entity<TextField> {
    let field = cx.new(|cx| TextField::new(placeholder.to_string(), cx));
    cx.subscribe_in(&field, window, |this, _, event, _, cx| match event {
        TextFieldEvent::Submit => this.submit(cx),
        TextFieldEvent::Cancel => close_dialog(cx),
        TextFieldEvent::Changed => cx.notify(),
    })
    .detach();
    field
}

pub(super) fn boxed(field: &Entity<TextField>, window: &Window, cx: &App) -> Div {
    ui::input_box(field.clone(), field.read(cx).focus_handle(cx).is_focused(window))
}

pub(super) fn buttons<T: Submit>(ids: (&'static str, &'static str), label: &'static str, cx: &Context<T>) -> Div {
    div()
        .flex()
        .gap(px(8.))
        .child(ui::button(ids.0, "Cancel").on_click(|_: &ClickEvent, _, cx| close_dialog(cx)))
        .child(ui::primary_button(ids.1, label).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.submit(cx))))
}
