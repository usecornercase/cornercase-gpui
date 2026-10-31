use gpui::{
    ClickEvent, Context, Corner, Div, InteractiveElement as _, IntoElement, MouseDownEvent, ParentElement as _,
    SharedString, Stateful, StatefulInteractiveElement as _, StyleRefinement, Styled as _, anchored, deferred, div,
    point, prelude::FluentBuilder as _, px,
};

use super::Browser;
use super::devices::{Device, PRESETS};
use super::toolbar::BUTTON;
use crate::notify::tip;
use crate::theme::{colours, radius};
use crate::ui::motion::{self, Way};
use crate::ui::{self, Icon, glass};

const MENU_WIDTH: f32 = 252.;
const ROW: f32 = 30.;
const BAR: f32 = 34.;
const DROP: f32 = 4.;
const BELOW: f32 = 6.;
const SMALL: f32 = 24.;

impl Browser {
    pub(super) fn device_slot(&self, cx: &mut Context<Self>) -> Div {
        div()
            .relative()
            .flex_none()
            .child(self.device_button(cx))
            .when(self.devices_open, |d| d.child(self.device_popover(cx)))
    }

    fn device_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let preset = self.view.device.preset;
        let label = preset.map_or_else(|| "Device".to_string(), |p| format!("Device: {}", p.name));
        ui::tool_button("browser-devices", Icon::Devices, BUTTON, preset.is_some() || self.devices_open)
            .tooltip(tip(label))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.devices_open = true;
                cx.notify();
            }))
    }

    fn close_devices(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.devices_open) {
            cx.notify();
        }
    }

    fn device_popover(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let rows: Vec<_> = PRESETS.iter().map(|p| self.device_row(Device::of(Some(*p)), cx)).collect();
        let card = glass::card(radius::CARD)
            .id("browser-devices-menu")
            .occlude()
            .w(px(MENU_WIDTH))
            .p(px(4.))
            .flex()
            .flex_col()
            .text_size(px(12.))
            .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| this.close_devices(cx)))
            .child(self.device_row(Device::default(), cx))
            .child(div().mx(px(8.)).my(px(4.)).h(px(1.)).bg(colours::border()))
            .children(rows);
        let shown = motion::slide(card, "browser-devices-in", Way::Pop, (0., -DROP));
        let placed =
            anchored().snap_to_window_with_margin(px(8.)).offset(point(px(0.), px(BELOW))).anchor(Corner::TopRight);
        div().absolute().right_0().top_full().child(deferred(placed.child(shown)).with_priority(2))
    }

    pub(super) fn device_backdrop(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let backdrop = div()
            .id("browser-devices-backdrop")
            .absolute()
            .inset_0()
            .occlude()
            .on_any_mouse_down(cx.listener(|this, _: &MouseDownEvent, _, cx| this.close_devices(cx)));
        self.devices_open.then(|| deferred(backdrop).with_priority(1))
    }

    fn device_row(&self, device: Device, cx: &mut Context<Self>) -> Stateful<Div> {
        let current = self.view.device.preset.map(|p| p.name) == device.preset.map(|p| p.name);
        let group = SharedString::from(format!("browser-device-{}", device.name()));
        div()
            .id(group.clone())
            .group(group.clone())
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(ROW))
            .px(px(8.))
            .text_color(colours::text())
            .rounded(px(radius::CONTROL))
            .hover(|s| s.bg(glass::cobalt()).text_color(colours::on_brand()))
            .cursor_pointer()
            .child(tick(current, &group))
            .child(ui::icon(device.icon(), 14.).text_color(colours::muted()).group_hover(group.clone(), on_brand))
            .child(div().flex_1().min_w_0().child(ui::one_line(device.name())))
            .child(
                div()
                    .flex_none()
                    .text_color(colours::faint())
                    .group_hover(group, on_brand)
                    .child(device.size_label(&self.view)),
            )
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_device(device, cx)))
    }

    pub(super) fn device_bar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let device = self.view.device;
        device.preset?;
        let facts = [Some(device.size_label(&self.view)), device.scale_label()];
        Some(
            div()
                .h(px(BAR))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .px(px(8.))
                .border_b_1()
                .border_color(colours::border())
                .text_size(px(12.))
                .child(ui::icon(device.icon(), 13.).text_color(colours::accent()))
                .child(div().text_color(colours::soft()).child(device.name()))
                .children(facts.into_iter().flatten().map(|fact| div().text_color(colours::faint()).child(fact)))
                .child(self.device_tools(cx)),
        )
    }

    fn device_tools(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let rotated = self.view.device.rotated;
        div()
            .flex()
            .items_center()
            .gap(px(2.))
            .child(
                ui::tool_button("browser-rotate", Icon::Rotate, SMALL, rotated)
                    .tooltip(tip("Rotate"))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.rotate_device(cx))),
            )
            .child(
                ui::tool_button("browser-responsive", Icon::Close, SMALL, false)
                    .tooltip(tip("Back to responsive"))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_device(Device::default(), cx))),
            )
    }
}

fn on_brand(style: StyleRefinement) -> StyleRefinement {
    style.text_color(colours::on_brand())
}

fn tick(current: bool, group: &SharedString) -> Div {
    let slot = div().flex_none().size(px(13.));
    if !current {
        return slot;
    }
    slot.child(ui::icon(Icon::Check, 13.).text_color(colours::accent()).group_hover(group.clone(), on_brand))
}
