use std::path::PathBuf;

use anyhow::Result;
use gpui::{Context, Rgba};
use serde_json::{Value, json};

use super::Browser;
use super::agent::toast;
use super::cdp::Event;
use super::element::Element;
use super::shots::Saved;
use crate::store::ToastKind;
use crate::theme::colours;

const PAD: f64 = 8.;
const MOST_SIDE: f64 = 2000.;
const DESCRIBE: &str =
    "function () { const page = window.__cornercase; return page && page.describe ? page.describe(this) : null; }";

pub(crate) enum Crop {
    Capturing,
    Ready(PathBuf),
    Missing,
}

pub(crate) struct Picked {
    pub(crate) element: Element,
    pub(crate) crop: Crop,
}

pub(crate) enum Pick {
    Picking(String),
    Reading(String),
    Picked(Box<Picked>),
}

impl Pick {
    pub(crate) fn inspecting(&self) -> Option<&str> {
        match self {
            Pick::Picking(session) | Pick::Reading(session) => Some(session),
            Pick::Picked(_) => None,
        }
    }
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a colour channel is 0 to 255")]
fn channel(value: f32) -> u8 {
    (value.clamp(0., 1.) * 255.).round() as u8
}

fn rgba(colour: Rgba, alpha: f32) -> Value {
    json!({ "r": channel(colour.r), "g": channel(colour.g), "b": channel(colour.b), "a": alpha })
}

pub(crate) fn highlight() -> Value {
    json!({
        "showInfo": true,
        "showStyles": true,
        "contentColor": rgba(colours::brand(), 0.28),
        "paddingColor": rgba(colours::brand(), 0.14),
        "borderColor": rgba(colours::brand_light(), 0.9),
        "marginColor": rgba(colours::brand(), 0.07),
    })
}

pub(crate) fn crop(element: &Element) -> Option<Value> {
    let (rect, page) = (element.rect, element.page);
    if rect.width < 1. || rect.height < 1. {
        return None;
    }
    let left = (rect.x - PAD).max(-page.x);
    let top = (rect.y - PAD).max(-page.y);
    let width = (rect.x + rect.width + PAD - left).min(MOST_SIDE);
    let height = (rect.y + rect.height + PAD - top).min(MOST_SIDE);
    let inside = left >= 0. && top >= 0. && left + width <= page.width && top + height <= page.height;
    let clip = json!({ "x": left + page.x, "y": top + page.y, "width": width, "height": height, "scale": 1 });
    Some(json!({ "format": "png", "clip": clip, "captureBeyondViewport": !inside }))
}

impl Browser {
    pub(super) fn picking(&self) -> bool {
        self.agents.pick.as_ref().is_some_and(|p| p.inspecting().is_some())
    }

    pub(super) fn toggle_pick(&mut self, cx: &mut Context<Self>) {
        if self.picking() {
            self.cancel_pick(cx);
            return;
        }
        let Some(session) = self.active_session() else { return };
        self.agents.pick = Some(Pick::Picking(session.clone()));
        self.agents.menu = None;
        let s = Some(session.as_str());
        self.send(s, "DOM.enable", json!({}));
        self.send(s, "Overlay.enable", json!({}));
        let params = json!({ "mode": "searchForNode", "highlightConfig": highlight() });
        self.call((s, "Overlay.setInspectMode", params), cx, |this, result, cx| {
            if let Err(e) = result {
                this.agents.pick = None;
                toast(format!("Cannot pick on this page: {e}"), ToastKind::Error, cx);
                cx.notify();
            }
        });
        cx.notify();
    }

    pub(crate) fn cancel_pick(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(pick) = self.agents.pick.take() else { return false };
        if let Some(session) = pick.inspecting() {
            self.end_inspect(session);
        }
        cx.notify();
        true
    }

    fn end_inspect(&self, session: &str) {
        self.send(Some(session), "Overlay.setInspectMode", json!({ "mode": "none", "highlightConfig": {} }));
        self.send(Some(session), "Overlay.hideHighlight", json!({}));
    }

    pub(super) fn overlay_event(&mut self, event: &Event, cx: &mut Context<Self>) {
        let ours =
            event.session.is_some() && event.session.as_deref() == self.agents.pick.as_ref().and_then(Pick::inspecting);
        match event.method.as_str() {
            "Overlay.inspectNodeRequested" if ours => self.inspected(&event.params, cx),
            "Overlay.inspectModeCanceled" if ours => {
                self.agents.pick = None;
                cx.notify();
            }
            _ => {}
        }
    }

    fn inspected(&mut self, params: &Value, cx: &mut Context<Self>) {
        let (Some(node), Some(Pick::Picking(session))) = (params["backendNodeId"].as_i64(), self.agents.pick.take())
        else {
            return;
        };
        self.end_inspect(&session);
        self.agents.pick = Some(Pick::Reading(session.clone()));
        let resolve = json!({ "backendNodeId": node });
        let s = session.clone();
        self.call((Some(&s), "DOM.resolveNode", resolve), cx, move |this, result, cx| {
            this.resolved(session, result, cx);
        });
        cx.notify();
    }

    fn resolved(&mut self, session: String, result: Result<Value>, cx: &mut Context<Self>) {
        let Some(object) = result.ok().and_then(|r| r["object"]["objectId"].as_str().map(str::to_string)) else {
            self.pick_failed(&session, cx);
            return;
        };
        let params = json!({ "objectId": object, "functionDeclaration": DESCRIBE, "returnByValue": true });
        let s = session.clone();
        self.call((Some(&s), "Runtime.callFunctionOn", params), cx, move |this, result, cx| {
            this.described(&session, result, cx);
        });
    }

    fn reading(&self, session: &str) -> bool {
        matches!(&self.agents.pick, Some(Pick::Reading(s)) if s == session)
    }

    fn pick_failed(&mut self, session: &str, cx: &mut Context<Self>) {
        if self.reading(session) {
            self.agents.pick = None;
            toast("Could not read that element", ToastKind::Error, cx);
            cx.notify();
        }
    }

    fn described(&mut self, session: &str, result: Result<Value>, cx: &mut Context<Self>) {
        let element = result.ok().and_then(|r| Element::parse(r["result"]["value"].clone()));
        let Some(element) = element.filter(|_| self.reading(session)) else {
            self.pick_failed(session, cx);
            return;
        };
        let shot = crop(&element);
        let crop = if shot.is_some() { Crop::Capturing } else { Crop::Missing };
        self.agents.pick = Some(Pick::Picked(Box::new(Picked { element, crop })));
        self.ask_for_change(cx);
        if let Some(params) = shot {
            self.capture((session, params), "pick", cx, Self::cropped);
        }
        cx.notify();
    }

    fn cropped(&mut self, saved: Result<Saved>, cx: &mut Context<Self>) {
        if let Some(Pick::Picked(picked)) = &mut self.agents.pick
            && matches!(picked.crop, Crop::Capturing)
        {
            picked.crop = saved.map_or(Crop::Missing, |s| Crop::Ready(s.path));
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests;
