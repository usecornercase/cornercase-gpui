use gpui::{
    AnyElement, ClickEvent, Context, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, div, list, px,
};

use super::QueuePanel;
use super::detail::View;
use super::lines::picked;
use crate::theme::{FONT_FAMILY, colours};
use crate::ui::selection::{self, CODE_ROW, Fit, Lit};

const GUTTER: f32 = 12.;
const TEXT_SIZE: f32 = 12.5;
const BOTTOM: f32 = 24.;
const TOP: f32 = 8.;

impl QueuePanel {
    fn line_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let detail = self.detail.as_ref()?;
        let selection = detail.selecting.selection.filter(|s| !s.is_empty())?;
        let copy = selection::action("queue-detail-copy-lines", "Copy").on_click(cx.listener(
            |this, _: &ClickEvent, _, cx| {
                this.copy_selection(cx);
            },
        ));
        Some(selection::action_bar(picked(selection), vec![copy.into_any_element()]).into_any_element())
    }

    fn text_line(&self, ix: usize) -> AnyElement {
        let Some(detail) = self.detail.as_ref() else { return div().into_any_element() };
        let Some(line) = detail.lines().get(ix) else { return div().into_any_element() };
        let lit = detail.selecting.lit(ix, line);
        let fit = Fit::of(true, px(self.side_width.size() - GUTTER - 8.));
        let frame = div()
            .flex()
            .w_full()
            .line_height(px(CODE_ROW))
            .font_family(FONT_FAMILY)
            .text_size(px(TEXT_SIZE))
            .text_color(colours::soft());
        let frame = if lit == Lit::Line { frame.bg(colours::selected()) } else { frame };
        let frame = if ix == 0 { frame.pt(px(TOP)) } else { frame };
        frame
            .child(div().w(px(GUTTER)).flex_none())
            .child(selection::code_cell(detail.selecting.laid(), (ix, line), (Vec::new(), &lit), fit))
            .into_any_element()
    }

    pub(super) fn text_body(&self, detail: &super::detail::Detail, cx: &mut Context<Self>) -> AnyElement {
        detail.selecting.laid().clear();
        let state = detail.lists.of(detail.page, View::Text).clone();
        let lines = list(state, cx.processor(|this, ix: usize, _, _| this.text_line(ix)));
        let bar = self.line_bar(cx);
        selection::body(cx)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, window, _| window.focus(&this.focus)),
            )
            .child(lines.size_full().pb(px(BOTTOM)))
            .child(selection::layer(cx.weak_entity(), &detail.selecting, bar, 0.))
            .into_any_element()
    }
}
