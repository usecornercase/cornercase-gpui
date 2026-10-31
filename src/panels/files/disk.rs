use std::io::Read as _;
use std::path::Path;
use std::time::SystemTime;

use gpui::ImageFormat;

pub(super) const MAX_BYTES: u64 = 8 * 1024 * 1024;
pub(super) const MAX_IMAGE_BYTES: u64 = 32 * 1024 * 1024;
const BINARY_PROBE: usize = 8000;

#[derive(Debug, Clone)]
pub(super) enum Content {
    Text(Text),
    Image(std::sync::Arc<gpui::Image>),
    Binary,
    TooLarge,
    NotRegular,
    Gone(String),
}

#[derive(Debug, Clone, Default)]
pub(super) struct Text {
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Stamp {
    pub len: u64,
    pub modified: Option<SystemTime>,
}

pub(super) fn stamp(path: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    Some(Stamp { len: meta.len(), modified: meta.modified().ok() })
}

pub(super) fn sniff_image(head: &[u8]) -> Option<ImageFormat> {
    if head.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(ImageFormat::Png)
    } else if head.starts_with(&[0xff, 0xd8, 0xff]) {
        Some(ImageFormat::Jpeg)
    } else if head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a") {
        Some(ImageFormat::Gif)
    } else if head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"WEBP" {
        Some(ImageFormat::Webp)
    } else if head.starts_with(b"BM") && head.len() >= 14 {
        Some(ImageFormat::Bmp)
    } else if head.starts_with(b"II*\0") || head.starts_with(b"MM\0*") {
        Some(ImageFormat::Tiff)
    } else {
        None
    }
}

pub(super) fn read(path: &Path) -> (Content, Option<Stamp>) {
    let meta = match std::fs::metadata(path) {
        Ok(meta) => meta,
        Err(e) => return (Content::Gone(e.to_string()), None),
    };
    let stamp = Some(Stamp { len: meta.len(), modified: meta.modified().ok() });
    if !meta.is_file() {
        return (Content::NotRegular, stamp);
    }
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) => return (Content::Gone(e.to_string()), stamp),
    };
    let mut bytes = Vec::new();
    if let Err(e) = (&mut file).take(16).read_to_end(&mut bytes) {
        return (Content::Gone(e.to_string()), stamp);
    }
    let cap = if sniff_image(&bytes).is_some() || path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg")) {
        MAX_IMAGE_BYTES
    } else {
        MAX_BYTES
    };
    if meta.len() > cap {
        return (Content::TooLarge, stamp);
    }
    if let Err(e) = (&mut file).take(cap + 1 - bytes.len() as u64).read_to_end(&mut bytes) {
        return (Content::Gone(e.to_string()), stamp);
    }
    (classify(path, bytes), stamp)
}

pub(super) fn classify(path: &Path, bytes: Vec<u8>) -> Content {
    let svg = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg"));
    let format = sniff_image(&bytes[..bytes.len().min(16)]).or(svg.then_some(ImageFormat::Svg));
    if let Some(format) = format {
        if bytes.len() as u64 > MAX_IMAGE_BYTES {
            return Content::TooLarge;
        }
        return Content::Image(std::sync::Arc::new(gpui::Image::from_bytes(format, bytes)));
    }
    if bytes.len() as u64 > MAX_BYTES {
        return Content::TooLarge;
    }
    if bytes[..bytes.len().min(BINARY_PROBE)].contains(&0) {
        return Content::Binary;
    }
    Content::Text(text(&String::from_utf8_lossy(&bytes)))
}

pub(super) fn text(source: &str) -> Text {
    let mut lines: Vec<String> =
        source.split('\n').map(|line| line.strip_suffix('\r').unwrap_or(line).replace('\t', "    ")).collect();
    if lines.len() > 1 && lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    Text { lines }
}

#[expect(clippy::cast_precision_loss, reason = "a size shown to one decimal")]
pub(crate) fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024. && unit < UNITS.len() - 1 {
        value /= 1024.;
        unit += 1;
    }
    if unit == 0 { format!("{bytes} B") } else { format!("{value:.1} {}", UNITS[unit]) }
}

#[cfg(test)]
mod tests;
