use std::ops::Range;

use gpui::{App, Bounds, ElementInputHandler, Entity, InputHandler, Pixels, Point, UTF16Selection, Window};

use super::TextField;

pub(super) struct Handler {
    inner: ElementInputHandler<TextField>,
    masked: bool,
}

impl Handler {
    pub(super) fn new(bounds: Bounds<Pixels>, field: Entity<TextField>, masked: bool) -> Self {
        Self { inner: ElementInputHandler::new(bounds, field), masked }
    }
}

fn press_and_hold(masked: bool) -> bool {
    !masked
}

fn taken(text: &str, masked: bool) -> bool {
    !masked || text.is_empty() || !text.chars().all(char::is_control)
}

impl InputHandler for Handler {
    fn selected_text_range(&mut self, ignore: bool, window: &mut Window, cx: &mut App) -> Option<UTF16Selection> {
        self.inner.selected_text_range(ignore, window, cx)
    }

    fn marked_text_range(&mut self, window: &mut Window, cx: &mut App) -> Option<Range<usize>> {
        self.inner.marked_text_range(window, cx)
    }

    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<String> {
        self.inner.text_for_range(range, adjusted, window, cx)
    }

    fn replace_text_in_range(&mut self, range: Option<Range<usize>>, text: &str, window: &mut Window, cx: &mut App) {
        if taken(text, self.masked) {
            self.inner.replace_text_in_range(range, text, window, cx);
        }
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.inner.replace_and_mark_text_in_range(range, text, selected, window, cx);
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut App) {
        self.inner.unmark_text(window, cx);
    }

    fn bounds_for_range(&mut self, range: Range<usize>, window: &mut Window, cx: &mut App) -> Option<Bounds<Pixels>> {
        self.inner.bounds_for_range(range, window, cx)
    }

    fn character_index_for_point(&mut self, point: Point<Pixels>, window: &mut Window, cx: &mut App) -> Option<usize> {
        self.inner.character_index_for_point(point, window, cx)
    }

    fn apple_press_and_hold_enabled(&mut self) -> bool {
        press_and_hold(self.masked)
    }
}

#[cfg(test)]
mod tests;
