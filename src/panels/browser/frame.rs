use std::sync::Arc;

use anyhow::{Context as _, Result};
use base64::Engine as _;
use gpui::RenderImage;
use image::Frame;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Meta {
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) offset_top: f32,
    pub(crate) scale: f32,
}

#[derive(Clone)]
pub(crate) struct Shot {
    pub(crate) image: Arc<RenderImage>,
    pub(crate) meta: Meta,
}

#[expect(clippy::cast_possible_truncation, reason = "page sizes in CSS pixels fit an f32")]
fn number(value: &Value, key: &str) -> Option<f32> {
    value.get(key).and_then(Value::as_f64).map(|n| n as f32)
}

pub(crate) fn meta(params: &Value) -> Option<Meta> {
    let m = params.get("metadata")?;
    Some(Meta {
        width: number(m, "deviceWidth")?,
        height: number(m, "deviceHeight")?,
        offset_top: number(m, "offsetTop").unwrap_or(0.),
        scale: number(m, "pageScaleFactor").unwrap_or(1.),
    })
}

pub(crate) fn decode(params: &Value) -> Result<Shot> {
    let data = params.get("data").and_then(Value::as_str).context("a frame without data")?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(data)?;
    let meta = meta(params).context("a frame without metadata")?;
    Ok(Shot { image: Arc::new(bgra(&bytes)?), meta })
}

pub(crate) fn bgra(bytes: &[u8]) -> Result<RenderImage> {
    let mut pixels = image::load_from_memory(bytes)?.into_rgba8();
    for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    Ok(RenderImage::new(vec![Frame::new(pixels)]))
}

#[cfg(test)]
mod tests;
