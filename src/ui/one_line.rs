use std::ops::Range;

use gpui::{
    App, AvailableSpace, Bounds, ContentMask, Element, GlobalElementId, HighlightStyle, InspectorElementId,
    IntoElement, LayoutId, Pixels, ShapedLine, SharedString, Size, Style, TextRun, TextStyle, Window, px,
};

const ELLIPSIS: &str = "…";
const BREAKS: [char; 3] = ['\n', '\r', '\t'];

pub(crate) struct OneLine {
    text: SharedString,
    highlights: Vec<(Range<usize>, HighlightStyle)>,
}

pub(crate) fn one_line(text: impl Into<SharedString>) -> OneLine {
    let text: SharedString = text.into();
    let text = if text.contains(BREAKS) { text.replace(BREAKS, " ").into() } else { text };
    OneLine { text, highlights: Vec::new() }
}

impl OneLine {
    pub(crate) fn highlights(mut self, highlights: impl IntoIterator<Item = (Range<usize>, HighlightStyle)>) -> Self {
        self.highlights = highlights.into_iter().collect();
        self
    }

    fn cut(&self, measured: &Measured, width: Pixels, window: &Window) -> ShapedLine {
        let system = window.text_system();
        let mark = system.shape_line(ELLIPSIS.into(), measured.font_size, std::slice::from_ref(&measured.mark), None);
        let at = measured.line.index_for_x(width - mark.width).unwrap_or(self.text.len());
        let kept = kept(&self.text, at);
        let text = SharedString::from(format!("{}{ELLIPSIS}", &self.text[..kept]));
        system.shape_line(text, measured.font_size, &ending(&measured.runs, kept, measured.mark.clone()), None)
    }
}

pub(crate) struct Measured {
    line: ShapedLine,
    runs: Vec<TextRun>,
    mark: TextRun,
    font_size: Pixels,
    line_height: Pixels,
}

impl IntoElement for OneLine {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for OneLine {
    type RequestLayoutState = Measured;
    type PrepaintState = ShapedLine;

    super::anonymous_element!();

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        _cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = style.line_height.to_pixels(font_size.into(), window.rem_size());
        let runs = runs(&self.text, &style, &self.highlights);
        let line = window.text_system().shape_line(self.text.clone(), font_size, &runs, None);
        let width = line.width.ceil();
        let layout = window.request_measured_layout(Style::default(), move |known, available, _, _| Size {
            width: known.width.unwrap_or_else(|| natural(width, available.width)),
            height: known.height.unwrap_or(line_height),
        });
        let mark = style.to_run(ELLIPSIS.len());
        (layout, Measured { line, runs, mark, font_size, line_height })
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        measured: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        if measured.line.width <= bounds.size.width.ceil() {
            return measured.line.clone();
        }
        self.cut(measured, bounds.size.width, window)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        measured: &mut Self::RequestLayoutState,
        line: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            let _ = line.paint_background(bounds.origin, measured.line_height, window, cx);
            let _ = line.paint(bounds.origin, measured.line_height, window, cx);
        });
    }
}

fn natural(width: Pixels, available: AvailableSpace) -> Pixels {
    match available {
        AvailableSpace::Definite(room) => width.min(room),
        AvailableSpace::MinContent => px(0.),
        AvailableSpace::MaxContent => width,
    }
}

fn runs(text: &str, style: &TextStyle, highlights: &[(Range<usize>, HighlightStyle)]) -> Vec<TextRun> {
    let mut runs = Vec::new();
    let mut at = 0;
    for (range, highlight) in highlights {
        let range = range.start.max(at)..range.end.min(text.len());
        if range.is_empty() {
            continue;
        }
        if at < range.start {
            runs.push(style.to_run(range.start - at));
        }
        runs.push(style.clone().highlight(*highlight).to_run(range.len()));
        at = range.end;
    }
    if at < text.len() {
        runs.push(style.to_run(text.len() - at));
    }
    runs
}

fn kept(text: &str, at: usize) -> usize {
    let at = (0..=at.min(text.len())).rev().find(|ix| text.is_char_boundary(*ix)).unwrap_or(0);
    text[..at].trim_end().len()
}

fn ending(runs: &[TextRun], kept: usize, mark: TextRun) -> Vec<TextRun> {
    let mut left = kept;
    let mut ended = Vec::new();
    for run in runs {
        if left == 0 {
            break;
        }
        let len = run.len.min(left);
        ended.push(TextRun { len, ..run.clone() });
        left -= len;
    }
    ended.push(mark);
    ended
}

#[cfg(test)]
mod tests;
