use std::cell::RefCell;

use gpui::{Bounds, FocusHandle, Pixels, WeakFocusHandle, Window};

thread_local! {
    static TYPING: RefCell<Option<(WeakFocusHandle, Bounds<Pixels>)>> = const { RefCell::new(None) };
}

pub(super) fn note(handle: &FocusHandle, bounds: Bounds<Pixels>, window: &Window) {
    if handle.is_focused(window) {
        TYPING.with(|typing| *typing.borrow_mut() = Some((handle.downgrade(), bounds)));
    }
}

pub(crate) fn typing_at(window: &Window) -> Option<Bounds<Pixels>> {
    TYPING.with(|typing| {
        let typing = typing.borrow();
        let (handle, bounds) = typing.as_ref()?;
        handle.upgrade().filter(|handle| handle.is_focused(window)).map(|_| *bounds)
    })
}
