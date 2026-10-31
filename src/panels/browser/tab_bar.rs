use std::cell::RefCell;

use gpui::{
    AnyElement, ClickEvent, Context, Div, InteractiveElement as _, IntoElement, MouseButton, MouseUpEvent,
    ParentElement as _, Rgba, ScrollHandle, SharedString, Stateful, StatefulInteractiveElement as _, Styled, Window,
    div, prelude::FluentBuilder as _, px, transparent_black,
};
use serde_json::json;

use super::Browser;
use super::tabs::Tab;
use crate::notify::tip;
use crate::theme::{colours, radius};
use crate::ui::{self, Icon};

const BAR: f32 = 40.;
const TAB: f32 = 28.;
const NARROWEST: f32 = 44.;
const NARROWEST_ACTIVE: f32 = 120.;
const WIDEST: f32 = 220.;
const CLOSE: f32 = 20.;
const BLANK: &str = "about:blank";

#[derive(Default)]
pub(crate) struct Strip {
    scroll: ScrollHandle,
    followed: RefCell<Option<String>>,
}

impl Strip {
    fn follow(&self, active: Option<&str>, faces: &[Face]) {
        if self.followed.borrow().as_deref() == active {
            return;
        }
        self.followed.replace(active.map(str::to_string));
        if let Some(at) = faces.iter().position(|f| f.active) {
            self.scroll.scroll_to_item(at);
        }
    }
}

struct Face {
    target: String,
    label: String,
    detail: String,
    loading: bool,
    active: bool,
    agent: bool,
    notice: Option<AnyElement>,
}

pub(crate) fn tidy(text: &str) -> String {
    let text = text.trim();
    if text.is_empty() || text == BLANK {
        return "New tab".to_string();
    }
    let bare = text.strip_prefix("https://").or_else(|| text.strip_prefix("http://")).unwrap_or(text);
    if bare.contains(char::is_whitespace) {
        return bare.to_string();
    }
    bare.strip_suffix('/').unwrap_or(bare).to_string()
}

pub(crate) fn detail(title: &str, url: &str) -> String {
    let title = title.trim();
    if title.is_empty() || title == url || url == BLANK { tidy(url) } else { format!("{title}\n{url}") }
}

impl Face {
    fn of(tab: &Tab, active: Option<&str>, agent: bool) -> Self {
        Self {
            target: tab.target.clone(),
            label: tidy(tab.label()),
            detail: detail(&tab.title, &tab.url),
            loading: tab.loading,
            active: active == Some(tab.target.as_str()),
            agent,
            notice: None,
        }
    }
}

impl Browser {
    pub(super) fn tab_bar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let active = self.tabs.active_target(self.workspace);
        let faces: Vec<Face> = self
            .tabs
            .shown(self.workspace)
            .map(|t| Face {
                notice: self.tab_mark(&t.target),
                ..Face::of(t, active.as_deref(), self.agent_tab(&t.target))
            })
            .collect();
        if faces.len() < 2 {
            return None;
        }
        self.strip.follow(active.as_deref(), &faces);
        Some(strip(faces, &self.strip.scroll, cx))
    }

    pub(super) fn refresh_title(&mut self, session: Option<&str>, cx: &mut Context<Self>) {
        let Some(target) = session.and_then(|s| self.tabs.by_session(s)).map(|t| t.target.clone()) else { return };
        let ask = json!({ "targetId": target });
        self.call((None, "Target.getTargetInfo", ask), cx, move |this, result, cx| {
            let Some(title) = result.ok().and_then(|r| r["targetInfo"]["title"].as_str().map(str::to_string)) else {
                return;
            };
            if let Some(tab) = this.tabs.get_mut(&target) {
                tab.title = title;
                cx.notify();
            }
        });
    }

    fn pick_tab(&mut self, target: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.show_tab(target, cx);
        self.address_url.clear();
        window.focus(&self.focus);
    }
}

fn strip(faces: Vec<Face>, scroll: &ScrollHandle, cx: &mut Context<Browser>) -> Stateful<Div> {
    div()
        .id("browser-tab-bar")
        .h(px(BAR))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(4.))
        .px(px(8.))
        .border_b_1()
        .border_color(colours::border())
        .overflow_x_scroll()
        .track_scroll(scroll)
        .children(faces.into_iter().map(|face| tab(face, cx)))
}

fn look(active: bool) -> (Rgba, Rgba, Rgba) {
    if active {
        (colours::selected(), colours::border(), colours::text())
    } else {
        (Rgba::from(transparent_black()), Rgba::from(transparent_black()), colours::muted())
    }
}

fn tab(mut face: Face, cx: &mut Context<Browser>) -> Stateful<Div> {
    let notice = face.notice.take();
    let group = SharedString::from(format!("browser-tab-{}", face.target));
    let (fill, line, ink) = look(face.active);
    let (shown, closed) = (face.target.clone(), face.target.clone());
    div()
        .id(group.clone())
        .group(group.clone())
        .flex_1()
        .min_w(px(if face.active { NARROWEST_ACTIVE } else { NARROWEST }))
        .max_w(px(WIDEST))
        .h(px(TAB))
        .flex()
        .items_center()
        .gap(px(6.))
        .pl(px(9.))
        .pr(px(4.))
        .rounded(px(radius::CONTROL))
        .border_1()
        .border_color(line)
        .bg(fill)
        .text_color(ink)
        .cursor_pointer()
        .when(!face.active, |d| d.hover(|s| s.bg(colours::hover()).text_color(colours::soft())))
        .child(mark(&face))
        .child(div().flex_1().min_w_0().text_size(px(12.)).child(ui::one_line(face.label.clone())))
        .children(notice)
        .child(close_button(&face, group, cx))
        .tooltip(tip(face.detail))
        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.pick_tab(&shown, window, cx)))
        .on_mouse_up(MouseButton::Middle, cx.listener(move |this, _: &MouseUpEvent, _, _| this.close_tab(&closed)))
}

fn mark(face: &Face) -> impl IntoElement + use<> {
    let slot = div().size(px(14.)).flex_none().flex().items_center().justify_center();
    if face.loading {
        return slot.child(ui::spinner(
            SharedString::from(format!("browser-tab-load-{}", face.target)),
            12.,
            colours::accent(),
        ));
    }
    let glyph = if face.agent { Icon::Agent } else { Icon::Globe };
    slot.child(ui::icon(glyph, 12.).text_color(if face.active || face.agent {
        colours::accent()
    } else {
        colours::faint()
    }))
}

fn close_button(face: &Face, group: SharedString, cx: &mut Context<Browser>) -> Stateful<Div> {
    let target = face.target.clone();
    ui::tool_button(SharedString::from(format!("browser-tab-x-{}", face.target)), Icon::Close, CLOSE, false)
        .when(!face.active, |d| d.invisible().group_hover(group, Styled::visible))
        .tooltip(tip("Close tab"))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            cx.stop_propagation();
            this.close_tab(&target);
        }))
}

#[cfg(test)]
mod tests;
