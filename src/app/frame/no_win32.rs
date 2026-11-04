use gpui::Window;

pub(super) fn backdrop(_window: &Window) -> Result<(), String> {
    Err("Windows' backdrop exists only on Windows".to_string())
}
