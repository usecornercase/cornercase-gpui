use std::rc::Rc;

use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, IntoElement, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, div, list, prelude::FluentBuilder as _, px,
};

use super::RecapPanel;
use super::data::{Choice, Page, writing};
use super::links;
use crate::notify::tip;
use crate::theme::colours;
use crate::ui::markdown::text::{self, OnLink};
use crate::ui::{self, Icon};

const SIDE: f32 = 14.;
const FAILED: &str = "The recap could not be written";
const ABOUT: &str = "A recap is what your agents did, in every project, summed up by an agent. It covers agent \
                     conversations only, not work done by hand.";
const FIRST_RUN: &str = "It reads your conversations, which can take a minute or more. You can close this panel \
                         meanwhile: the recap is here when you come back.";

impl RecapPanel {
    pub(super) fn toolbar(&self, running: bool, cx: &mut Context<Self>) -> Div {
        let choices = Choice::ALL.into_iter().map(|choice| self.choice_tab(choice, cx)).collect();
        div()
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .px(px(SIDE))
            .py(px(10.))
            .child(ui::segments(choices))
            .child(Self::refresh(running, cx))
    }

    fn choice_tab(&self, choice: Choice, cx: &mut Context<Self>) -> AnyElement {
        ui::segment(("recap-choice", choice.index()), choice.title(), self.choice == choice)
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.choose(choice, cx)))
            .into_any_element()
    }

    fn refresh(running: bool, cx: &mut Context<Self>) -> AnyElement {
        if running {
            return div()
                .size(px(28.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .child(ui::spinner("recap-running", 14., colours::muted()))
                .into_any_element();
        }
        ui::tool_button("recap-refresh", Icon::Refresh, 28., false)
            .tooltip(tip("Write it again"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.write_again(cx)))
            .into_any_element()
    }

    pub(super) fn notes(&self, running: bool, now: i64, cx: &mut Context<Self>) -> Div {
        let slot = self.slot();
        div()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(8.))
            .px(px(SIDE))
            .pb(px(10.))
            .border_b_1()
            .border_color(colours::border())
            .children(slot.page.as_ref().map(|page| heading(page, self.choice, now)))
            .when(running, |d| d.child(ui::busy("recap-writing", writing(slot.progress))))
            .when(!running, |d| d.children(self.failure(cx)))
            .children(slot.empty.as_ref().filter(|_| slot.page.is_some()).map(|empty| ui::note(empty.clone()).p_0()))
    }

    fn failure(&self, cx: &mut Context<Self>) -> Option<Div> {
        let error = self.slot().error.clone()?;
        Some(
            div()
                .flex()
                .flex_col()
                .items_start()
                .gap(px(8.))
                .child(ui::callout(colours::failed(), FAILED, error).w_full())
                .child(
                    ui::button("recap-retry", "Try again")
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.write_again(cx))),
                ),
        )
    }

    pub(super) fn body(&self, running: bool) -> AnyElement {
        let slot = self.slot();
        if let Some(page) = &slot.page {
            return text_of(page.clone());
        }
        if running {
            return explained(ui::dim(FIRST_RUN));
        }
        slot.empty.as_ref().map_or_else(
            || div().into_any_element(),
            |empty| {
                explained(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(6.))
                        .child(ui::emblem(Icon::Recap))
                        .child(div().text_size(px(13.)).child(empty.clone()))
                        .child(ui::dim(ABOUT)),
                )
            },
        )
    }
}

fn heading(page: &Page, choice: Choice, now: i64) -> Div {
    let label = page.answer.label.clone().unwrap_or_else(|| choice.title().to_string());
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(label))
        .child(byline(page, now))
}

fn byline(page: &Page, now: i64) -> Div {
    let parts = page.byline_parts(now).into_iter().map(ui::wrapped_fact).collect();
    ui::dotted(parts, Vec::new()).text_size(px(12.)).text_color(colours::faint())
}

fn explained(content: Div) -> AnyElement {
    div().flex_1().px(px(28.)).pt(px(48.)).text_center().child(content).into_any_element()
}

fn text_of(page: Rc<Page>) -> AnyElement {
    let list_state = page.list.clone();
    list(list_state, move |index, _, _| block(&page, index)).flex_1().into_any_element()
}

fn block(page: &Rc<Page>, index: usize) -> AnyElement {
    let Some(block) = page.blocks.get(index) else { return div().into_any_element() };
    let target = page.clone();
    let on_link: OnLink = Rc::new(move |link, cx| links::follow(link, &target.answer.places, cx));
    div()
        .w_full()
        .px(px(SIDE))
        .pt(px(if index == 0 { 10. } else { 6. }))
        .when(index + 1 == page.blocks.len(), |d| d.pb(px(20.)))
        .text_size(px(13.))
        .text_color(colours::text())
        .child(text::linked_block(format!("recap-{index}").into(), block, &on_link))
        .into_any_element()
}
