use std::cell::RefCell;
use std::collections::HashMap;

use gpui::{Bounds, IntoElement, Pixels, Point, SharedString, Styled as _, canvas};

thread_local! {
    static BOUNDS: RefCell<HashMap<SharedString, Bounds<Pixels>>> = RefCell::new(HashMap::new());
}

pub(crate) fn remember(id: &SharedString) -> impl IntoElement + use<> {
    let id = id.clone();
    canvas(
        move |bounds, _, _| {
            BOUNDS.with(|all| all.borrow_mut().insert(id.clone(), bounds));
        },
        |_, (), _, _| {},
    )
    .absolute()
    .size_full()
}

pub(crate) fn bounds_of(id: &str) -> Option<Bounds<Pixels>> {
    BOUNDS.with(|all| all.borrow().get(id).copied())
}

pub(crate) fn along(bounds: Bounds<Pixels>, at: Point<Pixels>) -> (f32, f32) {
    let ratio = |offset: Pixels, size: Pixels| {
        let size = f32::from(size);
        if size <= 0. { 0. } else { (f32::from(offset) / size).clamp(0., 1.) }
    };
    (ratio(at.x - bounds.left(), bounds.size.width), ratio(at.y - bounds.top(), bounds.size.height))
}

pub(crate) fn pointer_along(id: &str, at: Point<Pixels>) -> Option<(f32, f32)> {
    bounds_of(id).map(|bounds| along(bounds, at))
}

#[cfg(test)]
mod tests;
