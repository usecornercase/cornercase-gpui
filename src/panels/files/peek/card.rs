use std::ops::Range;
use std::path::PathBuf;

use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, InteractiveElement as _, IntoElement, ObjectFit,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, StyledImage as _, Window, div, img,
    prelude::FluentBuilder as _, px, surface, uniform_list,
};

use super::super::style::{CodeRow, code_row, file_icon, number_width, small_f32, span_styles};
use super::body::{Body, Code, Note, Picture, Still};
use super::place::{NARROW, Place, Side, fit, place, shown_rows};
use super::player::{Player, State};
use super::{Peek, Subject};
use crate::model::Link;
use crate::notify::tip;
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, motion};

const HEADER: f32 = 40.;
const ROW: f32 = 18.;
const ROWS: usize = 18;
const INSET: f32 = 12.;
const MEDIA_HEIGHT: f32 = 440.;
const UNKNOWN_SIZE: (f32, f32) = (640., 360.);

pub(super) fn lines_label(link: &Link) -> Option<String> {
    let lines = link.lines?;
    Some(if lines.last > lines.first {
        format!("lines {}–{}", lines.first, lines.last)
    } else {
        format!("line {}", lines.first)
    })
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "whole seconds, clamped first")]
pub(super) fn clock(seconds: f64) -> String {
    let whole = if seconds.is_finite() { seconds.clamp(0., 1e9) as u64 } else { 0 };
    format!("{}:{:02}", whole / 60, whole % 60)
}

pub(super) fn folder_of(shown: &str) -> String {
    shown.rsplit_once('/').map(|(folder, _)| folder.to_string()).unwrap_or_default()
}

fn media_box(size: (f32, f32), place: &Place) -> (f32, f32) {
    let room = (place.width - 2. * INSET, (place.height - HEADER).min(MEDIA_HEIGHT) - 2. * INSET);
    fit(size, room)
}

fn framed(element: impl IntoElement, width: f32) -> (AnyElement, f32) {
    let body = div().p(px(INSET)).flex().justify_center().bg(colours::pane()).child(element);
    (body.into_any_element(), (width + 2. * INSET).max(NARROW))
}

pub(super) fn header_row(icon: impl IntoElement, (name, rest): (String, String)) -> Div {
    div()
        .h(px(HEADER))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(8.))
        .pl(px(12.))
        .pr(px(6.))
        .border_b_1()
        .border_color(colours::border())
        .child(icon)
        .child(div().flex_none().flex().max_w(px(260.)).font_weight(FontWeight::SEMIBOLD).child(ui::one_line(name)))
        .child(
            div().flex_1().min_w_0().flex().text_size(px(11.)).text_color(colours::faint()).child(ui::one_line(rest)),
        )
}

fn positioned(place: &Place, card: impl IntoElement) -> Div {
    div()
        .absolute()
        .left(px(place.left))
        .when_some(place.top, |d, top| d.top(px(top)))
        .when_some(place.bottom, |d, bottom| d.bottom(px(bottom)))
        .child(card)
}

fn note_body(note: &Note) -> (AnyElement, f32) {
    let body = div()
        .p(px(16.))
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(div().text_color(colours::soft()).child(note.title))
        .child(div().text_size(px(12.)).text_color(colours::faint()).child(note.detail.clone()));
    (body.into_any_element(), NARROW)
}

pub(super) fn picture_body(picture: &Picture, place: &Place) -> (AnyElement, f32) {
    let (w, h) = media_box(picture.size.unwrap_or(UNKNOWN_SIZE), place);
    let image = img(picture.image.clone())
        .w(px(w))
        .h(px(h))
        .object_fit(ObjectFit::Contain)
        .rounded(px(radius::CHIP))
        .id("peek-picture");
    framed(image, w)
}

fn play_badge(playing: bool) -> Option<Div> {
    (!playing).then(|| {
        div().absolute().inset_0().flex().items_center().justify_center().child(
            div()
                .size(px(44.))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(colours::veil())
                .child(ui::icon(Icon::Play, 18.).text_color(colours::text())),
        )
    })
}

fn info(link: &Link, body: Option<&Body>) -> String {
    let detail = match body {
        Some(Body::Still(Still::Code(code))) => Some(code_info(code)),
        Some(Body::Still(Still::Picture(picture))) => Some(picture.label.clone()),
        Some(Body::Video(player) | Body::Audio(player)) => player.duration().map(clock),
        _ => None,
    };
    lines_label(link).into_iter().chain(detail).collect::<Vec<_>>().join("  ·  ")
}

fn code_info(code: &Code) -> String {
    let n = code.lines.len();
    let count = format!("{n} {}", if n == 1 { "line" } else { "lines" });
    code.language.map_or(count.clone(), |language| format!("{count}  ·  {language}"))
}

impl Peek {
    fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Body::Video(player) | Body::Audio(player)) = self.body.as_mut() else { return };
        player.tick();
        let state = player.state();
        if player.playing() || state == State::Loading {
            window.request_animation_frame();
        }
        if state == State::Failed
            && let Some(path) = self.link().map(|link| PathBuf::from(&link.path))
        {
            self.preview(path, cx);
        }
    }

    fn body_element(&self, place: &Place, cx: &mut Context<Self>) -> (AnyElement, f32) {
        match self.body.as_ref() {
            Some(Body::Still(Still::Code(code))) => self.code_body(code, place, cx),
            Some(Body::Still(Still::Picture(picture))) if self.page().is_some() => Self::page_shot(picture, place, cx),
            Some(Body::Still(Still::Picture(picture))) => picture_body(picture, place),
            Some(Body::Still(Still::Note(note))) => note_body(note),
            Some(Body::Video(player)) => Self::video_body(player, place, cx),
            Some(Body::Audio(player)) => Self::audio_body(player, cx),
            Some(Body::Ask) => Self::ask_body(cx),
            Some(Body::Loading) | None => {
                let words = if self.link().is_some() { "Reading…" } else { "Loading the page…" };
                (div().p(px(16.)).child(ui::busy("peek-loading", words)).into_any_element(), NARROW)
            }
        }
    }

    fn code_body(&self, code: &Code, place: &Place, cx: &mut Context<Self>) -> (AnyElement, f32) {
        let rows = shown_rows(code.lines.len(), place.height - HEADER - 8., ROW, ROWS);
        let list = uniform_list(
            "peek-code",
            code.lines.len(),
            cx.processor(|this, range: Range<usize>, _window, cx| this.code_rows(range, cx)),
        )
        .h(px(small_f32(rows) * ROW))
        .track_scroll(self.code_scroll.clone());
        (div().py(px(4.)).bg(colours::pane()).child(list).into_any_element(), place.width)
    }

    fn code_rows(&mut self, range: Range<usize>, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(Body::Still(Still::Code(code))) = &self.body else { return Vec::new() };
        let lit = self.link().and_then(|link| Some(link.first_line()?..=link.last_line()?));
        let width = number_width(code.lines.len());
        range
            .filter_map(|ix| code.lines.get(ix).map(|line| (ix, line)))
            .map(|(ix, line)| {
                let selected = lit.as_ref().is_some_and(|lit| lit.contains(&ix));
                let row = CodeRow::new(ix, line, span_styles(code.spans.as_ref(), ix), selected, width);
                code_row(row)
                    .id(("peek-line", ix))
                    .cursor_pointer()
                    .hover(|s| s.bg(colours::panel()))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.open_in_files(Some(ix), cx)))
                    .into_any_element()
            })
            .collect()
    }

    fn video_body(player: &Player, place: &Place, cx: &mut Context<Self>) -> (AnyElement, f32) {
        let (w, h) = media_box(player.size().unwrap_or(UNKNOWN_SIZE), place);
        let picture = match player.frame() {
            Some(frame) => surface(frame).size_full().object_fit(ObjectFit::Contain).into_any_element(),
            None => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(ui::busy("peek-video", "Opening…"))
                .into_any_element(),
        };
        let video = div()
            .id("peek-video")
            .relative()
            .w(px(w))
            .h(px(h))
            .rounded(px(radius::CHIP))
            .overflow_hidden()
            .bg(colours::pane())
            .cursor_pointer()
            .child(picture)
            .children(play_badge(player.playing()))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_playing(cx)));
        framed(video, w)
    }

    fn audio_body(player: &Player, cx: &mut Context<Self>) -> (AnyElement, f32) {
        let duration = player.duration();
        let position = player.position();
        #[expect(clippy::cast_possible_truncation, reason = "a fraction from 0 to 1")]
        let fraction = duration.map_or(0., |d| (position / d).clamp(0., 1.)) as f32;
        let glyph = if player.playing() { Icon::Pause } else { Icon::Play };
        let time = format!("{} / {}", clock(position), duration.map_or_else(|| "–:––".into(), clock));
        let body = div()
            .p(px(14.))
            .flex()
            .items_center()
            .gap(px(12.))
            .bg(colours::pane())
            .child(
                ui::tool_button("peek-audio-play", glyph, 32., player.playing())
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_playing(cx))),
            )
            .child(div().flex_1().child(ui::bar(fraction, colours::accent(), 4.)))
            .child(ui::dim(time));
        (body.into_any_element(), NARROW + 40.)
    }

    fn toggle_playing(&mut self, cx: &mut Context<Self>) {
        if let Some(Body::Video(player) | Body::Audio(player)) = self.body.as_mut() {
            player.toggle();
            cx.notify();
        }
    }

    fn toggle_muted(&mut self, cx: &mut Context<Self>) {
        if let Some(Body::Video(player)) = self.body.as_mut() {
            let muted = !player.muted();
            player.set_muted(muted);
            cx.notify();
        }
    }

    fn header(&self, link: &Link, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let names = (link.name().to_string(), folder_of(link.shown_path()));
        header_row(file_icon(link.name(), false, false), names)
            .child(
                div()
                    .flex_none()
                    .flex()
                    .text_size(px(11.))
                    .text_color(colours::muted())
                    .child(ui::one_line(info(link, self.body.as_ref()))),
            )
            .child(self.actions(cx))
    }

    fn actions(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let muted = match &self.body {
            Some(Body::Video(player)) => Some(player.muted()),
            _ => None,
        };
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(2.))
            .when_some(muted, |d, muted| {
                let (glyph, hint) = if muted { (Icon::Muted, "Turn the sound on") } else { (Icon::Sound, "Mute") };
                d.child(
                    ui::tool_button("peek-sound", glyph, 26., false)
                        .tooltip(tip(hint))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_muted(cx))),
                )
            })
            .child(
                ui::tool_button("peek-open-files", Icon::Files, 26., false)
                    .tooltip(tip("Open in the Files panel"))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.open_in_files(None, cx))),
            )
            .when(crate::cli::host().is_none(), |d| {
                d.child(
                    ui::tool_button("peek-open-app", Icon::External, 26., false)
                        .tooltip(tip("Open in its default app"))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.open_in_app(cx))),
                )
            })
    }

    fn card(
        &self,
        header: AnyElement,
        (body, width): (AnyElement, f32),
        place: &Place,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let card = ui::glass::card(radius::SHEET)
            .bg(colours::raised())
            .id("peek")
            .occlude()
            .w(px(width.min(place.width)))
            .max_h(px(place.height))
            .flex()
            .flex_col()
            .overflow_hidden()
            .text_size(px(12.))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| this.card_hovered(*hovered, cx)))
            .child(header)
            .child(body);
        let from = match place.side {
            Side::Beside => (-8., 0.),
            Side::Below => (0., -6.),
            Side::Above => (0., 6.),
        };
        motion::slide(card, ("peek-in", self.generation), motion::Way::In, from)
    }
}

impl Render for Peek {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(subject) = self.subject.clone().filter(|_| self.body.is_some()) else {
            return div().into_any_element();
        };
        self.tick(window, cx);
        let place = place(self.anchor.link, self.anchor.room);
        let body = self.body_element(&place, cx);
        let header = match &subject {
            Subject::File(link) => self.header(link, cx).into_any_element(),
            Subject::Web(page) => self.web_header(page, cx).into_any_element(),
        };
        positioned(&place, self.card(header, body, &place, cx)).into_any_element()
    }
}

#[cfg(test)]
mod tests;
