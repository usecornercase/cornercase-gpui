mod app;
mod cli;
mod context_menu;
mod dialogs;
mod keymap;
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
mod tour;
mod ui;

use gpui::{
    App, AppContext, Application, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions, Window,
    WindowBackgroundAppearance, WindowBounds, WindowOptions, actions, point, px, size,
};

actions!(cornercase_gpui, [Quit, Hide, HideOthers, ShowAll, Minimise, Zoom, FullScreen]);

fn on_window(cx: &mut App, act: fn(&Window)) {
    if let Some(window) = cx.active_window() {
        let _ = window.update(cx, |_, window, _| act(window));
    }
}

fn system_keys(cx: &mut App) {
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &Hide, cx| cx.hide());
    cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
    cx.on_action(|_: &Minimise, cx| on_window(cx, Window::minimize_window));
    cx.on_action(|_: &Zoom, cx| on_window(cx, Window::zoom_window));
    cx.on_action(|_: &FullScreen, cx| on_window(cx, Window::toggle_fullscreen));
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("alt-cmd-h", HideOthers, None),
        KeyBinding::new("cmd-m", Minimise, None),
        KeyBinding::new("ctrl-cmd-f", FullScreen, None),
    ]);
    cx.set_menus(vec![
        Menu {
            name: "cornercase".into(),
            items: vec![
                MenuItem::action("Hide cornercase", Hide),
                MenuItem::action("Hide Others", HideOthers),
                MenuItem::action("Show All", ShowAll),
                MenuItem::separator(),
                MenuItem::action("Quit cornercase", Quit),
            ],
        },
        Menu {
            name: "Window".into(),
            items: vec![
                MenuItem::action("Minimize", Minimise),
                MenuItem::action("Zoom", Zoom),
                MenuItem::separator(),
                MenuItem::action("Enter Full Screen", FullScreen),
            ],
        },
    ]);
}

fn main() {
    let target = cli::Target::from_args(&std::env::args().collect::<Vec<_>>()).unwrap_or_else(|e| {
        eprintln!("cornercase-gpui: {e}");
        std::process::exit(2)
    });
    cli::connect(target);
    Application::new().with_assets(ui::icons::Assets).run(|cx: &mut App| {
        system_keys(cx);
        ui::text_field::bind_keys(cx);
        cx.on_app_quit(|_| async {
            panels::browser::quit();
            cli::quit();
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
                    traffic_light_position: Some(point(px(16.), px(16.))),
                }),
                window_background: WindowBackgroundAppearance::Blurred,
                window_min_size: Some(size(px(640.), px(360.))),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| app::Root::new(window, cx)),
        )
        .expect("could not open the window");
        cx.activate(true);
    });
}
