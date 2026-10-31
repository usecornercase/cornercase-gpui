pub(crate) mod brand;
pub(crate) mod file_icons;
pub(crate) mod glass;
pub(crate) mod icons;
pub(crate) mod motion;
pub(crate) mod one_line;
pub(crate) mod signs;
pub(crate) mod split;
pub(crate) mod text_field;

use gpui::{
    AnimationElement, AnyElement, App, Div, ElementId, Entity, Focusable as _, FontWeight, Hsla,
    InteractiveElement as _, IntoElement, ParentElement as _, Rgba, SharedString, Stateful, Styled as _, Svg, Window,
    div, prelude::FluentBuilder as _, px, relative,
};

pub(crate) use icons::{Icon, icon};
pub(crate) use one_line::{OneLine, one_line};
pub(crate) use signs::{
    ROW_ACTION, accent_chip, callout, done_line, dot, error_line, field_action, quiet_chip, resting_action, row_action,
    tinted_chip, warning_line,
};
pub(crate) use text_field::{TextField, TextFieldEvent};

use crate::theme::{colours, radius};

fn button_base(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .px(px(12.))
        .py(px(4.))
        .rounded(px(radius::CONTROL))
        .text_size(px(12.))
        .border_1()
        .cursor_pointer()
        .whitespace_nowrap()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tone {
    Plain,
    Primary,
    Danger,
}

fn ink(tone: Tone) -> Rgba {
    match tone {
        Tone::Plain => colours::soft(),
        Tone::Primary => colours::on_brand(),
        Tone::Danger => colours::failed(),
    }
}

fn filled(el: Stateful<Div>, tone: Tone) -> Stateful<Div> {
    match tone {
        Tone::Plain => el.bg(colours::hover()).border_color(colours::border()),
        Tone::Primary => el
            .font_weight(FontWeight::MEDIUM)
            .text_color(colours::on_brand())
            .bg(glass::cobalt())
            .border_color(glass::tint(colours::brand_light(), 0.7))
            .shadow(glass::glow(colours::brand(), 0.28, 10.)),
        Tone::Danger => el
            .text_color(colours::failed())
            .bg(glass::tint(colours::failed(), 0.1))
            .border_color(glass::tint(colours::failed(), 0.35)),
    }
}

fn pressable(el: Stateful<Div>, tone: Tone) -> Stateful<Div> {
    let el = filled(el, tone);
    match tone {
        Tone::Plain => el.hover(|s| s.bg(colours::selected()).border_color(colours::edge())),
        Tone::Primary => el.hover(|s| s.bg(colours::brand_hover())),
        Tone::Danger => {
            el.hover(|s| s.bg(glass::tint(colours::failed(), 0.18)).border_color(glass::tint(colours::failed(), 0.7)))
        }
    }
}

macro_rules! anonymous_element {
    () => {
        fn id(&self) -> Option<gpui::ElementId> {
            None
        }

        fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
            None
        }
    };
}

pub(crate) use anonymous_element;

pub(crate) fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    button_coloured(id, label, colours::soft())
}

pub(crate) fn primary_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    pressable(button_base(id).child(label.into()), Tone::Primary)
}

pub(crate) fn danger_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    pressable(button_base(id).child(label.into()), Tone::Danger)
}

pub(crate) fn button_coloured(id: impl Into<ElementId>, label: impl Into<SharedString>, colour: Rgba) -> Stateful<Div> {
    pressable(button_base(id).text_color(colour).child(label.into()), Tone::Plain)
}

pub(crate) fn spinner(id: impl Into<ElementId>, size: f32, colour: Rgba) -> AnimationElement<Svg> {
    motion::spin(icon(Icon::Loader, size).text_color(colour), id)
}

pub(crate) fn busy_button(id: impl Into<ElementId>, label: impl Into<SharedString>, tone: Tone) -> Stateful<Div> {
    let el = button_base(id).flex().items_center().gap(px(6.)).text_color(ink(tone)).cursor_default();
    filled(el, tone).child(spinner("spinner", 12., ink(tone))).child(label.into())
}

pub(crate) fn resting_button(id: impl Into<ElementId>, label: impl Into<SharedString>, tone: Tone) -> Stateful<Div> {
    filled(button_base(id).text_color(ink(tone)).cursor_default().child(label.into()), tone).opacity(0.4)
}

pub(crate) fn busy(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .text_size(px(12.))
        .text_color(colours::muted())
        .child(spinner(id, 12., colours::muted()))
        .child(one_line(text))
}

pub(crate) fn icon_button(id: impl Into<ElementId>, icon: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .px(px(6.))
        .py(px(1.))
        .rounded(px(radius::CHIP))
        .text_color(colours::faint())
        .cursor_pointer()
        .hover(|s| s.bg(colours::selected()).text_color(colours::text()))
        .child(icon.into())
}

pub(crate) fn tool_button(id: impl Into<ElementId>, glyph: Icon, size: f32, active: bool) -> Stateful<Div> {
    let id: ElementId = id.into();
    let group = SharedString::from(format!("{id}"));
    let glyph = icon(glyph, (size * 0.5).round());
    let el = div()
        .id(id)
        .group(group.clone())
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded(px(radius::CONTROL))
        .cursor_pointer();
    if active {
        return el.bg(colours::accent_soft()).child(glyph.text_color(colours::accent()));
    }
    el.hover(|s| s.bg(colours::hover()))
        .child(glyph.text_color(colours::muted()).group_hover(group, |s| s.text_color(colours::text())))
}

pub(crate) fn icon_text_button(
    id: impl Into<ElementId>,
    glyph: Icon,
    label: impl Into<SharedString>,
    tone: Tone,
) -> Stateful<Div> {
    let colour = ink(tone);
    let el = button_base(id)
        .flex()
        .items_center()
        .gap(px(6.))
        .text_color(colour)
        .child(icon(glyph, 14.).text_color(colour))
        .child(label.into());
    pressable(el, tone)
}

pub(crate) fn labelled_button(
    id: impl Into<ElementId>,
    glyph: Icon,
    label: impl Into<SharedString>,
    prominent: bool,
) -> Stateful<Div> {
    let tone = if prominent { Tone::Primary } else { Tone::Plain };
    icon_text_button(id, glyph, label, tone).flex_1().justify_center().h(px(30.))
}

pub(crate) fn status_badge(status: Option<&str>) -> Option<(Icon, Rgba)> {
    match status? {
        "working" => Some((Icon::Working, colours::working())),
        "waiting" => Some((Icon::Waiting, colours::waiting())),
        "done" => Some((Icon::Done, colours::done())),
        "failed" => Some((Icon::Failed, colours::failed())),
        "pending" => Some((Icon::Pending, colours::accent())),
        "idle" => Some((Icon::Idle, colours::faint())),
        _ => None,
    }
}

pub(crate) fn badge(status: Option<&str>) -> Div {
    let slot = div().w(px(14.)).flex_none().flex().items_center().justify_center();
    let Some((glyph, colour)) = status_badge(status) else {
        return slot;
    };
    let glyph = icon(glyph, 12.).text_color(colour);
    slot.child(if status == Some("working") { motion::orbit(glyph) } else { glyph })
}

pub(crate) fn section(title: impl Into<SharedString>) -> Div {
    let title: SharedString = title.into();
    div()
        .pt(px(14.))
        .pb(px(6.))
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(colours::muted())
        .child(title.to_uppercase())
}

pub(crate) fn row(id: impl Into<ElementId>, selected: bool) -> Stateful<Div> {
    div()
        .id(id)
        .relative()
        .flex()
        .items_center()
        .gap(px(8.))
        .px(px(10.))
        .py(px(6.))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .when(selected, glass::lit)
        .when(!selected, |d| d.hover(|s| s.bg(colours::hover())))
}

pub(crate) fn note(text: impl Into<SharedString>) -> Div {
    div().p(px(16.)).text_color(colours::muted()).child(text.into())
}

pub(crate) fn loading(text: impl Into<SharedString>) -> Div {
    busy("loading", text).p(px(16.)).text_size(px(13.))
}

pub(crate) fn undo_bar(text: impl Into<SharedString>, undo: Stateful<Div>) -> Div {
    div()
        .flex_none()
        .px(px(12.))
        .py(px(8.))
        .flex()
        .items_center()
        .gap(px(8.))
        .rounded(px(radius::CONTROL))
        .bg(colours::panel())
        .border_1()
        .border_color(colours::border())
        .child(div().flex_1().min_w(px(0.)).text_size(px(12.)).child(one_line(text)))
        .child(undo)
}

pub(crate) fn dim(text: impl IntoElement) -> Div {
    div().text_size(px(11.)).text_color(colours::faint()).child(text)
}

pub(crate) fn clamped(lines: u8, line_height: f32) -> Div {
    div().line_height(px(line_height)).max_h(px(f32::from(lines) * line_height)).overflow_hidden()
}

pub(crate) fn field(label: impl Into<SharedString>, control: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(div().text_size(px(11.)).text_color(colours::muted()).child(label.into()))
        .child(control)
}

pub(crate) fn input_box(field: impl IntoElement, focused: bool) -> Div {
    div()
        .flex()
        .flex_col()
        .px(px(10.))
        .py(px(6.))
        .rounded(px(radius::CONTROL))
        .bg(colours::field())
        .border_1()
        .border_color(if focused { colours::accent() } else { colours::edge() })
        .when(focused, |d| d.shadow(glass::ring(colours::brand())))
        .child(field)
}

pub(crate) fn boxed(field: &Entity<TextField>, window: &Window, cx: &App) -> Div {
    input_box(field.clone(), field.read(cx).focus_handle(cx).is_focused(window))
}

pub(crate) fn search_field(field: Entity<TextField>, focused: bool, clear: Option<Stateful<Div>>) -> Div {
    input_box(
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(icon(Icon::Search, 13.).text_color(if focused { colours::accent() } else { colours::muted() }))
            .child(div().flex_1().min_w_0().child(field))
            .children(clear),
        focused,
    )
}

pub(crate) fn bar(fraction: f32, colour: impl Into<Hsla>, height: f32) -> Div {
    let fraction = fraction.clamp(0., 1.);
    div().w_full().h(px(height)).rounded_full().bg(colours::border()).overflow_hidden().when(fraction > 0., |d| {
        d.child(div().h_full().min_w(px(height)).w(relative(fraction)).rounded_full().bg(colour.into()))
    })
}

pub(crate) fn checkbox(id: impl Into<ElementId>, checked: bool, label: impl Into<SharedString>) -> Stateful<Div> {
    let tick = div()
        .size(px(15.))
        .flex_none()
        .rounded(px(radius::CHIP))
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(if checked { colours::brand_light() } else { colours::faint() })
        .when(checked, |d| {
            d.bg(glass::cobalt())
                .shadow(glass::glow(colours::brand(), 0.3, 6.))
                .text_size(px(10.))
                .text_color(colours::on_brand())
                .child("✓")
        });
    div().id(id).flex().items_center().gap(px(8.)).cursor_pointer().child(tick).child(label.into())
}

pub(crate) fn segments(items: Vec<AnyElement>) -> Div {
    div()
        .flex()
        .gap(px(2.))
        .p(px(3.))
        .rounded(px(radius::CONTROL))
        .bg(colours::sidebar())
        .border_1()
        .border_color(colours::border())
        .children(items)
}

pub(crate) fn segment(id: impl Into<ElementId>, label: impl Into<SharedString>, selected: bool) -> Stateful<Div> {
    div()
        .id(id)
        .relative()
        .px(px(12.))
        .py(px(3.))
        .rounded(px(radius::CHIP))
        .text_size(px(12.))
        .cursor_pointer()
        .when(selected, |d| d.bg(colours::selected()).text_color(colours::text()))
        .when(!selected, |d| {
            d.text_color(colours::muted()).hover(|s| s.bg(colours::hover()).text_color(colours::text()))
        })
        .child(label.into())
}

pub(crate) fn home_relative(path: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    match path.strip_prefix(&home) {
        Some(rest) if !home.is_empty() => format!("~{rest}"),
        _ => path.to_string(),
    }
}

pub(crate) fn title(text: impl Into<SharedString>) -> Div {
    div().font_weight(FontWeight::SEMIBOLD).child(text.into())
}

pub(crate) fn emblem(glyph: Icon) -> Div {
    div()
        .size(px(52.))
        .mb(px(4.))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(glass::tint(colours::brand(), 0.16))
        .border_1()
        .border_color(glass::tint(colours::brand_light(), 0.35))
        .child(icon(glyph, 24.).text_color(colours::accent()))
}
