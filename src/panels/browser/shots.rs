use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result};
use base64::Engine as _;
use gpui::{AppContext as _, Context};
use serde_json::{Value, json};

use super::Browser;
use super::agent::toast;
use crate::store::{Busy, Store, ToastKind};

const KEEP: Duration = Duration::from_hours(24);
const MOST_CSS_HEIGHT: f64 = 20000.;
const MOST_DEVICE_SIDE: f64 = 16000.;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shot {
    Visible,
    Full,
}

impl Shot {
    fn what(self) -> &'static str {
        match self {
            Shot::Visible => "visible area",
            Shot::Full => "full page",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Saved {
    pub(crate) path: PathBuf,
    pub(crate) size: (u32, u32),
}

struct Order {
    shot: Shot,
    session: String,
    url: String,
    pane: u64,
}

pub(crate) fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let header = bytes.get(..24)?;
    if &header[..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let word = |at: usize| header.get(at..at + 4).and_then(|b| b.try_into().ok()).map(u32::from_be_bytes);
    Some((word(16)?, word(20)?))
}

pub(crate) fn caption(shot: Shot, url: &str, (width, height): (u32, u32)) -> String {
    format!("Screenshot of {url} ({}, {width}×{height} px) ", shot.what())
}

pub(crate) fn full_page(metrics: &Value, scale: f64) -> Option<Value> {
    let size = metrics.get("cssContentSize").or_else(|| metrics.get("contentSize"))?;
    let width = size["width"].as_f64()?.max(1.);
    let height = size["height"].as_f64()?.clamp(1., MOST_CSS_HEIGHT);
    let device = width.max(height) * scale.max(1.);
    let zoom = (MOST_DEVICE_SIDE / device).min(1.);
    let clip = json!({ "x": 0, "y": 0, "width": width, "height": height, "scale": zoom });
    Some(json!({ "format": "png", "clip": clip, "captureBeyondViewport": true }))
}

pub(crate) fn shots_dir() -> PathBuf {
    std::env::temp_dir().join("cornercase-gpui")
}

fn sweep(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let old = |at: SystemTime| at.elapsed().is_ok_and(|age| age > KEEP);
    for entry in entries.flatten() {
        let stale = entry.metadata().and_then(|m| m.modified()).is_ok_and(old);
        if stale && entry.path().extension().is_some_and(|e| e == "png") {
            let _ = fs::remove_file(entry.path());
        }
    }
}

fn save(data: &str, prefix: &str) -> Result<Saved> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(data)?;
    let size = png_size(&bytes).context("the browser sent no PNG")?;
    let dir = shots_dir();
    fs::create_dir_all(&dir)?;
    sweep(&dir);
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or_default();
    let path = dir.join(format!("{prefix}-{stamp}.png"));
    fs::write(&path, &bytes)?;
    Ok(Saved { path, size })
}

impl Browser {
    pub(super) fn capture(
        &self,
        (session, params): (&str, Value),
        prefix: &'static str,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, Result<Saved>, &mut Context<Self>) + 'static,
    ) {
        let beyond = params["captureBeyondViewport"].as_bool() == Some(true);
        let tab = session.to_string();
        self.call((Some(session), "Page.captureScreenshot", params), cx, move |this, shot, cx| {
            if beyond {
                this.refresh_cast(&tab);
            }
            let data = shot.and_then(|r| r["data"].as_str().map(str::to_string).context("the browser sent no image"));
            let saving = cx.background_spawn(async move { save(&data?, prefix) });
            cx.spawn(async move |this, cx| {
                let saved = saving.await;
                let _ = this.update(cx, |this, cx| then(this, saved, cx));
            })
            .detach();
        });
    }

    fn refresh_cast(&self, session: &str) {
        self.size_page(session);
        let casting = self.casting.as_deref().and_then(|t| self.tabs.get(t)).and_then(|t| t.session.as_deref());
        if casting == Some(session) {
            self.send(Some(session), "Page.stopScreencast", json!({}));
            self.start_cast(session);
        }
    }

    pub(super) fn shoot(&mut self, shot: Shot, cx: &mut Context<Self>) {
        self.agents.menu = None;
        let (Some(session), Some(url)) = (self.active_session(), self.active_url()) else { return };
        let Some(pane) = self.target_agent(cx) else {
            toast("No agent in this workspace to give the screenshot to", ToastKind::Info, cx);
            return;
        };
        let busy = Busy::new("browser-shot", "Taking a screenshot…");
        self.agents.shooting = Some(Store::get(cx).update(cx, |s, cx| s.start_job(busy, cx)));
        let order = Order { shot, session, url, pane };
        match shot {
            Shot::Visible => self.take(order, json!({ "format": "png" }), cx),
            Shot::Full => self.measure_page(order, cx),
        }
        cx.notify();
    }

    fn measure_page(&mut self, order: Order, cx: &mut Context<Self>) {
        let scale = f64::from(self.view.scale);
        let session = order.session.clone();
        self.call((Some(&session), "Page.getLayoutMetrics", json!({})), cx, move |this, metrics, cx| {
            match metrics.ok().and_then(|m| full_page(&m, scale)) {
                Some(params) => this.take(order, params, cx),
                None => this.shot_taken(&order, Err(anyhow::anyhow!("the page's size is unknown")), cx),
            }
        });
    }

    fn take(&mut self, order: Order, params: Value, cx: &mut Context<Self>) {
        let session = order.session.clone();
        self.capture((&session, params), "shot", cx, move |this, saved, cx| this.shot_taken(&order, saved, cx));
    }

    fn shot_taken(&mut self, order: &Order, saved: Result<Saved>, cx: &mut Context<Self>) {
        if let Some(job) = self.agents.shooting.take() {
            Store::get(cx).update(cx, |s, cx| s.end_job(job, cx));
        }
        match saved {
            Ok(saved) => {
                let text = caption(order.shot, &order.url, saved.size);
                if Self::give_to_agent(order.pane, &text, &[saved.path.as_path()], cx) {
                    let name = Self::agent_name(order.pane, cx);
                    toast(format!("Screenshot handed to {name}"), ToastKind::Info, cx);
                }
            }
            Err(e) => toast(format!("Could not take the screenshot: {e:#}"), ToastKind::Error, cx),
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests;
