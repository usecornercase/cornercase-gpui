//! A native window for cornercase, drawn with GPUI. cornercase's server keeps owning the shells,
//! agents and worktrees; this program is one more client of it, driven through the `cornercase`
//! command line (`view`, `surface`, `input` and friends).

mod app;
mod cli;
mod context_menu;
mod dialogs;
mod keys;
mod model;
mod notify;
mod panels;
mod render;
mod sidebar;
mod store;
mod tabs;
mod terminal;
#[cfg(test)]
mod test_util;
mod theme;
mod ui;

use gpui::{
    App, AppContext, Application, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions, WindowBounds, WindowOptions,
    actions, point, px, size,
};

actions!(cornercase_gpui, [Quit]);

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        ui::text_field::bind_keys(cx);
        cx.set_menus(vec![Menu { name: "cornercase".into(), items: vec![MenuItem::action("Quit cornercase", Quit)] }]);
        cx.on_app_quit(|_| async {
            cli::kill_all();
            let _ = cli::run(&["theme", "--clear"]);
        })
        .detach();
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("cornercase".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(14.), px(13.))),
                }),
                window_min_size: Some(size(px(640.), px(360.))),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| app::Root::new(window, cx)),
        )
        .expect("could not open the window");
        cx.activate(true);
    });
}
