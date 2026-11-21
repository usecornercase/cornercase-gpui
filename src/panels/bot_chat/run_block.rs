use gpui::{
    AnyElement, ClickEvent, Div, FontWeight, IntoElement, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, div, px,
};

use super::entries::Block;
use super::picking::{ERROR_PART, Spot};
use super::rows::{Meta, answer, error_with, line, steps};
use super::runs::offers_again;
use super::words::{ANSWERING, RUN_AGAIN, STARTING};
use crate::store::Store;
use crate::theme::colours;
use crate::ui::{self, Tone, press};

pub(super) fn again_stop(run: u64) -> String {
    format!("bot-run-again-{run}")
}

fn title(block: &Block) -> Div {
    div().font_weight(FontWeight::SEMIBOLD).text_size(px(12.)).text_color(colours::muted()).child(block.title.clone())
}

fn answering(run: u64) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .text_size(px(11.5))
        .text_color(colours::muted())
        .child(ui::spinner(("bot-run-answering", run), 11., colours::working()))
        .child(ANSWERING)
}

fn end(text: &str) -> Div {
    div().text_size(px(11.5)).text_color(colours::faint()).child(text.to_string())
}

fn other(text: &str) -> Div {
    div().text_color(colours::muted()).child(text.to_string())
}

fn again_button(run: u64, meta: &Meta) -> Option<AnyElement> {
    if meta.again_busy.contains(&run) {
        return Some(ui::busy_button(("bot-run-again", run), STARTING, Tone::Plain).into_any_element());
    }
    let handle = meta.again_stops.get(&run)?;
    let name = again_stop(run);
    let button = press::stop_at(ui::button(("bot-run-again", run), RUN_AGAIN), handle, name.clone(), meta.stops);
    let pressed = button.on_click(move |event: &ClickEvent, _, cx| {
        if press::pressed(event, &name) {
            Store::get(cx).update(cx, |store, cx| store.run_again(run, cx));
        }
    });
    Some(pressed.into_any_element())
}

fn error(block: &Block, meta: &Meta) -> Option<Div> {
    let text = block.error.as_deref()?;
    let again = offers_again(block, &meta.runs).then(|| again_button(block.run, meta)).flatten();
    Some(error_with((Spot { entry: block.head, part: ERROR_PART }, text), again, meta))
}

pub(super) fn run(block: &Block, meta: &Meta) -> Div {
    let steps = (!block.steps.is_empty()).then(|| steps(block.head, &block.steps, 0, meta));
    let answer = block.answer.as_ref().map(|(id, text, at)| answer((block.head, *id), text, *at, meta));
    let column = div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .pl(px(12.))
        .border_l_2()
        .border_color(colours::border())
        .child(title(block))
        .children(steps)
        .children(answer)
        .children(block.others.iter().map(|text| other(text)))
        .children(error(block, meta))
        .children(block.end.as_deref().map(end))
        .children(meta.runs.answers(block.run).then(|| answering(block.run)));
    line(column)
}
