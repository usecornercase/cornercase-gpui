//! The workspace's files: a tree, a viewer and a search. Read from disk, as #345 says a GUI should.

use gpui::{AnyView, AppContext as _, Context, IntoElement, Render, Window};

use crate::ui;

pub struct FilesPanel {}

impl FilesPanel {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {}
    }
}

impl Render for FilesPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        ui::note("Files: not built yet")
    }
}

pub fn build(window: &mut Window, cx: &mut gpui::App) -> AnyView {
    cx.new(|cx| FilesPanel::new(window, cx)).into()
}
