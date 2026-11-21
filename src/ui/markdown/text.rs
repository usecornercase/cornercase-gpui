use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, App, ElementId, FontStyle, FontWeight, HighlightStyle, InteractiveText, IntoElement,
    ParentElement as _, SharedString, StrikethroughStyle, Styled as _, StyledText, UnderlineStyle, div,
    prelude::FluentBuilder as _, px,
};

use super::{Block, Code, Inline, Mark};
use crate::model::link::is_web;
use crate::store::{Store, ToastKind};
use crate::theme::code::Inks;
use crate::theme::{self, FONT_FAMILY};
use crate::theme::{colours, radius};
use crate::ui::highlight::Span;
use crate::ui::selection::Pick;

pub(crate) type OnLink = Rc<dyn Fn(&str, &mut App)>;
pub(crate) type Picks<'a> = &'a dyn Fn(usize, &str) -> Option<Pick>;

struct Drawing<'a> {
    on_link: &'a OnLink,
    pick: Option<Pick>,
}

pub(crate) fn open_web(address: &str, cx: &mut App) {
    if is_web(address) {
        cx.open_url(address);
    } else {
        let text = format!("{address} is not a web address, so it cannot be opened");
        Store::get(cx).update(cx, |s, cx| s.toast(text, ToastKind::Info, cx));
    }
}

pub(crate) fn opens_web_addresses() -> OnLink {
    Rc::new(open_web)
}

pub(crate) fn blocks(id: &str, blocks: &[Block]) -> Vec<AnyElement> {
    linked_blocks(id, blocks, &opens_the_address())
}

pub(crate) fn linked_blocks(id: &str, blocks: &[Block], on_link: &OnLink) -> Vec<AnyElement> {
    blocks.iter().enumerate().map(|(i, b)| linked_block(SharedString::from(format!("{id}-{i}")), b, on_link)).collect()
}

pub(crate) fn picked_blocks(id: &str, blocks: &[Block], on_link: &OnLink, picks: Picks<'_>) -> Vec<AnyElement> {
    let drawn = |(i, block): (usize, &Block)| {
        let pick = block.picked_text().and_then(|text| picks(i, &text));
        drawn_block(SharedString::from(format!("{id}-{i}")), block, Drawing { on_link, pick })
    };
    blocks.iter().enumerate().map(drawn).collect()
}

pub(crate) fn raw(source: &str) -> AnyElement {
    code_box(&Code::plain(source), None)
}

pub(crate) fn block(id: SharedString, block: &Block) -> AnyElement {
    linked_block(id, block, &opens_the_address())
}

fn opens_the_address() -> OnLink {
    Rc::new(|url, cx| cx.open_url(url))
}

pub(crate) fn linked_block(id: SharedString, block: &Block, on_link: &OnLink) -> AnyElement {
    drawn_block(id, block, Drawing { on_link, pick: None })
}

fn drawn_block(id: SharedString, block: &Block, drawing: Drawing<'_>) -> AnyElement {
    match block {
        Block::Heading(level, text) => heading(*level, inline((id, 0), text, drawing)),
        Block::Paragraph(text) => div().child(inline((id, 0), text, drawing)).into_any_element(),
        Block::Item(depth, marker, text) => item(*depth, marker, inline((id, 0), text, drawing)),
        Block::Quote(text) => div()
            .pl(px(10.))
            .border_l_2()
            .border_color(colours::selected())
            .text_color(colours::muted())
            .child(inline((id, 0), text, drawing))
            .into_any_element(),
        Block::Code(code) => code_box(code, drawing.pick),
        Block::Table(rows) => table(rows),
        Block::Rule => div().h(px(1.)).my(px(4.)).bg(colours::border()).into_any_element(),
    }
}

fn heading(level: u8, text: AnyElement) -> AnyElement {
    let size = match level {
        1 => 17.,
        2 => 15.,
        _ => 13.5,
    };
    div()
        .pt(px(if level <= 2 { 8. } else { 4. }))
        .text_size(px(size))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(colours::text())
        .when(level <= 2, |d| d.pb(px(4.)).border_b_1().border_color(colours::border()))
        .child(text)
        .into_any_element()
}

fn item(depth: usize, marker: &str, text: AnyElement) -> AnyElement {
    let done = marker == "☑";
    let depth = f32::from(u8::try_from(depth).unwrap_or(u8::MAX));
    div()
        .flex()
        .gap(px(6.))
        .pl(px(4. + 16. * depth))
        .child(
            div()
                .flex_none()
                .min_w(px(14.))
                .text_color(if done { colours::done() } else { colours::faint() })
                .child(marker.to_string()),
        )
        .child(div().flex_1().min_w_0().child(text))
        .into_any_element()
}

fn code_box(code: &Code, pick: Option<Pick>) -> AnyElement {
    div()
        .px(px(12.))
        .py(px(10.))
        .rounded(px(radius::control()))
        .bg(colours::terminal())
        .border_1()
        .border_color(colours::border())
        .font_family(FONT_FAMILY)
        .text_size(px(12.))
        .text_color(colours::text())
        .overflow_hidden()
        .children(code.shown().map(|shown| code_lines(shown, code.spans(), pick)))
        .into_any_element()
}

fn code_lines(shown: String, spans: &[Span], pick: Option<Pick>) -> AnyElement {
    let inked = || styles(spans, &theme::code_inks());
    match pick {
        Some(pick) => pick.draw(shown, inked(), IntoElement::into_any_element).into_any_element(),
        None if spans.is_empty() => shown.into_any_element(),
        None => StyledText::new(shown).with_highlights(inked()).into_any_element(),
    }
}

pub(super) fn styles(spans: &[Span], inks: &Inks) -> Vec<(Range<usize>, HighlightStyle)> {
    spans
        .iter()
        .map(|(range, kind)| {
            let style = match kind.ansi() {
                Some(index) => {
                    HighlightStyle { color: Some(inks.ansi[usize::from(index)].into()), ..Default::default() }
                }
                None => HighlightStyle {
                    color: Some(inks.comment.into()),
                    font_style: Some(FontStyle::Italic),
                    ..Default::default()
                },
            };
            (range.clone(), style)
        })
        .collect()
}

fn table(rows: &[Vec<String>]) -> AnyElement {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    div()
        .flex()
        .flex_col()
        .rounded(px(radius::control()))
        .border_1()
        .border_color(colours::border())
        .text_size(px(12.))
        .children(rows.iter().enumerate().map(move |(r, row)| {
            div()
                .flex()
                .when(r == 0, |d| {
                    d.font_weight(FontWeight::SEMIBOLD).rounded_t(px(radius::control() - 1.)).bg(colours::inset())
                })
                .when(r > 0, |d| d.border_t_1().border_color(colours::border()))
                .children((0..columns).map(|c| {
                    div().flex_1().min_w_0().px(px(8.)).py(px(3.)).child(row.get(c).cloned().unwrap_or_default())
                }))
        }))
        .into_any_element()
}

fn inline(id: impl Into<ElementId>, text: &Inline, drawing: Drawing<'_>) -> AnyElement {
    let highlights: Vec<_> = text.runs().into_iter().map(|(range, marks)| (range, style(&marks))).collect();
    let linked = |styled: StyledText| linked(id, styled, text, drawing.on_link);
    match drawing.pick {
        Some(pick) => pick.draw(text.text.clone(), highlights, linked).into_any_element(),
        None => linked(StyledText::new(text.text.clone()).with_highlights(highlights)),
    }
}

fn linked(id: impl Into<ElementId>, styled: StyledText, text: &Inline, on_link: &OnLink) -> AnyElement {
    if text.links.is_empty() {
        return styled.into_any_element();
    }
    let (ranges, urls): (Vec<_>, Vec<_>) = text.links.iter().cloned().unzip();
    let on_link = on_link.clone();
    InteractiveText::new(id, styled)
        .on_click(ranges, move |i, _, cx| {
            if let Some(url) = urls.get(i) {
                on_link(url, cx);
            }
        })
        .into_any_element()
}

fn style(marks: &[Mark]) -> HighlightStyle {
    let mut style = HighlightStyle::default();
    for mark in marks {
        match mark {
            Mark::Bold => style.font_weight = Some(FontWeight::BOLD),
            Mark::Italic => style.font_style = Some(FontStyle::Italic),
            Mark::Strike => {
                style.strikethrough = Some(StrikethroughStyle { thickness: px(1.), color: None });
            }
            Mark::Code => {
                style.color = Some(colours::working().into());
                style.background_color = Some(colours::terminal().into());
            }
            Mark::Link => {
                style.color = Some(colours::accent().into());
                style.underline =
                    Some(UnderlineStyle { thickness: px(1.), color: Some(colours::accent().into()), wavy: false });
            }
        }
    }
    style
}

#[cfg(test)]
mod tests;
