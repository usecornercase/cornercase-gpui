use gpui::{Div, ParentElement as _, Rgba, Styled as _, div, linear_color_stop, linear_gradient, px, relative};

use crate::theme::{colour, colours, palette_rgba};

const EYE_WIDTH: f32 = 0.12;
const EYE_HEIGHT: f32 = 0.22;
const EYE_TOP: f32 = 0.34;
const EYE_GAP: f32 = 0.16;
const SHINE: f32 = 0.18;
const EYES: Rgba = Rgba { r: 0.07, g: 0.07, b: 0.09, a: 0.86 };

const BRIGHT_LIFT: f32 = 0.32;

fn body(colour_index: u8) -> Rgba {
    let own = palette_rgba(colour_index);
    let plain = colour_index.checked_sub(8).filter(|_| colour_index > 8).map(palette_rgba);
    if plain == Some(own) { colour::mix(own, colours::white(), BRIGHT_LIFT) } else { own }
}

pub(crate) fn face(colour_index: u8, size: f32) -> Div {
    let body = body(colour_index);
    let lit = colour::mix(body, colours::white(), SHINE);
    let eye = |left: f32| {
        div()
            .absolute()
            .left(relative(left))
            .top(relative(EYE_TOP))
            .w(relative(EYE_WIDTH))
            .h(relative(EYE_HEIGHT))
            .rounded_full()
            .bg(EYES)
    };
    let middle = 0.5 - EYE_WIDTH / 2.;
    div()
        .relative()
        .flex_none()
        .size(px(size))
        .rounded_tl(px(size * 0.5))
        .rounded_tr(px(size * 0.46))
        .rounded_br(px(size * 0.5))
        .rounded_bl(px(size * 0.42))
        .bg(linear_gradient(160., linear_color_stop(lit, 0.), linear_color_stop(body, 1.)))
        .child(eye(middle - EYE_GAP / 2. - EYE_WIDTH / 2.))
        .child(eye(middle + EYE_GAP / 2. + EYE_WIDTH / 2.))
}

pub(crate) fn crowd(colour_indices: &[u8], size: f32) -> Div {
    let overlap = (size * 0.28).round();
    let faces = colour_indices.iter().enumerate().map(|(at, index)| {
        let face = face(*index, size).border_2().border_color(colours::raised_hover());
        if at == 0 { face } else { face.ml(px(-overlap)) }
    });
    div().flex().items_end().children(faces)
}
