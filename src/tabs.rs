//! The tabs of the shown workspace, above the terminal, with the split buttons.

use gpui::{
    ClickEvent, Context, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement,
    Styled, div, prelude::FluentBuilder as _, px,
};

use crate::app::{Root, TITLEBAR_HEIGHT};
use crate::model::Tab;
use crate::theme::colours;
use crate::ui::{badge, button_coloured as button};

impl Root {
    pub(crate) fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let shown_tab = self.session.as_ref().and_then(|s| s.shown().tab);
        let mut tabs = div().flex().items_end().gap(px(2.)).h_full().overflow_hidden();
        if let Some((_, ws)) = self.shown_workspace() {
            for tab in &ws.tabs {
                let id = tab.id;
                let selected = Some(id) == shown_tab;
                let agent = tab.agent_pane();
                let detail = agent.and_then(|a| match (&a.model, a.context) {
                    (Some(m), Some(c)) => Some(format!("{m} · {c}%")),
                    (Some(m), None) => Some(m.clone()),
                    _ => None,
                });
                let confirming = self.confirm_close == Some(id);
                let tab_clone = tab.clone();
                tabs = tabs.child(
                    div()
                        .id(SharedString::from(format!("tab-{id}")))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .h(px(30.))
                        .px(px(10.))
                        .rounded_t(px(6.))
                        .cursor_pointer()
                        .when(selected, |d| d.bg(colours::terminal()).text_color(colours::text()))
                        .when(!selected, |d| d.text_color(colours::muted()).hover(|s| s.bg(colours::hover())))
                        .child(badge(tab.loudest()))
                        .child(div().whitespace_nowrap().child(tab.name.clone()))
                        .when_some(detail, |d, text| {
                            d.child(div().text_size(px(11.)).text_color(colours::faint()).whitespace_nowrap().child(text))
                        })
                        .child(
                            div()
                                .id(SharedString::from(format!("tab-close-{id}")))
                                .px(px(4.))
                                .rounded(px(4.))
                                .text_size(px(11.))
                                .when(confirming, |d| d.bg(colours::failed()).text_color(colours::sidebar()).child("close?"))
                                .when(!confirming, |d| {
                                    d.text_color(colours::faint()).hover(|s| s.bg(colours::selected()).text_color(colours::text())).child("✕")
                                })
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    cx.stop_propagation();
                                    this.close_tab(&tab_clone, cx);
                                })),
                        )
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, _| this.focus_item("tab", id))),
                );
            }
        }
        let new_tab = button("new-tab", "+", colours::muted())
            .on_click(cx.listener(|this, _: &ClickEvent, _, _| this.act(vec!["new-tab".into(), "--focus".into()])));
        div()
            .h(px(TITLEBAR_HEIGHT))
            .flex_none()
            .flex()
            .items_end()
            .justify_between()
            .px(px(8.))
            .bg(colours::sidebar())
            .border_b_1()
            .border_color(colours::border())
            .child(div().flex().items_end().h_full().gap(px(4.)).overflow_hidden().child(tabs).child(
                div().pb(px(5.)).child(new_tab),
            ))
            .child(
                div()
                    .flex()
                    .gap(px(4.))
                    .pb(px(5.))
                    .child(
                        button("split-right", "Split →", colours::muted())
                            .on_click(cx.listener(|this, _: &ClickEvent, _, _| this.split(false))),
                    )
                    .child(
                        button("split-down", "Split ↓", colours::muted())
                            .on_click(cx.listener(|this, _: &ClickEvent, _, _| this.split(true))),
                    ),
            )
    }

    pub(crate) fn close_tab(&mut self, tab: &Tab, cx: &mut Context<Self>) {
        // A tab running an agent asks for a second click, as closing it stops the agent.
        if tab.agent_pane().is_some() && self.confirm_close != Some(tab.id) {
            self.confirm_close = Some(tab.id);
            cx.notify();
            return;
        }
        self.confirm_close = None;
        self.queue.cmd(vec!["close".into(), "--tab".into(), tab.id.to_string()]);
    }
}
