use std::io::Cursor;
use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use super::thumbnailers::Missing;

const PIXELS: u16 = 900;
const WITHIN: Duration = Duration::from_secs(8);

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Pixels {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

pub(super) fn within<T: Send + 'static>(
    limit: Duration,
    work: impl FnOnce() -> Result<T, Missing> + Send + 'static,
) -> Result<T, Missing> {
    let (tx, rx) = mpsc::channel();
    let started = std::thread::Builder::new().name("thumbnail".into()).spawn(move || {
        let _ = tx.send(work());
    });
    started.map_err(|e| Missing::Failed(format!("Windows' thumbnail could not start: {e}")))?;
    match rx.recv_timeout(limit) {
        Ok(answer) => answer,
        Err(RecvTimeoutError::Timeout) => {
            Err(Missing::Failed(format!("Windows made no thumbnail within {} s", limit.as_secs())))
        }
        Err(RecvTimeoutError::Disconnected) => Err(Missing::Failed("Windows' thumbnail stopped".into())),
    }
}

fn straight(colour: u8, alpha: u8) -> u8 {
    let value = (u32::from(colour) * 255 + u32::from(alpha) / 2) / u32::from(alpha);
    u8::try_from(value).unwrap_or(u8::MAX)
}

pub(super) fn rgba_of(bgra: &[u8]) -> Vec<u8> {
    let (pixels, _) = bgra.as_chunks::<4>();
    let opaque = pixels.iter().all(|&[.., a]| a == 0);
    let mut rgba = Vec::with_capacity(bgra.len());
    for &[b, g, r, a] in pixels {
        match a {
            _ if opaque => rgba.extend([r, g, b, u8::MAX]),
            0 => rgba.extend([0, 0, 0, 0]),
            u8::MAX => rgba.extend([r, g, b, a]),
            _ => rgba.extend([straight(r, a), straight(g, a), straight(b, a), a]),
        }
    }
    rgba
}

pub(super) fn png(pixels: &Pixels) -> Result<Vec<u8>, Missing> {
    let image = image::RgbaImage::from_raw(pixels.width, pixels.height, rgba_of(&pixels.bgra))
        .ok_or_else(|| Missing::Failed("Windows gave a thumbnail of the wrong size".into()))?;
    let mut out = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
        .map_err(|e| Missing::Failed(format!("Windows' thumbnail cannot be shown: {e}")))?;
    Ok(out)
}

pub(super) fn thumbnail(path: &Path) -> Result<Vec<u8>, Missing> {
    let path = path.to_path_buf();
    png(&within(WITHIN, move || pixels(&path, PIXELS))?)
}

#[cfg(not(windows))]
fn pixels(_path: &Path, _side: u16) -> Result<Pixels, Missing> {
    Err(Missing::NoReader)
}

#[cfg(windows)]
fn pixels(path: &Path, side: u16) -> Result<Pixels, Missing> {
    use windows::Win32::System::Com::{
        COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
    };

    let started = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
    started.ok().map_err(|e| Missing::Failed(format!("Windows' thumbnails cannot start: {}", e.message())))?;
    let answer = shell_image(path, side);
    unsafe { CoUninitialize() };
    answer
}

#[cfg(windows)]
fn shell_image(path: &Path, side: u16) -> Result<Pixels, Missing> {
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::DeleteObject;
    use windows::Win32::UI::Shell::{
        IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_RESIZETOFIT, SIIGBF_THUMBNAILONLY,
    };
    use windows::core::HSTRING;

    let name = HSTRING::from(path);
    let factory: IShellItemImageFactory = unsafe { SHCreateItemFromParsingName(&name, None) }
        .map_err(|e| Missing::Failed(format!("Windows cannot find it: {}", e.message())))?;
    let wanted = SIZE { cx: i32::from(side), cy: i32::from(side) };
    let bitmap = unsafe { factory.GetImage(wanted, SIIGBF_RESIZETOFIT | SIIGBF_THUMBNAILONLY) }
        .map_err(|_| Missing::NoReader)?;
    let pixels = bitmap_pixels(bitmap);
    let _ = unsafe { DeleteObject(bitmap.into()) };
    pixels
}

#[cfg(windows)]
fn bitmap_pixels(bitmap: windows::Win32::Graphics::Gdi::HBITMAP) -> Result<Pixels, Missing> {
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, GetDIBits,
        GetObjectW,
    };

    let unread = || Missing::Failed("Windows gave a thumbnail that cannot be read".into());
    let mut shape = BITMAP::default();
    let size = i32::try_from(size_of::<BITMAP>()).map_err(|_| unread())?;
    let read = unsafe { GetObjectW(bitmap.into(), size, Some((&raw mut shape).cast())) };
    let (width, height) = (u32::try_from(shape.bmWidth).map_err(|_| unread())?, shape.bmHeight.unsigned_abs());
    if read == 0 || width == 0 || height == 0 {
        return Err(unread());
    }
    let header = BITMAPINFOHEADER {
        biSize: u32::try_from(size_of::<BITMAPINFOHEADER>()).map_err(|_| unread())?,
        biWidth: shape.bmWidth,
        biHeight: -shape.bmHeight.abs(),
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..BITMAPINFOHEADER::default()
    };
    let mut info = BITMAPINFO { bmiHeader: header, ..BITMAPINFO::default() };
    let mut bgra = vec![0u8; usize::try_from(u64::from(width) * u64::from(height) * 4).map_err(|_| unread())?];
    let dc = unsafe { CreateCompatibleDC(None) };
    let lines =
        unsafe { GetDIBits(dc, bitmap, 0, height, Some(bgra.as_mut_ptr().cast()), &raw mut info, DIB_RGB_COLORS) };
    let _ = unsafe { DeleteDC(dc) };
    if u32::try_from(lines).ok() != Some(height) {
        return Err(unread());
    }
    Ok(Pixels { width, height, bgra })
}

#[cfg(test)]
mod tests;
