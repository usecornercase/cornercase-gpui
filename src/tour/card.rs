use gpui::{
    ClickEvent, Context, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use super::Guide;
use super::model::{Step, Tour};
use crate::dialogs;
use crate::theme::{colours, radius};
use crate::ui;

const WIDTH: f32 = 340.;

fn move_button(
    id: &'static str,
    label: &'static str,
    action: &'static str,
    cx: &mut Context<Guide>,
) -> impl IntoElement {
    ui::button(id, label).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.command(&["tour", action], cx)))
}

fn heading(tour: &Tour, index: usize, step: &Step) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(ui::dim(format!("{} · {}/{}", tour.title, index + 1, tour.steps.len())))
        .child(div().font_weight(FontWeight::SEMIBOLD).child(step.title.clone()))
}

fn body(step: &Step) -> Div {
    div().flex().flex_col().gap(px(8.)).text_size(px(12.)).text_color(colours::soft()).child(step.text.clone()).when(
        step.offers_a_prompt(),
        |d| {
            d.child(
                div().flex().child(
                    ui::primary_button("tour-prompt", "Start an agent…")
                        .on_click(|_: &ClickEvent, window, cx| dialogs::start_agent(window, cx)),
                ),
            )
        },
    )
}

impl Guide {
    fn buttons(tour: &Tour, index: usize, step: &Step, cx: &mut Context<Self>) -> Div {
        let last = index + 1 == tour.steps.len();
        let docs = tour.docs.clone().map(SharedString::from);
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .children(
                docs.map(|url| {
                    ui::button("tour-docs", "Docs").on_click(move |_: &ClickEvent, _, cx| cx.open_url(&url))
                }),
            )
            .child(move_button("tour-end", "End", "end", cx))
            .child(div().flex_1())
            .when(index > 0, |d| d.child(move_button("tour-back", "Back", "back", cx)))
            .when(!step.explains(), |d| d.child(move_button("tour-skip", "Skip", "skip", cx)))
            .when(step.explains(), |d| {
                let label = if last { "Finish" } else { "Next" };
                d.child(
                    ui::primary_button("tour-next", label)
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.command(&["tour", "next"], cx))),
                )
            })
    }
}

impl Render for Guide {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some((tour, index, step)) = self.catalogue.current() else { return div() };
        let (tour, step) = (tour.clone(), step.clone());
        div().absolute().bottom(px(44.)).left_0().right_0().flex().justify_center().child(
            div().occlude().child(
                ui::glass::card(radius::CARD)
                    .w(px(WIDTH))
                    .p(px(16.))
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(heading(&tour, index, &step))
                    .child(body(&step))
                    .child(Self::buttons(&tour, index, &step, cx)),
            ),
        )
    }
}
