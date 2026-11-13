use gpui::{
    ClickEvent, Div, InteractiveElement as _, ParentElement as _, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, div, px,
};

use super::view::PortView;
use crate::model::tunnels::Toward;
use crate::notify::tip;
use crate::store::Asked;
use crate::store::Control;
use crate::store::Store;
use crate::theme::colours;
use crate::ui::{self, Tone};

pub(super) fn tunnel_button(view: &PortView, toward: &Toward, store: &Store) -> Option<Stateful<Div>> {
    view.url.as_ref()?;
    let number = view.number;
    let id = SharedString::from(format!("tunnel-{}", view.key));
    let (open, close) = (toward.clone(), toward.clone());
    match store.tunnel_control(toward, number) {
        Control::Hidden => None,
        Control::Tunnel => Some(ui::button(id, "Tunnel").on_click(move |_: &ClickEvent, _, cx| {
            Store::get(cx).update(cx, |s, cx| s.hold_tunnel(&open, number, Asked::Click, cx));
        })),
        Control::Opening => Some(ui::busy_button(id, "Tunnelling…", Tone::Plain)),
        Control::Close => Some(ui::button(id, "Close tunnel").on_click(move |_: &ClickEvent, _, cx| {
            Store::get(cx).update(cx, |s, cx| s.close_tunnel(&close, number, false, cx));
        })),
        Control::Closing => Some(ui::busy_button(id, "Closing…", Tone::Plain)),
    }
}

pub(super) fn tunnel_mark(view: &PortView, toward: &Toward, store: &Store) -> Option<Stateful<Div>> {
    view.url.as_ref()?;
    let mark = store.tunnel_mark(toward, view.number)?;
    let id = SharedString::from(format!("tunnel-mark-{}", view.key));
    let colour = if mark.waiting { colours::waiting() } else { colours::accent() };
    let text = div().id(id).text_size(px(11.)).text_color(colour).child(mark.text);
    Some(match mark.tip {
        Some(why) => text.tooltip(tip(why)),
        None => text,
    })
}
