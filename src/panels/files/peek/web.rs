use anyhow::Result;
use gpui::{
    AnyElement, ClickEvent, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use super::body::{Body, Picture, note, picture};
use super::card::{header_row, picture_body};
use super::place::{NARROW, Place};
use super::{Anchor, Peek, Subject};
use crate::notify::tip;
use crate::panels::browser::{self, Browser, Preview};
use crate::store::Store;
use crate::theme::colours;
use crate::ui::{self, Icon};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WebPage {
    pub url: String,
    pub load: bool,
    pub outside: bool,
}

fn without_scheme(url: &str) -> &str {
    url.split_once("://").map_or(url, |(_, rest)| rest)
}

pub(super) fn names(title: &str, url: &str) -> (String, String) {
    let rest = without_scheme(url);
    if !title.is_empty() {
        return (title.to_string(), rest.to_string());
    }
    let (host, path) = rest.split_at(rest.find(['/', '?', '#']).unwrap_or(rest.len()));
    (host.to_string(), path.to_string())
}

impl Peek {
    pub(crate) fn hover_web(&mut self, page: WebPage, anchor: Anchor, cx: &mut Context<Self>) {
        self.over_link = true;
        self.anchor = anchor;
        if self.page().is_some_and(|shown| shown.url == page.url) {
            cx.notify();
            return;
        }
        self.show(Subject::Web(page), cx);
    }

    pub(super) fn page(&self) -> Option<&WebPage> {
        match &self.subject {
            Some(Subject::Web(page)) => Some(page),
            _ => None,
        }
    }

    pub(super) fn open_page(&mut self, page: &WebPage, cx: &mut Context<Self>) {
        match browser::running(cx).filter(|_| page.load) {
            Some(browser) => self.capture(&browser, page.url.clone(), cx),
            None => self.body = Some(Body::Ask),
        }
    }

    fn load_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(url) = self.page().map(|page| page.url.clone()) else { return };
        let browser = browser::shared(window, cx);
        self.capture(&browser, url, cx);
        cx.notify();
    }

    fn capture(&mut self, browser: &Entity<Browser>, url: String, cx: &mut Context<Self>) {
        self.body = Some(Body::Loading);
        let generation = self.generation;
        let task = browser.update(cx, |browser, cx| browser.preview(url, cx));
        self.capturing = Some(cx.spawn(async move |this, cx| {
            let shot = task.await;
            let _ = this.update(cx, |this, cx| this.captured(generation, shot, cx));
        }));
    }

    fn captured(&mut self, generation: u64, shot: Result<Preview>, cx: &mut Context<Self>) {
        if generation != self.generation {
            return;
        }
        self.capturing = None;
        let still = match shot {
            Ok(preview) => {
                self.title = preview.title;
                picture(preview.image, String::new())
            }
            Err(e) => note("Could not show the page", format!("{e:#}")),
        };
        self.body = Some(Body::Still(still));
        cx.notify();
    }

    fn open_web(&mut self, in_panel: bool, cx: &mut Context<Self>) {
        let Some(url) = self.page().map(|page| page.url.clone()) else { return };
        self.close(cx);
        if in_panel {
            Store::get(cx).update(cx, |s, cx| s.open_in_browser(url, None, cx));
        } else {
            cx.open_url(&url);
        }
    }

    fn open_as_set(&mut self, cx: &mut Context<Self>) {
        let outside = self.page().is_some_and(|page| page.outside);
        let in_panel = Store::get(cx).read(cx).prefs.opens_in_panel(outside);
        self.open_web(in_panel, cx);
    }

    pub(super) fn page_shot(shot: &Picture, place: &Place, cx: &mut Context<Self>) -> (AnyElement, f32) {
        let (body, width) = picture_body(shot, place);
        let body = div()
            .id("peek-page")
            .cursor_pointer()
            .child(body)
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.open_as_set(cx)));
        (body.into_any_element(), width)
    }

    pub(super) fn ask_body(cx: &mut Context<Self>) -> (AnyElement, f32) {
        let why = "Loading a page can act on it, such as using up a one-time sign-in link. Settings › Links \
                   chooses which pages load at once.";
        let body = div()
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(div().text_color(colours::soft()).child("This page loads when you ask"))
            .child(div().text_size(px(12.)).text_color(colours::faint()).child(why))
            .child(
                div().pt(px(4.)).flex().child(
                    ui::button("peek-load-page", "Show the page")
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.load_page(window, cx))),
                ),
            );
        (body.into_any_element(), NARROW + 60.)
    }

    pub(super) fn web_header(&self, page: &WebPage, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let icon = ui::icon(Icon::Globe, 14.).text_color(colours::muted());
        header_row(icon, names(&self.title, &page.url)).child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(px(2.))
                .child(
                    ui::tool_button("peek-open-browser", Icon::Globe, 26., false)
                        .tooltip(tip("Open in the Browser panel"))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.open_web(true, cx))),
                )
                .when(page.outside, |d| {
                    d.child(
                        ui::tool_button("peek-open-outside", Icon::External, 26., false)
                            .tooltip(tip("Open in your default browser"))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.open_web(false, cx))),
                    )
                }),
        )
    }
}

#[cfg(test)]
mod tests;
