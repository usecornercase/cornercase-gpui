//! The TODO list (`cornercase todo`, `status --app`).

use gpui::{AnyView, AppContext as _, Context, IntoElement, Render, Window};

use crate::ui;

pub struct TodoPanel {}

impl TodoPanel {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {}
    }
}

impl Render for TodoPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        ui::note("TODO: not built yet")
    }
}

pub fn build(window: &mut Window, cx: &mut gpui::App) -> AnyView {
    cx.new(|cx| TodoPanel::new(window, cx)).into()
}
