use gpui::Context;
use serde_json::{Value, json};

use super::Browser;
use super::viewport::View;
use crate::ui::Icon;

const UNKNOWN_SCALE: f32 = 2.;
const TOUCH_POINTS: u8 = 5;
const IOS: &str = "18_0 like Mac OS X";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Form {
    Phone,
    Tablet,
    Laptop,
    Desktop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum System {
    Ios,
    Ipados,
    Android,
    Desktop,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Preset {
    pub(crate) name: &'static str,
    width: u16,
    height: u16,
    scale: f32,
    form: Form,
    system: System,
}

const fn preset(name: &'static str, (width, height): (u16, u16), scale: f32, (form, system): (Form, System)) -> Preset {
    Preset { name, width, height, scale, form, system }
}

pub(crate) const PRESETS: [Preset; 5] = [
    preset("iPhone 15", (393, 852), 3., (Form::Phone, System::Ios)),
    preset("Pixel 8", (412, 915), 2.625, (Form::Phone, System::Android)),
    preset("iPad Air", (820, 1180), 2., (Form::Tablet, System::Ipados)),
    preset("Laptop", (1280, 800), 2., (Form::Laptop, System::Desktop)),
    preset("Desktop", (1440, 900), 2., (Form::Desktop, System::Desktop)),
];

#[expect(clippy::cast_possible_truncation, reason = "a viewport is a few thousand pixels at most")]
fn whole(n: f32) -> i64 {
    n.round() as i64
}

impl Preset {
    pub(crate) fn icon(&self) -> Icon {
        match self.form {
            Form::Phone => Icon::Phone,
            Form::Tablet => Icon::Tablet,
            Form::Laptop => Icon::Laptop,
            Form::Desktop => Icon::Monitor,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Device {
    pub(crate) preset: Option<Preset>,
    pub(crate) rotated: bool,
}

impl Device {
    pub(crate) fn of(preset: Option<Preset>) -> Self {
        Self { preset, rotated: false }
    }

    pub(crate) fn rotate(self) -> Self {
        Self { rotated: self.preset.is_some() && !self.rotated, ..self }
    }

    pub(crate) fn name(&self) -> &'static str {
        self.preset.map_or("Responsive", |p| p.name)
    }

    pub(crate) fn icon(&self) -> Icon {
        self.preset.as_ref().map_or(Icon::Devices, Preset::icon)
    }

    pub(crate) fn size(&self) -> Option<(f32, f32)> {
        let p = self.preset?;
        let (w, h) = (f32::from(p.width), f32::from(p.height));
        Some(if self.rotated { (h, w) } else { (w, h) })
    }

    fn touch(&self) -> bool {
        self.preset.is_some_and(|p| matches!(p.form, Form::Phone | Form::Tablet))
    }

    fn system(&self) -> System {
        self.preset.map_or(System::Desktop, |p| p.system)
    }

    pub(crate) fn same_agent(&self, other: &Device) -> bool {
        self.system() == other.system()
    }

    pub(crate) fn size_label(&self, view: &View) -> String {
        match self.size() {
            Some((w, h)) => format!("{} × {}", whole(w), whole(h)),
            None if view.known() => format!("{} × {}", whole(view.width), whole(view.height)),
            None => String::new(),
        }
    }

    pub(crate) fn scale_label(&self) -> Option<String> {
        self.preset.map(|p| format!("{}×", p.scale))
    }

    pub(crate) fn metrics(&self, view: &View) -> Option<Value> {
        let Some(p) = self.preset else { return responsive(view) };
        let (w, h) = self.size()?;
        let kind = if h > w { "portraitPrimary" } else { "landscapePrimary" };
        let angle = if self.rotated { 90 } else { 0 };
        Some(json!({
            "width": whole(w),
            "height": whole(h),
            "deviceScaleFactor": f64::from(p.scale),
            "mobile": self.touch(),
            "screenWidth": whole(w),
            "screenHeight": whole(h),
            "screenOrientation": { "type": kind, "angle": angle },
        }))
    }

    pub(crate) fn frame_pixels(&self, view: &View) -> (i64, i64) {
        let (Some(p), Some((w, h))) = (self.preset, self.size()) else {
            return (whole(view.width * view.scale), whole(view.height * view.scale));
        };
        let display = if view.scale > 0. { view.scale } else { UNKNOWN_SCALE };
        let shown = if view.known() { (view.width / w).min(view.height / h).min(1.) } else { 1. };
        let factor = p.scale.min(display * shown);
        (whole(w * factor), whole(h * factor))
    }

    pub(crate) fn user_agent(&self, desktop: &str) -> String {
        let Some(v) = chrome_version(desktop) else { return desktop.to_string() };
        let ios = |device: &str| {
            format!(
                "Mozilla/5.0 ({device}) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/{v} Mobile/15E148 Safari/604.1"
            )
        };
        match self.system() {
            System::Desktop => desktop.to_string(),
            System::Ios => ios(&format!("iPhone; CPU iPhone OS {IOS}")),
            System::Ipados => ios(&format!("iPad; CPU OS {IOS}")),
            System::Android => format!(
                "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{v} Mobile Safari/537.36"
            ),
        }
    }

    pub(crate) fn emulation(&self, desktop: Option<&str>) -> Vec<(&'static str, Value)> {
        let touch = self.touch();
        let points = if touch {
            json!({ "enabled": true, "maxTouchPoints": TOUCH_POINTS })
        } else {
            json!({ "enabled": false })
        };
        let mut calls = vec![
            ("Emulation.setTouchEmulationEnabled", points),
            ("Emulation.setEmitTouchEventsForMouse", json!({ "enabled": touch, "configuration": "mobile" })),
        ];
        if let Some(desktop) = desktop {
            calls.push(("Emulation.setUserAgentOverride", json!({ "userAgent": self.user_agent(desktop) })));
        }
        calls
    }
}

fn responsive(view: &View) -> Option<Value> {
    let scale = f64::from(view.scale);
    view.known().then(|| {
        json!({ "width": whole(view.width), "height": whole(view.height), "deviceScaleFactor": scale, "mobile": false })
    })
}

pub(crate) fn chrome_version(agent: &str) -> Option<&str> {
    let at = agent.find("Chrome/")? + "Chrome/".len();
    agent[at..].split_whitespace().next().filter(|v| !v.is_empty())
}

impl Browser {
    pub(super) fn set_device(&mut self, device: Device, cx: &mut Context<Self>) {
        let reload = !device.same_agent(&self.view.device);
        self.view.device = device;
        self.devices_open = false;
        self.apply_size();
        if reload {
            self.reload(cx);
        }
        cx.notify();
    }

    pub(super) fn rotate_device(&mut self, cx: &mut Context<Self>) {
        self.set_device(self.view.device.rotate(), cx);
    }

    pub(super) fn emulate(&self, session: &str) {
        for (method, params) in self.view.device.emulation(self.user_agent.as_deref()) {
            self.send(Some(session), method, params);
        }
    }
}

#[cfg(test)]
mod tests;
