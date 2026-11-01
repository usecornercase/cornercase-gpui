use gpui::{
    App, AppContext as _, ClickEvent, Context, Div, Entity, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, Window, div, px,
};

use crate::store::Store;
use crate::ui::{self, Icon, TextField, TextFieldEvent, Tone};

pub(super) use crate::ui::boxed;

pub(super) trait Submit: Sized + 'static {
    fn submit(&mut self, cx: &mut Context<Self>);

    fn changed(&mut self, cx: &mut Context<Self>) {
        cx.notify();
    }
}

pub(super) fn close_dialog(cx: &mut App) {
    Store::get(cx).update(cx, Store::close_dialog);
}

pub(super) fn field<T: Submit>(placeholder: &str, cx: &mut Context<T>, window: &mut Window) -> Entity<TextField> {
    let field = cx.new(|cx| TextField::new(placeholder.to_string(), cx).in_tab_order());
    subscribed(field, cx, window)
}

pub(super) fn filled_field<T: Submit>(
    placeholder: &str,
    text: &str,
    cx: &mut Context<T>,
    window: &mut Window,
) -> Entity<TextField> {
    let text = text.to_string();
    let field = cx.new(|cx| {
        let mut field = TextField::new(placeholder.to_string(), cx).in_tab_order().wraps();
        field.set_text(text, cx);
        field
    });
    subscribed(field, cx, window)
}

fn subscribed<T: Submit>(field: Entity<TextField>, cx: &mut Context<T>, window: &mut Window) -> Entity<TextField> {
    cx.subscribe_in(&field, window, |this, _, event, _, cx| match event {
        TextFieldEvent::Submit => this.submit(cx),
        TextFieldEvent::Cancel => close_dialog(cx),
        TextFieldEvent::Changed => this.changed(cx),
    })
    .detach();
    field
}

pub(super) fn buttons<T: Submit>(
    ids: (&'static str, &'static str),
    (glyph, label): (Icon, &'static str),
    busy: Option<&'static str>,
    cx: &Context<T>,
) -> Div {
    let submit = match busy {
        Some(busy) => ui::busy_button(ids.1, busy, Tone::Primary),
        None => ui::icon_text_button(ids.1, glyph, label, Tone::Primary)
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.submit(cx))),
    };
    div()
        .flex()
        .gap(px(8.))
        .child(ui::button(ids.0, "Cancel").on_click(|_: &ClickEvent, _, cx| close_dialog(cx)))
        .child(submit)
}
