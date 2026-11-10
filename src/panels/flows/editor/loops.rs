use gpui::{
    AnyElement, ClickEvent, Context, Div, IntoElement as _, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use super::super::draft::{Back, reports, targets};
use super::super::editor_view::{adder, choice};
use super::{FlowEditor, StageFields};
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, boxed};

impl FlowEditor {
    pub(in crate::panels::flows) fn loop_part(
        index: usize,
        stage: &StageFields,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let Some(back) = stage.back else {
            return (index > 0).then(|| {
                adder(
                    ("editor-add-loop", index),
                    Icon::Repeat,
                    "Add a loop back",
                    true,
                    cx.listener(move |this, _: &ClickEvent, _, cx| this.add_loop(index, cx)),
                )
                .into_any_element()
            });
        };
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(7.))
                .px(px(10.))
                .py(px(7.))
                .rounded(px(radius::control()))
                .border_1()
                .border_dashed()
                .border_color(colours::edge())
                .bg(colours::inset())
                .text_size(px(12.))
                .text_color(colours::muted())
                .child(Self::when_row(index, stage, window, cx))
                .child(Self::back_row(index, back, cx))
                .into_any_element(),
        )
    }

    fn when_row(index: usize, stage: &StageFields, window: &Window, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(7.))
            .child(ui::icon(Icon::Repeat, 13.).text_color(colours::muted()))
            .child(div().flex_none().child(reports(stage.roles.len())))
            .child(boxed(&stage.when, window, cx).flex_1().min_w_0())
            .child(
                ui::tool_button(("loop-remove", index), Icon::Close, 22., false)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.remove_loop(index, cx))),
            )
    }

    fn back_row(index: usize, back: Back, cx: &Context<Self>) -> Div {
        let targets = targets(index).map(|n| Self::target(index, n, back.to == n, cx)).collect();
        div()
            .pl(px(20.))
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(7.))
            .child("go back to stage")
            .child(ui::segments(targets))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .whitespace_nowrap()
                    .child("at most")
                    .child(Self::stepper(index, back, cx))
                    .child(if back.at_most == 1 { "time" } else { "times" }),
            )
    }

    fn target(index: usize, to: usize, selected: bool, cx: &Context<Self>) -> AnyElement {
        let key = (SharedString::from(format!("loop-{index}")), to);
        choice(key, to.to_string(), selected, cx, move |this, _, cx| this.set_loop_to(index, to, cx))
    }

    fn stepper(index: usize, back: Back, cx: &Context<Self>) -> Div {
        let less = ui::letter_button(("loop-less", index), "−", 24.);
        let less = if back.lowest() {
            less.opacity(0.35).cursor_default()
        } else {
            less.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.step_loop(index, false, cx)))
        };
        let more = ui::letter_button(("loop-more", index), "+", 24.)
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.step_loop(index, true, cx)));
        div()
            .flex()
            .items_center()
            .h(px(26.))
            .rounded(px(radius::control()))
            .border_1()
            .border_color(colours::edge())
            .child(less)
            .child(
                div()
                    .px(px(4.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colours::text())
                    .child(back.at_most.to_string()),
            )
            .child(more)
    }
}
