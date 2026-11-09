use std::rc::Rc;

use gpui::{
    App, AppContext as _, Div, DragMoveEvent, InteractiveElement as _, MouseButton, ParentElement as _, SharedString,
    Stateful, StatefulInteractiveElement as _, Styled as _, div, px, relative,
};

use super::split::Blank;
use super::track;
use crate::theme::colours;

const WIDTH: f32 = 200.;
const THUMB: f32 = 14.;
const HEIGHT: f32 = 20.;

#[derive(Clone)]
struct Grip(SharedString);

pub(crate) fn fraction_of(value: f32, (least, most): (f32, f32)) -> f32 {
    if most <= least { 0. } else { ((value - least) / (most - least)).clamp(0., 1.) }
}

pub(crate) fn value_at(fraction: f32, (least, most): (f32, f32), step: f32) -> f32 {
    let raw = least + fraction.clamp(0., 1.) * (most - least);
    if step <= 0. {
        return raw;
    }
    (((raw - least) / step).round() * step + least).clamp(least, most)
}

fn parts(id: &SharedString, at: f32) -> Div {
    let rail = div().absolute().top(px((HEIGHT - 4.) / 2.)).left_0().right_0().h(px(4.)).rounded_full();
    let fill = div().absolute().top(px((HEIGHT - 4.) / 2.)).left_0().w(relative(at)).h(px(4.)).rounded_full();
    let thumb = div()
        .absolute()
        .top(px((HEIGHT - THUMB) / 2.))
        .left(relative(at))
        .ml(px(-THUMB / 2.))
        .size(px(THUMB))
        .rounded_full()
        .bg(colours::raised())
        .border_1()
        .border_color(colours::edge());
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .left(px(THUMB / 2.))
        .right(px(THUMB / 2.))
        .child(track::remember(id))
        .child(rail.bg(colours::border()))
        .child(fill.bg(colours::brand()))
        .child(thumb)
}

pub(crate) fn slider(
    key: &str,
    value: f32,
    range: (f32, f32),
    step: f32,
    on_change: impl Fn(f32, &mut App) + 'static,
) -> Stateful<Div> {
    let id = SharedString::from(format!("slider-{key}"));
    let change = Rc::new(on_change);
    let (pressed, dragged) = (change.clone(), change);
    let (down_id, move_id) = (id.clone(), id.clone());
    div()
        .id(id.clone())
        .relative()
        .w(px(WIDTH))
        .h(px(HEIGHT))
        .cursor_pointer()
        .child(parts(&id, fraction_of(value, range)))
        .on_drag(Grip(id.clone()), |_, _, _, cx: &mut App| cx.new(|_| Blank))
        .on_mouse_down(MouseButton::Left, move |event, _, cx| {
            cx.stop_propagation();
            if let Some((x, _)) = track::pointer_along(&down_id, event.position) {
                pressed(value_at(x, range, step), cx);
            }
        })
        .on_drag_move::<Grip>(move |event: &DragMoveEvent<Grip>, _, cx| {
            if event.drag(cx).0 != move_id {
                return;
            }
            if let Some((x, _)) = track::pointer_along(&move_id, event.event.position) {
                dragged(value_at(x, range, step), cx);
            }
        })
}

#[cfg(test)]
mod tests;
