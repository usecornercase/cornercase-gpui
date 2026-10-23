//! The issue trackers: lists, an issue, starting work on it (`cornercase issue`, `start --issue`).

use gpui::{AnyView, AppContext as _, Context, IntoElement, Render, Window};

use crate::ui;

pub struct IssuesPanel {}

impl IssuesPanel {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {}
    }
}

impl Render for IssuesPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        ui::note("Issues: not built yet")
    }
}

pub fn build(window: &mut Window, cx: &mut gpui::App) -> AnyView {
    cx.new(|cx| IssuesPanel::new(window, cx)).into()
}
