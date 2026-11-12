use gpui::{
    ClickEvent, Div, InteractiveElement as _, ParentElement as _, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, div, px,
};

use super::data::Port;
use crate::notify::tip;
use crate::store::Asked;
use crate::store::Control;
use crate::store::Store;
use crate::theme::colours;
use crate::ui::{self, Tone};

pub(super) fn tunnel_button(port: &Port, store: &Store) -> Option<Stateful<Div>> {
    port.url.as_ref()?;
    let number = port.number;
    let id = SharedString::from(format!("tunnel-{}", port.key()));
    match store.tunnel_control(number) {
        Control::Hidden => None,
        Control::Tunnel => Some(ui::button(id, "Tunnel").on_click(move |_: &ClickEvent, _, cx| {
            Store::get(cx).update(cx, |s, cx| s.hold_tunnel(number, Asked::Click, cx));
        })),
        Control::Opening => Some(ui::busy_button(id, "Tunnelling…", Tone::Plain)),
        Control::Close => Some(ui::button(id, "Close tunnel").on_click(move |_: &ClickEvent, _, cx| {
            Store::get(cx).update(cx, |s, cx| s.close_tunnel(number, false, cx));
        })),
        Control::Closing => Some(ui::busy_button(id, "Closing…", Tone::Plain)),
    }
}

pub(super) fn tunnel_mark(port: &Port, store: &Store) -> Option<Stateful<Div>> {
    port.url.as_ref()?;
    let mark = store.tunnel_mark(port.number)?;
    let id = SharedString::from(format!("tunnel-mark-{}", port.key()));
    let colour = if mark.waiting { colours::waiting() } else { colours::accent() };
    let text = div().id(id).text_size(px(11.)).text_color(colour).child(mark.text);
    Some(match mark.tip {
        Some(why) => text.tooltip(tip(why)),
        None => text,
    })
}
