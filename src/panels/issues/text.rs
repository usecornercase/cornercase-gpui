//! Draws the Markdown blocks of an issue's body or comment as formatted text: headings, lists,
//! quotes, code in the terminal's font, tables, and links that open in the browser.

use gpui::{
    AnyElement, ElementId, FontStyle, FontWeight, HighlightStyle, InteractiveText, IntoElement, ParentElement as _,
    SharedString, StrikethroughStyle, Styled as _, StyledText, UnderlineStyle, div, prelude::FluentBuilder as _, px,
};

use super::markdown::{Block, Inline, Mark};
use crate::theme::FONT_FAMILY;
use crate::theme::colours;

/// The blocks of `source`, one element each; `id` keeps their link ids apart from other texts'.
pub(super) fn markdown(id: &str, source: &str) -> Vec<AnyElement> {
    super::markdown::parse(source)
        .into_iter()
        .enumerate()
        .map(|(i, block)| self::block(SharedString::from(format!("{id}-{i}")), block))
        .collect()
}

/// The source as it is, in the terminal's font.
pub(super) fn raw(source: &str) -> AnyElement {
    code_box(source.to_string())
}

fn block(id: SharedString, block: Block) -> AnyElement {
    match block {
        Block::Heading(level, text) => {
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
                .child(inline((id, 0), text))
                .into_any_element()
        }
        Block::Paragraph(text) => div().child(inline((id, 0), text)).into_any_element(),
        Block::Item(depth, marker, text) => {
            let done = marker == "☑";
            div()
                .flex()
                .gap(px(6.))
                .pl(px(4. + 16. * depth as f32))
                .child(
                    div()
                        .flex_none()
                        .min_w(px(14.))
                        .text_color(if done { colours::done() } else { colours::faint() })
                        .child(marker),
                )
                .child(div().flex_1().min_w_0().child(inline((id, 0), text)))
                .into_any_element()
        }
        Block::Quote(text) => div()
            .pl(px(10.))
            .border_l_2()
            .border_color(colours::selected())
            .text_color(colours::muted())
            .child(inline((id, 0), text))
            .into_any_element(),
        Block::Code(text) => code_box(text),
        Block::Table(rows) => table(rows),
        Block::Rule => div().h(px(1.)).my(px(4.)).bg(colours::border()).into_any_element(),
    }
}

fn code_box(text: String) -> AnyElement {
    div()
        .px(px(10.))
        .py(px(8.))
        .rounded(px(5.))
        .bg(colours::terminal())
        .border_1()
        .border_color(colours::border())
        .font_family(FONT_FAMILY)
        .text_size(px(12.))
        .text_color(colours::text())
        .overflow_hidden()
        .children(text.lines().map(|line| {
            // Keep blank lines' height, and tabs readable.
            let line = line.replace('\t', "    ");
            div().min_h(px(16.)).child(if line.is_empty() { " ".to_string() } else { line })
        }))
        .into_any_element()
}

fn table(rows: Vec<Vec<String>>) -> AnyElement {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    div()
        .flex()
        .flex_col()
        .rounded(px(5.))
        .border_1()
        .border_color(colours::border())
        .text_size(px(12.))
        .children(rows.into_iter().enumerate().map(move |(r, row)| {
            div()
                .flex()
                .when(r == 0, |d| d.font_weight(FontWeight::SEMIBOLD).bg(colours::sidebar()))
                .when(r > 0, |d| d.border_t_1().border_color(colours::border()))
                .children((0..columns).map(|c| {
                    div().flex_1().min_w_0().px(px(8.)).py(px(3.)).child(row.get(c).cloned().unwrap_or_default())
                }))
        }))
        .into_any_element()
}

/// A line of text with its emphasis, code and links; a click on a link opens it.
pub(super) fn inline(id: impl Into<ElementId>, text: Inline) -> AnyElement {
    let highlights: Vec<_> = text.runs().into_iter().map(|(range, marks)| (range, style(&marks))).collect();
    let styled = StyledText::new(text.text.clone()).with_highlights(highlights);
    if text.links.is_empty() {
        return styled.into_any_element();
    }
    let (ranges, urls): (Vec<_>, Vec<_>) = text.links.into_iter().unzip();
    InteractiveText::new(id, styled)
        .on_click(ranges, move |i, _, cx| {
            if let Some(url) = urls.get(i) {
                cx.open_url(url);
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
