//! Plan limits and stats (`cornercase usage`, `cornercase stats`).

use gpui::{AnyView, AppContext as _, Context, IntoElement, Render, Window};

use crate::ui;

pub struct UsagePanel {}

impl UsagePanel {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {}
    }
}

impl Render for UsagePanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        ui::note("Usage: not built yet")
    }
}

pub fn build(window: &mut Window, cx: &mut gpui::App) -> AnyView {
    cx.new(|cx| UsagePanel::new(window, cx)).into()
}
