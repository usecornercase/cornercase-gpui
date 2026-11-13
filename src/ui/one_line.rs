use std::ops::Range;

use gpui::{
    App, AvailableSpace, Bounds, ContentMask, Element, GlobalElementId, HighlightStyle, InspectorElementId,
    IntoElement, LayoutId, Pixels, ShapedLine, SharedString, Size, Style, TextRun, TextStyle, Window, px,
};

const ELLIPSIS: &str = "…";
const BREAKS: [char; 3] = ['\n', '\r', '\t'];

type OnLaid = Box<dyn FnOnce(LaidOut)>;

pub(crate) struct OneLine {
    text: SharedString,
    highlights: Vec<(Range<usize>, HighlightStyle)>,
    on_laid: Option<OnLaid>,
    widest: Option<Pixels>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Cut {
    kept: usize,
    mark_x: Pixels,
}

pub(crate) struct LaidOut {
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) line: ShapedLine,
    cut: Option<Cut>,
}

impl LaidOut {
    pub(crate) fn index_at(&self, x: Pixels) -> usize {
        let local = x - self.bounds.left();
        caret_in(self.cut, self.line.len(), local, self.line.closest_index_for_x(local))
    }
}

fn caret_in(cut: Option<Cut>, len: usize, x: Pixels, closest: usize) -> usize {
    match cut {
        Some(cut) if x >= cut.mark_x => len,
        Some(cut) => closest.min(cut.kept),
        None => closest,
    }
}

pub(crate) fn one_line(text: impl Into<SharedString>) -> OneLine {
    let text: SharedString = text.into();
    let text = if text.contains(BREAKS) { text.replace(BREAKS, " ").into() } else { text };
    OneLine { text, highlights: Vec::new(), on_laid: None, widest: None }
}

impl OneLine {
    pub(crate) fn highlights(mut self, highlights: impl IntoIterator<Item = (Range<usize>, HighlightStyle)>) -> Self {
        self.highlights = highlights.into_iter().collect();
        self
    }

    pub(crate) fn widest(mut self, widest: Pixels) -> Self {
        self.widest = Some(widest);
        self
    }

    pub(crate) fn on_laid(mut self, report: impl FnOnce(LaidOut) + 'static) -> Self {
        self.on_laid = Some(Box::new(report));
        self
    }

    fn cut(&self, measured: &Measured, width: Pixels, window: &Window) -> (ShapedLine, Cut) {
        let system = window.text_system();
        let mark = system.shape_line(ELLIPSIS.into(), measured.font_size, std::slice::from_ref(&measured.mark), None);
        let at = measured.line.index_for_x(width - mark.width).unwrap_or(self.text.len());
        let kept = kept(&self.text, at);
        let text = SharedString::from(format!("{}{ELLIPSIS}", &self.text[..kept]));
        let cut = Cut { kept, mark_x: measured.line.x_for_index(kept) };
        (system.shape_line(text, measured.font_size, &ending(&measured.runs, kept, measured.mark.clone()), None), cut)
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
        let widest = self.widest;
        let layout = window.request_measured_layout(Style::default(), move |known, available, _, _| Size {
            width: known.width.unwrap_or_else(|| natural(width, available.width, widest)),
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
        let (shown, cut) = if measured.line.width <= bounds.size.width.ceil() {
            (measured.line.clone(), None)
        } else {
            let (shown, cut) = self.cut(measured, bounds.size.width, window);
            (shown, Some(cut))
        };
        if let Some(report) = self.on_laid.take() {
            report(LaidOut { bounds, line: measured.line.clone(), cut });
        }
        shown
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

fn natural(width: Pixels, available: AvailableSpace, widest: Option<Pixels>) -> Pixels {
    let width = widest.map_or(width, |widest| width.min(widest));
    match available {
        AvailableSpace::Definite(room) => width.min(room),
        AvailableSpace::MinContent if widest.is_none() => px(0.),
        AvailableSpace::MinContent | AvailableSpace::MaxContent => width,
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

fn ending(runs: &[TextRun], kept: usize, mut mark: TextRun) -> Vec<TextRun> {
    let mut at = 0;
    let mut ended = Vec::new();
    for run in runs {
        if at < kept {
            ended.push(TextRun { len: run.len.min(kept - at), ..run.clone() });
        }
        at += run.len;
        if at > kept {
            mark.background_color = mark.background_color.or(run.background_color);
        }
    }
    ended.push(mark);
    ended
}

#[cfg(test)]
mod tests;
