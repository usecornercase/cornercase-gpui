use gpui::{
    BoxShadow, ClickEvent, Context, Div, ElementId, FontWeight, Hsla, InteractiveElement as _, ParentElement as _,
    Rgba, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, div, linear_color_stop,
    linear_gradient, point, prelude::FluentBuilder as _, px, rgb,
};

use super::FlowsPanel;
use super::diagram::{Plan, role_colour};
use super::graph::{Card, Rect};
use super::model::{self, Member, RoleSpec};
use super::widgets::show_pane;
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, glass, glass::tint, status_badge};

const AVATAR: f32 = 30.;
const CONDUCTOR: f32 = 34.;

pub(super) fn ground() -> Rgba {
    rgb(0x17_1a_1f)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Look {
    Plain,
    Focused,
    Holder,
    Working,
    Closed,
}

impl Look {
    fn of(plan: Plan, role: &str) -> Self {
        let member = plan.member(role);
        if member.is_some_and(|m| m.closed) {
            Self::Closed
        } else if member.and_then(|m| m.status.as_deref()) == Some("working") {
            Self::Working
        } else if plan.holder(role) {
            Self::Holder
        } else if plan.focus == Some(role) {
            Self::Focused
        } else {
            Self::Plain
        }
    }

    fn sheen(self) -> f32 {
        match self {
            Self::Working | Self::Holder => 0.26,
            Self::Focused => 0.2,
            Self::Plain | Self::Closed => 0.11,
        }
    }

    fn edge(self, colour: Rgba) -> Rgba {
        match self {
            Self::Working | Self::Focused => colour,
            Self::Holder => tint(colour, 0.75),
            Self::Plain | Self::Closed => tint(colour, 0.28),
        }
    }

    fn shadows(self, colour: Rgba) -> Vec<BoxShadow> {
        let dark = Hsla { h: 0., s: 0., l: 0., a: 0.35 };
        let mut shadows =
            vec![BoxShadow { color: dark, offset: point(px(0.), px(6.)), blur_radius: px(14.), spread_radius: px(0.) }];
        match self {
            Self::Working => shadows.extend(glass::glow(colour, 0.5, 20.)),
            Self::Focused => shadows.extend(glass::ring(colour)),
            Self::Plain | Self::Holder | Self::Closed => {}
        }
        shadows
    }
}

pub(super) fn avatar(name: &str, colour: Rgba, size: f32) -> Div {
    div()
        .size(px(size))
        .flex_none()
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(linear_gradient(150., linear_color_stop(colour, 0.), linear_color_stop(tint(colour, 0.62), 1.)))
        .text_size(px((size * 0.38).round()))
        .font_weight(FontWeight::BOLD)
        .text_color(colours::ink())
        .child(model::initials(name))
}

pub(super) fn with_status(avatar: Div, member: Option<&Member>) -> Div {
    badge(avatar, member.filter(|m| !m.closed).and_then(|m| m.status.as_deref()))
}

pub(super) fn badge(face: Div, status: Option<&str>) -> Div {
    face.relative().children(status_badge(status).map(|(_, colour)| {
        div()
            .absolute()
            .right(px(-3.))
            .bottom(px(-3.))
            .size(px(12.))
            .rounded_full()
            .bg(colour)
            .border_2()
            .border_color(ground())
    }))
}

fn shell(id: impl Into<ElementId>, rect: Rect, colour: Rgba, look: Look) -> Stateful<Div> {
    div()
        .id(id)
        .absolute()
        .left(px(rect.x))
        .top(px(rect.y))
        .w(px(rect.w))
        .h(px(rect.h))
        .rounded(px(radius::CARD))
        .bg(ground())
        .border_1()
        .border_color(look.edge(colour))
        .shadow(look.shadows(colour))
        .when(look == Look::Closed, |d| d.opacity(0.45))
        .child(div().absolute().top_0().left_0().size_full().rounded(px(radius::CARD)).bg(linear_gradient(
            180.,
            linear_color_stop(tint(colour, look.sheen()), 0.),
            linear_color_stop(tint(colour, 0.02), 1.),
        )))
}

fn body(avatar: Div, name: SharedString, line: Div) -> Div {
    div().relative().size_full().flex().items_center().gap(px(10.)).px(px(12.)).child(avatar).child(
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child(ui::one_line(name)))
            .child(line),
    )
}

pub(super) fn status_word(member: Option<&Member>) -> Option<(String, Rgba)> {
    let member = member?;
    if member.closed {
        return Some(("closed".into(), colours::faint()));
    }
    let status = member.status.as_deref()?;
    Some((status.to_string(), status_badge(Some(status)).map_or(colours::muted(), |(_, colour)| colour)))
}

fn facts(spec: Option<&RoleSpec>, member: Option<&Member>, running: bool) -> Div {
    let agent = spec.map(|r| r.agent_facts().join(" · ")).unwrap_or_default();
    let row = div().flex().items_center().gap(px(5.)).min_w_0().text_size(px(11.)).text_color(colours::muted());
    let status = if running { status_word(member) } else { None };
    row.children(status.map(|(word, colour)| {
        div()
            .flex()
            .flex_none()
            .gap(px(5.))
            .child(div().text_color(colour).child(word))
            .child(div().text_color(colours::faint()).child("·"))
    }))
    .child(div().flex_1().min_w_0().child(ui::one_line(agent)))
    .when(!running, |d| d.children(spec.map(capabilities)))
}

fn capabilities(spec: &RoleSpec) -> Div {
    let glyph = |icon| ui::icon(icon, 11.).text_color(colours::faint());
    div()
        .flex()
        .flex_none()
        .gap(px(4.))
        .child(glyph(if spec.edits { Icon::Pencil } else { Icon::Eye }))
        .when(spec.worktree == "own", |d| d.child(glyph(Icon::Branch)))
}

pub(super) fn conductor(size: f32) -> Div {
    div()
        .size(px(size))
        .flex_none()
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(glass::cobalt())
        .border_1()
        .border_color(tint(colours::brand_light(), 0.8))
        .shadow(glass::glow(colours::brand(), 0.45, size * 0.4))
        .child(ui::icon(Icon::Waypoints, (size * 0.5).round()).text_color(colours::on_brand()))
}

pub(super) fn role_card(plan: Plan, (index, card): (usize, &Card), cx: &Context<FlowsPanel>) -> Stateful<Div> {
    let colour = role_colour(plan.flow, &card.role);
    let spec = plan.flow.role(&card.role);
    let member = plan.member(&card.role);
    let avatar = with_status(avatar(&card.role, colour, AVATAR), member);
    let line = facts(spec, member, plan.run.is_some());
    let card_div = shell(("flow-card", index), card.rect, colour, Look::of(plan, &card.role)).child(body(
        avatar,
        card.role.clone().into(),
        line,
    ));
    clickable(card_div, plan, &card.role, colour, cx)
}

pub(super) fn lead_card(plan: Plan, rect: Rect, cx: &Context<FlowsPanel>) -> Stateful<Div> {
    let lead = &plan.flow.orchestrator;
    let name = if lead.name.is_empty() { "orchestrator" } else { lead.name.as_str() };
    let member = plan.run.and_then(|r| r.orchestrator());
    let line = facts(Some(lead), member, plan.run.is_some());
    let colour = colours::soft();
    let lead_div = shell("flow-lead", rect, colour, Look::of(plan, name)).child(body(
        with_status(conductor(CONDUCTOR), member),
        name.to_string().into(),
        line,
    ));
    clickable(lead_div, plan, name, colour, cx)
}

fn pointer(card: Stateful<Div>, colour: Rgba) -> Stateful<Div> {
    card.cursor_pointer().hover(move |s| s.border_color(colour))
}

fn clickable(card: Stateful<Div>, plan: Plan, role: &str, colour: Rgba, cx: &Context<FlowsPanel>) -> Stateful<Div> {
    if plan.run.is_none() {
        let name = role.to_string();
        return pointer(card, colour)
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_prompt(&name, cx)));
    }
    match plan.member(role).filter(|m| !m.closed).and_then(|m| m.pane) {
        Some(pane) => pointer(card, colour).on_click(move |_: &ClickEvent, _, cx| show_pane(pane, cx)),
        None => card,
    }
}
