//! Reading a file for the viewer: text up to `MAX_BYTES`, images (by their first bytes) up to
//! `MAX_IMAGE_BYTES`, and the rest named for what it is (binary, too large, not a regular file,
//! gone). Only regular files are read: a link to `/dev/zero` or a FIFO would never end.

use std::io::Read as _;
use std::path::Path;
use std::time::SystemTime;

use gpui::ImageFormat;

pub(super) const MAX_BYTES: u64 = 8 * 1024 * 1024;
pub(super) const MAX_IMAGE_BYTES: u64 = 32 * 1024 * 1024;
/// A NUL in this many first bytes makes a file binary.
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
    /// The lines as drawn: tabs as four spaces, no `\r`.
    pub lines: Vec<String>,
}

/// What a re-check compares to see whether the file changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Stamp {
    pub len: u64,
    pub modified: Option<SystemTime>,
}

pub(super) fn stamp(path: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    Some(Stamp { len: meta.len(), modified: meta.modified().ok() })
}

/// The image format of these first bytes, if any.
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

/// Reads a file for the viewer, with what it was read at.
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
    // The first bytes say whether it is an image, which has the larger cap.
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
    // The read stops one byte past the cap, so a file that grew since the size check cannot
    // get past it.
    if let Err(e) = (&mut file).take(cap + 1 - bytes.len() as u64).read_to_end(&mut bytes) {
        return (Content::Gone(e.to_string()), stamp);
    }
    (classify(path, bytes), stamp)
}

/// What these bytes are, for a file at this path.
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

/// The drawn lines of a source.
pub(super) fn text(source: &str) -> Text {
    let mut lines: Vec<String> =
        source.split('\n').map(|line| line.strip_suffix('\r').unwrap_or(line).replace('\t', "    ")).collect();
    // A final newline ends the last line rather than starting an empty one.
    if lines.len() > 1 && lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    Text { lines }
}

/// `1.2 KB`, for the viewer's header.
pub(super) fn human_size(bytes: u64) -> String {
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
mod tests {
    use super::*;

    #[test]
    fn text_lines_drop_cr_and_expand_tabs() {
        assert_eq!(text("a\r\n\tb\n").lines, vec!["a", "    b"]);
        assert_eq!(text("").lines, vec![""]);
        assert_eq!(text("x\n\n").lines, vec!["x", ""]);
    }

    #[test]
    fn classifies_by_content() {
        let p = Path::new("f.txt");
        assert!(matches!(classify(p, b"hello".to_vec()), Content::Text(_)));
        assert!(matches!(classify(p, b"he\0llo".to_vec()), Content::Binary));
        assert!(
            matches!(classify(p, b"\x89PNG\r\n\x1a\nrest".to_vec()), Content::Image(i) if i.format == ImageFormat::Png)
        );
        assert!(
            matches!(classify(Path::new("a.SVG"), b"<svg/>".to_vec()), Content::Image(i) if i.format == ImageFormat::Svg)
        );
        assert!(matches!(classify(p, vec![b'a'; MAX_BYTES as usize + 1]), Content::TooLarge));
        // Invalid UTF-8 is replaced, not refused.
        assert!(matches!(classify(p, vec![0xff, b'a']), Content::Text(_)));
    }

    #[test]
    fn reads_only_regular_files() {
        assert!(matches!(read(Path::new("/dev/zero")).0, Content::NotRegular));
        assert!(matches!(read(Path::new("/definitely/not/here")).0, Content::Gone(_)));
        assert!(matches!(read(Path::new(env!("CARGO_MANIFEST_DIR"))).0, Content::NotRegular));
    }

    #[test]
    fn sizes_read_well() {
        assert_eq!(human_size(12), "12 B");
        assert_eq!(human_size(1536), "1.5 KB");
        assert_eq!(human_size(3 * 1024 * 1024), "3.0 MB");
    }
}
