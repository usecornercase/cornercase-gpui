use std::path::Path;

use core_foundation::base::TCFType as _;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_video::pixel_buffer::{CVPixelBuffer, CVPixelBufferKeys, kCVPixelFormatType_420YpCbCr8BiPlanarFullRange};
use objc2::encode::{Encode, Encoding, RefEncode};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{class, msg_send};

#[repr(C)]
#[derive(Clone, Copy)]
struct CMTime {
    value: i64,
    timescale: i32,
    flags: u32,
    epoch: i64,
}

unsafe impl Encode for CMTime {
    const ENCODING: Encoding = Encoding::Struct("?", &[i64::ENCODING, i32::ENCODING, u32::ENCODING, i64::ENCODING]);
}

unsafe impl RefEncode for CMTime {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGSize {
    width: f64,
    height: f64,
}

unsafe impl Encode for CGSize {
    const ENCODING: Encoding = Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING]);
}

#[repr(C)]
struct CVBuffer {
    _opaque: [u8; 0],
}

unsafe impl RefEncode for CVBuffer {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("__CVBuffer", &[]));
}

#[link(name = "AVFoundation", kind = "framework")]
unsafe extern "C" {}

#[link(name = "CoreMedia", kind = "framework")]
unsafe extern "C" {
    fn CMTimeGetSeconds(time: CMTime) -> f64;
    static kCMTimeZero: CMTime;
}

const READY: isize = 1;
const FAILED: isize = 2;
const KEEP_PLAYING: isize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum State {
    Loading,
    Ready,
    Failed,
}

pub(super) struct Player {
    av: Retained<AnyObject>,
    item: Retained<AnyObject>,
    output: Option<Retained<AnyObject>>,
    frame: Option<CVPixelBuffer>,
    playing: bool,
    muted: bool,
    looping: bool,
}

fn seconds(time: CMTime) -> Option<f64> {
    let seconds = unsafe { CMTimeGetSeconds(time) };
    seconds.is_finite().then_some(seconds)
}

fn file_url(path: &Path) -> Option<Retained<AnyObject>> {
    let path = CFString::new(path.to_str()?);
    let path = path.as_concrete_TypeRef().cast::<AnyObject>();
    unsafe { msg_send![class!(NSURL), fileURLWithPath: path] }
}

fn video_output() -> Option<Retained<AnyObject>> {
    let format = CFNumber::from(i64::from(kCVPixelFormatType_420YpCbCr8BiPlanarFullRange));
    let format_key = CFString::from(CVPixelBufferKeys::PixelFormatType);
    let metal_key = CFString::from(CVPixelBufferKeys::MetalCompatibility);
    let settings = CFDictionary::from_CFType_pairs(&[
        (format_key.as_CFType(), format.as_CFType()),
        (metal_key.as_CFType(), CFBoolean::true_value().as_CFType()),
    ]);
    let settings = settings.as_concrete_TypeRef().cast::<AnyObject>();
    unsafe { msg_send![msg_send![class!(AVPlayerItemVideoOutput), alloc], initWithPixelBufferAttributes: settings] }
}

impl Player {
    pub(super) fn open(path: &Path, video: bool) -> Option<Self> {
        let url = file_url(path)?;
        let item: Option<Retained<AnyObject>> = unsafe { msg_send![class!(AVPlayerItem), playerItemWithURL: &*url] };
        let item = item?;
        let output = if video { Some(video_output()?) } else { None };
        if let Some(output) = &output {
            let () = unsafe { msg_send![&*item, addOutput: &**output] };
        }
        let player: Option<Retained<AnyObject>> = unsafe { msg_send![class!(AVPlayer), playerWithPlayerItem: &*item] };
        let mut this = Self { av: player?, item, output, frame: None, playing: false, muted: video, looping: video };
        let () = unsafe { msg_send![&*this.av, setActionAtItemEnd: KEEP_PLAYING] };
        this.set_muted(video);
        if video {
            this.play();
        }
        Some(this)
    }

    pub(super) fn state(&self) -> State {
        let status: isize = unsafe { msg_send![&*self.item, status] };
        match status {
            READY => State::Ready,
            FAILED => State::Failed,
            _ => State::Loading,
        }
    }

    #[expect(clippy::cast_possible_truncation, reason = "a video's size in pixels")]
    pub(super) fn size(&self) -> Option<(f32, f32)> {
        let size: CGSize = unsafe { msg_send![&*self.item, presentationSize] };
        (size.width > 0. && size.height > 0.).then_some((size.width as f32, size.height as f32))
    }

    pub(super) fn duration(&self) -> Option<f64> {
        seconds(unsafe { msg_send![&*self.item, duration] }).filter(|d| *d > 0.)
    }

    pub(super) fn position(&self) -> f64 {
        seconds(unsafe { msg_send![&*self.item, currentTime] }).unwrap_or(0.)
    }

    pub(super) fn frame(&self) -> Option<CVPixelBuffer> {
        self.frame.clone()
    }

    pub(super) fn playing(&self) -> bool {
        self.playing
    }

    pub(super) fn muted(&self) -> bool {
        self.muted
    }

    pub(super) fn tick(&mut self) {
        self.wrap_at_end();
        self.pull_frame();
    }

    fn wrap_at_end(&mut self) {
        let Some(duration) = self.duration() else { return };
        if !self.playing || self.position() < duration {
            return;
        }
        let () = unsafe { msg_send![&*self.av, seekToTime: kCMTimeZero] };
        if !self.looping {
            self.pause();
        }
    }

    fn pull_frame(&mut self) {
        let Some(output) = &self.output else { return };
        let time: CMTime = unsafe { msg_send![&*self.item, currentTime] };
        let fresh: Bool = unsafe { msg_send![&**output, hasNewPixelBufferForItemTime: time] };
        if !fresh.as_bool() {
            return;
        }
        let none = std::ptr::null_mut::<CMTime>();
        let raw: *mut CVBuffer =
            unsafe { msg_send![&**output, copyPixelBufferForItemTime: time, itemTimeForDisplay: none] };
        if raw.is_null() {
            return;
        }
        let frame = unsafe { CVPixelBuffer::wrap_under_create_rule(raw.cast()) };
        if frame.get_pixel_format() == kCVPixelFormatType_420YpCbCr8BiPlanarFullRange {
            self.frame = Some(frame);
        }
    }

    pub(super) fn play(&mut self) {
        let () = unsafe { msg_send![&*self.av, play] };
        self.playing = true;
    }

    pub(super) fn pause(&mut self) {
        let () = unsafe { msg_send![&*self.av, pause] };
        self.playing = false;
    }

    pub(super) fn toggle(&mut self) {
        if self.playing { self.pause() } else { self.play() }
    }

    pub(super) fn set_muted(&mut self, muted: bool) {
        let () = unsafe { msg_send![&*self.av, setMuted: Bool::new(muted)] };
        self.muted = muted;
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.pause();
    }
}
