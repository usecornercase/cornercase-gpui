use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, ClipboardItem, FontStyle, FontWeight, HighlightStyle, InteractiveElement as _,
    IntoElement, ListState, MouseButton, ParentElement as _, Rgba, SharedString, StatefulInteractiveElement as _,
    Styled as _, WeakEntity, div, list, prelude::FluentBuilder as _, px,
};

use crate::store::{Store, ToastKind};
use crate::theme::{FONT_FAMILY, colours};
use crate::ui;
use crate::ui::selection::{self, Fit, Laid, Lit, Selecting, Selection, Styles, overlay};

use super::diff::{Part, Spot};
use super::pieces::{plural, toast};
use super::wire::{FileStatus, Hunk, LineKind, Segment};
use super::{ChangesPanel, DiffData, Look, Row, diff};

struct Picked {
    selection: Option<Selection<Spot>>,
    laid: Laid<Spot>,
    fit: Fit,
}

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
    spot: Spot,
    picked: &'a Picked,
}

pub(super) fn diff_list(
    state: &ListState,
    wraps: bool,
    data: Rc<DiffData>,
    panel: WeakEntity<ChangesPanel>,
    selecting: &Selecting<Spot>,
) -> AnyElement {
    let laid = selecting.laid().clone();
    let fit = Fit::of(wraps, laid.body().size.width);
    let picked = Picked { selection: selecting.selection, laid, fit };
    list(state.clone(), move |ix, _window, _cx| diff_row(&data, ix, &panel, &picked)).size_full().into_any_element()
}

fn empty() -> AnyElement {
    div().into_any_element()
}

fn diff_row(data: &DiffData, ix: usize, panel: &WeakEntity<ChangesPanel>, picked: &Picked) -> AnyElement {
    let Some(row) = data.rows.get(ix) else { return empty() };
    match *row {
        Row::Gap { hunk, lines } => gap_button(hunk, lines, panel.clone()),
        Row::GapLine { hunk, line } => gap_line(data, Spot { hunk, part: Part::Gap, line }, picked),
        Row::Line { hunk, line } => patch_line(data, Spot { hunk, part: Part::Patch, line }, picked),
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
        .on_mouse_down(MouseButton::Left, selection::keep_selection)
        .child(format!("⋯ {} unchanged", plural(lines as usize, "line")))
        .on_click(move |_: &ClickEvent, _, cx| {
            let _ = panel.update(cx, |this, cx| this.load_gap(hunk, cx));
        })
        .into_any_element()
}

fn gap_line(data: &DiffData, spot: Spot, picked: &Picked) -> AnyElement {
    let Some((gap, looks)) = data.gaps.get(&spot.hunk) else { return empty() };
    let Some(l) = gap.lines.get(spot.line) else { return empty() };
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
        spot,
        picked,
    }
    .render()
}

fn patch_line(data: &DiffData, spot: Spot, picked: &Picked) -> AnyElement {
    let Some(l) = data.patch.hunks.get(spot.hunk).and_then(|h| h.lines.get(spot.line)) else { return empty() };
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
        spot,
        picked,
    }
    .render()
}

fn hunk_title(hunk: &Hunk) -> impl IntoElement + use<> {
    div()
        .flex_1()
        .min_w(px(0.))
        .font_family(FONT_FAMILY)
        .text_size(px(11.))
        .text_color(colours::accent())
        .child(ui::one_line(diff::header(hunk)))
}

fn copy_hunk(patch_text: &str, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string(patch_text.to_string()));
    toast("Copied the hunk as a patch", ToastKind::Info, cx);
}

fn hunk_actions(data: &DiffData, i: usize, hunk: &Hunk) -> Vec<AnyElement> {
    let (workspace, path) = (data.selected.workspace, data.patch.file.path.clone());
    let changed = diff::changed(hunk);
    let reference = selection::reference(&path, changed);
    let patch_text = diff::patch(hunk);
    let mut actions = Vec::new();
    if data.patch.file.status != FileStatus::Deleted {
        let open = selection::action(("hunk-open", i), "Open").on_click(move |_: &ClickEvent, _, cx| {
            Store::get(cx).read(cx).open_in_editor(workspace, &path, changed.0);
        });
        actions.push(open.into_any_element());
    }
    let ask = selection::action(("hunk-ask", i), "Ask agent").on_click(move |_: &ClickEvent, _, cx| {
        Store::get(cx).update(cx, |s, cx| s.ask_about(workspace, &reference, cx));
    });
    let copy =
        selection::action(("hunk-copy", i), "Copy").on_click(move |_: &ClickEvent, _, cx| copy_hunk(&patch_text, cx));
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

fn line_number(n: Option<u32>, selected: bool) -> impl IntoElement {
    div()
        .w(px(34.))
        .flex_none()
        .pr(px(6.))
        .text_right()
        .text_color(if selected { colours::accent() } else { colours::faint() })
        .child(n.map(|n| n.to_string()).unwrap_or_default())
}

fn line_ground(tint: Option<Rgba>, selected: bool, dim: bool) -> Option<Rgba> {
    match (tint, selected) {
        (Some(tint), true) => Some(Rgba { a: 0.26, ..tint }),
        (Some(tint), false) => Some(Rgba { a: 0.11, ..tint }),
        (None, true) => Some(colours::selected()),
        (None, false) => dim.then(colours::sidebar),
    }
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

    fn highlights(&self, lit: &Lit) -> Styles {
        let styles = diff::runs(self.text, self.syntax, self.emphasis)
            .into_iter()
            .map(|run| (run.range, self.run_style(self.looks.get(run.style).copied(), run.emphasis)))
            .collect();
        match lit.drawn_range(self.text) {
            Some(range) => overlay(styles, &[range], selection::selected_chars()),
            None => styles,
        }
    }

    fn render(self) -> AnyElement {
        let tint = self.tint;
        let lit = self.picked.selection.map_or(Lit::Nothing, |selection| selection.lit(self.spot, self.text));
        let selected = lit == Lit::Line;
        let fit = self.picked.fit;
        let cell = selection::code_cell(&self.picked.laid, (self.spot, self.text), self.highlights(&lit), fit);
        fit.row(div())
            .w_full()
            .flex()
            .font_family(FONT_FAMILY)
            .text_size(px(12.))
            .line_height(px(selection::CODE_ROW))
            .when_some(line_ground(tint, selected, self.dim), gpui::Styled::bg)
            .child(line_number(self.old, selected))
            .child(line_number(self.new, selected))
            .child(
                div()
                    .w(px(14.))
                    .flex_none()
                    .text_color(tint.map_or(colours::faint(), Into::into))
                    .child(self.sign.to_string()),
            )
            .child(cell.text_color(colours::text()))
            .into_any_element()
    }
}
