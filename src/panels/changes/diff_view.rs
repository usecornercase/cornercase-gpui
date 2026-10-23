use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, ClipboardItem, FontStyle, FontWeight, HighlightStyle, InteractiveElement as _,
    IntoElement, ListState, ParentElement as _, Rgba, SharedString, StatefulInteractiveElement as _, Styled as _,
    StyledText, WeakEntity, div, list, prelude::FluentBuilder as _, px,
};

use crate::store::ToastKind;
use crate::theme::{FONT_FAMILY, colours};

use super::pieces::{plural, toast};
use super::wire::{FileStatus, Hunk, LineKind, Segment};
use super::{ChangesPanel, DiffData, Look, Row, diff};

struct CodeLine<'a> {
    old: Option<u32>,
    new: Option<u32>,
    sign: char,
    text: &'a str,
    syntax: Option<&'a [Segment]>,
    emphasis: &'a [[usize; 2]],
    looks: &'a [Look],
    tint: Option<Rgba>,
    dim: bool,
}

pub(super) fn diff_list(state: &ListState, data: Rc<DiffData>, panel: WeakEntity<ChangesPanel>) -> AnyElement {
    list(state.clone(), move |ix, _window, _cx| diff_row(&data, ix, &panel)).size_full().into_any_element()
}

fn empty() -> AnyElement {
    div().into_any_element()
}

fn diff_row(data: &DiffData, ix: usize, panel: &WeakEntity<ChangesPanel>) -> AnyElement {
    let Some(row) = data.rows.get(ix) else { return empty() };
    match *row {
        Row::Gap { hunk, lines } => gap_button(hunk, lines, panel.clone()),
        Row::GapLine { hunk, line } => gap_line(data, hunk, line),
        Row::Line { hunk, line } => patch_line(data, hunk, line),
        Row::Hunk(i) => hunk_header(data, i),
    }
}

fn gap_button(hunk: usize, lines: u32, panel: WeakEntity<ChangesPanel>) -> AnyElement {
    div()
        .id(("gap", hunk))
        .w_full()
        .flex()
        .justify_center()
        .py(px(3.))
        .bg(colours::sidebar())
        .text_size(px(11.))
        .text_color(colours::faint())
        .cursor_pointer()
        .hover(|s| s.text_color(colours::accent()).bg(colours::hover()))
        .child(format!("⋯ {} unchanged", plural(lines as usize, "line")))
        .on_click(move |_: &ClickEvent, _, cx| {
            let _ = panel.update(cx, |this, cx| this.load_gap(hunk, cx));
        })
        .into_any_element()
}

fn gap_line(data: &DiffData, hunk: usize, line: usize) -> AnyElement {
    let Some((gap, looks)) = data.gaps.get(&hunk) else { return empty() };
    let Some(l) = gap.lines.get(line) else { return empty() };
    CodeLine {
        old: Some(l.old),
        new: Some(l.new),
        sign: ' ',
        text: &l.text,
        syntax: l.syntax.as_deref(),
        emphasis: &[],
        looks,
        tint: None,
        dim: true,
    }
    .render()
}

fn patch_line(data: &DiffData, hunk: usize, line: usize) -> AnyElement {
    let Some(l) = data.patch.hunks.get(hunk).and_then(|h| h.lines.get(line)) else { return empty() };
    let (sign, tint) = match l.kind {
        LineKind::Context => (' ', None),
        LineKind::Removed => ('-', Some(colours::failed())),
        LineKind::Added => ('+', Some(colours::done())),
    };
    CodeLine {
        old: l.old,
        new: l.new,
        sign,
        text: &l.text,
        syntax: l.syntax.as_deref(),
        emphasis: &l.emphasis,
        looks: &data.looks,
        tint,
        dim: false,
    }
    .render()
}

fn hunk_action(id: &'static str, i: usize, label: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id((id, i))
        .px(px(6.))
        .rounded(px(4.))
        .text_size(px(11.))
        .text_color(colours::muted())
        .cursor_pointer()
        .hover(|s| s.bg(colours::selected()).text_color(colours::text()))
        .child(label)
}

fn hunk_title(hunk: &Hunk) -> impl IntoElement + use<> {
    div()
        .flex_1()
        .min_w(px(0.))
        .truncate()
        .font_family(FONT_FAMILY)
        .text_size(px(11.))
        .text_color(colours::accent())
        .child(diff::header(hunk))
}

fn copy_hunk(patch_text: &str, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string(patch_text.to_string()));
    toast("Copied the hunk as a patch", ToastKind::Info, cx);
}

fn hunk_actions(data: &DiffData, i: usize, hunk: &Hunk) -> Vec<AnyElement> {
    let (workspace, path) = (data.selected.workspace, data.patch.file.path.clone());
    let changed = diff::changed(hunk);
    let reference = diff::reference(&path, changed);
    let patch_text = diff::patch(hunk);
    let mut actions = Vec::new();
    if data.patch.file.status != FileStatus::Deleted {
        let open = hunk_action("hunk-open", i, "Open").on_click(move |_: &ClickEvent, _, cx| {
            ChangesPanel::open_in_editor(workspace, &path, changed.0, cx);
        });
        actions.push(open.into_any_element());
    }
    let ask = hunk_action("hunk-ask", i, "Ask agent")
        .on_click(move |_: &ClickEvent, _, cx| ChangesPanel::ask_agent(workspace, &reference, cx));
    let copy = hunk_action("hunk-copy", i, "Copy").on_click(move |_: &ClickEvent, _, cx| copy_hunk(&patch_text, cx));
    actions.extend([ask.into_any_element(), copy.into_any_element()]);
    actions
}

fn hunk_header(data: &DiffData, i: usize) -> AnyElement {
    let Some(hunk) = data.patch.hunks.get(i) else { return empty() };
    div()
        .group(SharedString::from(format!("hunk-{i}")))
        .w_full()
        .flex()
        .items_center()
        .gap(px(4.))
        .mt(px(if i == 0 { 0. } else { 6. }))
        .px(px(8.))
        .py(px(3.))
        .bg(Rgba { a: 0.10, ..colours::accent() })
        .border_y_1()
        .border_color(Rgba { a: 0.18, ..colours::accent() })
        .child(hunk_title(hunk))
        .children(hunk_actions(data, i, hunk))
        .into_any_element()
}

fn line_number(n: Option<u32>) -> impl IntoElement {
    div()
        .w(px(34.))
        .flex_none()
        .pr(px(6.))
        .text_right()
        .text_color(colours::faint())
        .child(n.map(|n| n.to_string()).unwrap_or_default())
}

impl CodeLine<'_> {
    fn run_style(&self, look: Option<Look>, emphasis: bool) -> HighlightStyle {
        let mut colour = look.map_or_else(|| colours::text().into(), |l| l.fg);
        if self.dim {
            colour.a *= 0.6;
        }
        HighlightStyle {
            color: Some(colour),
            background_color: self.tint.filter(|_| emphasis).map(|t| Rgba { a: 0.32, ..t }.into()),
            font_weight: look.filter(|l| l.bold).map(|_| FontWeight::BOLD),
            font_style: look.filter(|l| l.italic).map(|_| FontStyle::Italic),
            ..HighlightStyle::default()
        }
    }

    fn highlights(&self) -> Vec<(Range<usize>, HighlightStyle)> {
        diff::runs(self.text, self.syntax, self.emphasis)
            .into_iter()
            .map(|run| (run.range, self.run_style(self.looks.get(run.style).copied(), run.emphasis)))
            .collect()
    }

    fn styled_text(&self) -> StyledText {
        if self.text.is_empty() {
            StyledText::new(SharedString::from(" "))
        } else {
            StyledText::new(SharedString::from(self.text.to_string())).with_highlights(self.highlights())
        }
    }

    fn render(self) -> AnyElement {
        let tint = self.tint;
        div()
            .w_full()
            .flex()
            .font_family(FONT_FAMILY)
            .text_size(px(12.))
            .line_height(px(18.))
            .when_some(tint, |d, t| d.bg(Rgba { a: 0.11, ..t }))
            .when(self.dim, |d| d.bg(colours::sidebar()))
            .child(line_number(self.old))
            .child(line_number(self.new))
            .child(
                div()
                    .w(px(14.))
                    .flex_none()
                    .text_color(tint.map_or(colours::faint(), Into::into))
                    .child(self.sign.to_string()),
            )
            .child(div().flex_1().min_w(px(0.)).pr(px(8.)).text_color(colours::text()).child(self.styled_text()))
            .into_any_element()
    }
}
