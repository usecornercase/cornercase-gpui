//! Find where some text was said, in transcripts and pane history (`cornercase search`).

use gpui::{AnyView, AppContext as _, Context, IntoElement, Render, Window};

use crate::ui;

pub struct SearchPanel {}

impl SearchPanel {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {}
    }
}

impl Render for SearchPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        ui::note("Search: not built yet")
    }
}

pub fn build(window: &mut Window, cx: &mut gpui::App) -> AnyView {
    cx.new(|cx| SearchPanel::new(window, cx)).into()
}
