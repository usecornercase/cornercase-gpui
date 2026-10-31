use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{AppContext as _, Context, Image, ImageFormat, ScrollStrategy, Task};

use super::super::disk::{self, Content};
use super::super::source::Source;
use super::super::style::image_label;
use super::super::{Spans, highlight};
use super::Peek;
use super::player::Player;
use super::quicklook;
use crate::cli;
use crate::model::Link;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Media {
    Video,
    Audio,
}

pub(super) enum Body {
    Loading,
    Still(Still),
    Video(Player),
    Audio(Player),
}

pub(super) enum Still {
    Code(Code),
    Picture(Picture),
    Note(Note),
}

pub(super) struct Code {
    pub lines: Arc<Vec<String>>,
    pub spans: Option<Spans>,
    pub language: Option<&'static str>,
}

pub(super) struct Picture {
    pub image: Arc<Image>,
    pub size: Option<(f32, f32)>,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Note {
    pub title: &'static str,
    pub detail: String,
}

pub(super) fn media_of(path: &Path) -> Option<Media> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "mp4" | "m4v" | "mov" | "qt" | "3gp" | "3g2" => Some(Media::Video),
        "mp3" | "m4a" | "m4b" | "aac" | "wav" | "aif" | "aiff" | "aifc" | "caf" | "flac" | "amr" => Some(Media::Audio),
        _ => None,
    }
}

fn pixels(n: u32) -> f32 {
    u16::try_from(n).map_or(f32::from(u16::MAX), f32::from)
}

pub(super) fn image_size(bytes: &[u8]) -> Option<(f32, f32)> {
    let reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format().ok()?;
    let (w, h) = reader.into_dimensions().ok()?;
    Some((pixels(w), pixels(h)))
}

fn note(title: &'static str, detail: impl Into<String>) -> Still {
    Still::Note(Note { title, detail: detail.into() })
}

fn picture(image: Arc<Image>, label: String) -> Still {
    let size = image_size(&image.bytes);
    Still::Picture(Picture { image, size, label })
}

fn preview(path: &Path) -> Still {
    match quicklook::thumbnail(path) {
        Some(png) => picture(Arc::new(Image::from_bytes(ImageFormat::Png, png)), "Quick Look preview".into()),
        None => note("No preview", "macOS has no preview for this kind of file."),
    }
}

fn code(path: &Path, lines: Vec<String>) -> Still {
    let language = highlight::language(path, lines.first().map_or("", String::as_str));
    Still::Code(Code { lines: Arc::new(lines), spans: None, language })
}

pub(super) fn described(content: Content, path: &Path, remote: bool) -> Still {
    match content {
        Content::Text(text) => code(path, text.lines),
        Content::Image(image) => {
            let label = image_label(image.format);
            picture(image, label)
        }
        Content::Binary | Content::TooLarge if !remote => preview(path),
        Content::Binary => note("No preview from another machine", "Only text and images are shown from there."),
        Content::TooLarge => {
            note("Too large to preview", format!("Files over {} are not read.", disk::human_size(disk::MAX_BYTES)))
        }
        Content::NotRegular => note("Not a regular file", "Devices, sockets and pipes are not read."),
        Content::Gone(why) => note("Cannot read this file", why),
    }
}

impl Peek {
    pub(super) fn load(&mut self, link: &Link, cx: &mut Context<Self>) {
        let remote = cli::host().is_some();
        let path = PathBuf::from(&link.path);
        match media_of(&path).filter(|_| !remote) {
            Some(media) => self.play(path, media, cx),
            None => self.read(link, remote, cx),
        }
    }

    fn play(&mut self, path: PathBuf, media: Media, cx: &mut Context<Self>) {
        match (Player::open(&path, media == Media::Video), media) {
            (Some(player), Media::Video) => self.body = Some(Body::Video(player)),
            (Some(player), Media::Audio) => self.body = Some(Body::Audio(player)),
            (None, _) => self.preview(path, cx),
        }
    }

    pub(super) fn preview(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.body = Some(Body::Loading);
        let task = cx.background_spawn(async move { preview(&path) });
        self.land(task, cx);
    }

    fn read(&mut self, link: &Link, remote: bool, cx: &mut Context<Self>) {
        let source = if remote { Source::Server(link.workspace.unwrap_or_default()) } else { Source::Disk("/".into()) };
        let path = PathBuf::from(&link.path);
        let task = cx.background_spawn(async move { described(source.read(&path).0, &path, remote) });
        self.land(task, cx);
    }

    fn land(&self, task: Task<Still>, cx: &mut Context<Self>) {
        let generation = self.generation;
        cx.spawn(async move |this, cx| {
            let body = task.await;
            let _ = this.update(cx, |this, cx| this.loaded(generation, body, cx));
        })
        .detach();
    }

    fn loaded(&mut self, generation: u64, still: Still, cx: &mut Context<Self>) {
        if generation != self.generation {
            return;
        }
        if let Still::Code(code) = &still {
            self.highlight(code.lines.clone(), cx);
            let first = self.link.as_ref().and_then(Link::first_line).unwrap_or(0);
            self.code_scroll.scroll_to_item(first.saturating_sub(3), ScrollStrategy::Top);
        }
        self.body = Some(Body::Still(still));
        cx.notify();
    }

    fn highlight(&self, lines: Arc<Vec<String>>, cx: &mut Context<Self>) {
        let (generation, Some(path)) = (self.generation, self.link.as_ref().map(|l| PathBuf::from(&l.path))) else {
            return;
        };
        let task = cx.background_spawn(async move { highlight::highlight(&path, &lines).map(Arc::new) });
        cx.spawn(async move |this, cx| {
            let spans = task.await;
            let _ = this.update(cx, |this, cx| this.highlighted(generation, spans, cx));
        })
        .detach();
    }

    fn highlighted(&mut self, generation: u64, spans: Option<Spans>, cx: &mut Context<Self>) {
        if generation != self.generation {
            return;
        }
        if let Some(Body::Still(Still::Code(code))) = self.body.as_mut() {
            code.spans = spans;
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests;
