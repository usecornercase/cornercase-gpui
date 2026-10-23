use std::ops::Range;
use std::path::PathBuf;

use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, HighlightStyle, InteractiveElement as _, IntoElement,
    MouseButton, MouseDownEvent, ParentElement as _, Stateful, StatefulInteractiveElement as _, Styled as _,
    StyledText, div, px, uniform_list,
};

use crate::theme::{FONT_FAMILY, colours};
use crate::ui;

use super::style::{clip, ellipsis, faint_small, file_icon, lit_background, plural, result_row};
use super::{CODE_SIZE, FilesPanel, Results, Target, TextRow, search};

fn names_summary(hits: &[search::NameHit]) -> String {
    match hits.len() {
        0 => "No file names match".to_string(),
        n if n >= search::MAX_NAMES => format!("The best {n} files"),
        1 => "1 file".to_string(),
        n => format!("{n} files"),
    }
}

fn text_summary(text: &search::TextResults) -> String {
    let matches = text.count();
    let files = text.files.len();
    if matches == 0 {
        "No matches".to_string()
    } else if text.capped {
        format!("The first {matches} matches, in {files} files")
    } else {
        format!("{matches} {} in {files} {}", plural(matches, "match", "matches"), plural(files, "file", "files"))
    }
}

fn name_lit() -> HighlightStyle {
    HighlightStyle { color: Some(colours::accent().into()), font_weight: Some(FontWeight::BOLD), ..Default::default() }
}

fn lit_parts(lit: &[Range<usize>], within: Range<usize>) -> Vec<(Range<usize>, HighlightStyle)> {
    clip(lit, within).into_iter().map(|r| (r, name_lit())).collect()
}

fn line_hit_row(ix: usize, hit: &search::LineHit) -> Stateful<Div> {
    let lit_bg = HighlightStyle {
        background_color: Some(lit_background(0.28)),
        color: Some(colours::text().into()),
        ..Default::default()
    };
    let spans: Vec<_> = hit.lit.iter().map(|r| (r.clone(), lit_bg)).collect();
    result_row(("files-hit-line", ix))
        .pl(px(28.))
        .font_family(FONT_FAMILY)
        .text_size(px(CODE_SIZE))
        .child(div().w(px(38.)).flex_none().text_color(colours::faint()).child(hit.line.to_string()))
        .child(
            ellipsis()
                .flex_1()
                .text_color(colours::muted())
                .child(StyledText::new(hit.text.clone()).with_highlights(spans)),
        )
}

fn file_hit_row(ix: usize, file: &search::FileHits) -> Stateful<Div> {
    let name = file.path.rsplit('/').next().unwrap_or(&file.path).to_string();
    result_row(("files-hit-file", ix))
        .mt(px(if ix == 0 { 0. } else { 6. }))
        .child(file_icon(&name, false, false))
        .child(ellipsis().flex_1().font_weight(FontWeight::MEDIUM).child(file.path.clone()))
        .child(ui::dim(file.lines.len().to_string()).flex_none())
}

fn name_hit_row(ix: usize, hit: &search::NameHit) -> Stateful<Div> {
    let split = hit.path.rfind('/').map_or(0, |i| i + 1);
    let (folder, name) = hit.path.split_at(split);
    let name_lit = lit_parts(&hit.lit, split..hit.path.len());
    let folder_lit = lit_parts(&hit.lit, 0..split);
    result_row(("files-name", ix))
        .child(file_icon(name, false, false))
        .child(ellipsis().child(StyledText::new(name.to_string()).with_highlights(name_lit)))
        .child(
            faint_small()
                .flex_1()
                .child(StyledText::new(folder.trim_end_matches('/').to_string()).with_highlights(folder_lit)),
        )
}

impl FilesPanel {
    fn results_summary(&self) -> (usize, String) {
        let (count, summary) = match &self.search.results {
            Results::None => (0, String::new()),
            Results::Names(hits) => (hits.len(), names_summary(hits)),
            Results::Text(text) => (self.search.rows.len(), text_summary(text)),
        };
        if self.search.running.is_some() { (count, "Searching…".to_string()) } else { (count, summary) }
    }

    pub(super) fn render_results(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let (count, summary) = self.results_summary();
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(div().flex_none().px(px(12.)).pt(px(6.)).pb(px(4.)).child(ui::dim(summary)))
            .child(
                div().flex_1().min_h_0().px(px(6.)).pb(px(6.)).child(
                    uniform_list(
                        "files-results",
                        count,
                        cx.processor(|this, range: Range<usize>, _window, cx| this.result_items(range, cx)),
                    )
                    .size_full()
                    .track_scroll(self.results_scroll.clone()),
                ),
            )
            .into_any_element()
    }

    fn result_items(&mut self, range: Range<usize>, cx: &mut Context<Self>) -> Vec<AnyElement> {
        range.filter_map(|ix| self.result_item(ix, cx)).collect()
    }

    fn result_item(&self, ix: usize, cx: &mut Context<Self>) -> Option<AnyElement> {
        let row = match &self.search.results {
            Results::None => return None,
            Results::Names(hits) => {
                let hit = hits.get(ix)?;
                Self::with_menu(name_hit_row(ix, hit), PathBuf::from(&hit.path), cx)
            }
            Results::Text(text) => match self.search.rows.get(ix)? {
                TextRow::File(f) => {
                    let file = text.files.get(*f)?;
                    Self::with_menu(file_hit_row(ix, file), PathBuf::from(&file.path), cx)
                }
                TextRow::Line(f, l) => line_hit_row(ix, text.files.get(*f)?.lines.get(*l)?),
            },
        };
        Some(row.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.open_result(ix, cx))).into_any_element())
    }

    fn with_menu(row: Stateful<Div>, rel: PathBuf, cx: &mut Context<Self>) -> Stateful<Div> {
        let target = Target { rel, dir: false, line: None };
        row.on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, e: &MouseDownEvent, _, cx| this.show_menu(e.position, target.clone(), cx)),
        )
    }
}
