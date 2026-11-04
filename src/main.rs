#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]

mod app;
mod cli;
mod context_menu;
mod desktop;
mod dialogs;
mod keymap;
mod keys;
mod model;
mod notify;
mod panels;
mod platform;
mod prefs;
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
    App, AppContext, Application, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions, Window, WindowBounds,
    WindowOptions, actions, point, px,
};

use crate::platform::Os;

actions!(cornercase_gpui, [Quit, Hide, HideOthers, ShowAll, Minimise, Zoom, FullScreen]);

const SHOWN: (f32, f32) = (1280., 800.);
const LEAST: (f32, f32) = (640., 360.);

fn on_window(cx: &mut App, act: fn(&mut Window, &mut App)) {
    if let Some(window) = cx.active_window() {
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| act(window, cx));
        });
    }
}

fn system_bindings(os: Os) -> Vec<KeyBinding> {
    match os {
        Os::Mac => vec![
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-h", Hide, None),
            KeyBinding::new("alt-cmd-h", HideOthers, None),
            KeyBinding::new("cmd-m", Minimise, None),
            KeyBinding::new("ctrl-cmd-f", FullScreen, None),
        ],
        Os::Linux => vec![
            KeyBinding::new("ctrl-shift-q", Quit, None),
            KeyBinding::new("ctrl-shift-m", Minimise, None),
            KeyBinding::new("f11", FullScreen, None),
        ],
        Os::Windows => vec![KeyBinding::new("ctrl-shift-m", Minimise, None), KeyBinding::new("f11", FullScreen, None)],
    }
}

fn system_keys(cx: &mut App) {
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &Hide, cx| cx.hide());
    cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
    cx.on_action(|_: &Minimise, cx| on_window(cx, |window, _| window.minimize_window()));
    cx.on_action(|_: &Zoom, cx| on_window(cx, |window, _| window.zoom_window()));
    cx.on_action(|_: &FullScreen, cx| on_window(cx, app::frame::toggle_full_screen));
    cx.bind_keys(system_bindings(Os::CURRENT));
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

fn open_window(cx: &mut App) {
    let screen = cx.primary_display().map(|display| display.bounds().size);
    let shown = app::frame::opening_size(SHOWN, LEAST, screen);
    let bounds = Bounds::centered(None, app::frame::outer(Os::CURRENT, shown), cx);
    let wayland = cx.compositor_name() == "Wayland";
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("cornercase".into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(16.), px(16.))),
            }),
            window_background: app::frame::background(Os::CURRENT),
            window_decorations: app::frame::decorations(Os::CURRENT),
            app_id: Some(app::frame::APP_ID.into()),
            window_min_size: Some(app::frame::min_size(Os::CURRENT, wayland, LEAST)),
            ..Default::default()
        },
        move |window, cx| {
            app::frame::fit_to_frame(Os::CURRENT, wayland, window);
            cx.new(|cx| app::Root::new(window, cx))
        },
    );
    if let Err(why) = opened {
        fail(&format!("could not open the window: {why:#}"), 1);
    }
}

fn fail(why: &str, code: i32) -> ! {
    eprintln!("cornercase-gpui: {why}");
    alert(why);
    std::process::exit(code)
}

#[cfg(windows)]
fn alert(why: &str) {
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
    use windows::core::HSTRING;
    unsafe { MessageBoxW(None, &HSTRING::from(why), &HSTRING::from("cornercase"), MB_OK | MB_ICONERROR) };
}

#[cfg(not(windows))]
fn alert(_: &str) {}

fn main() {
    let target =
        cli::Target::from_args(&std::env::args().collect::<Vec<_>>()).unwrap_or_else(|e| fail(&e.to_string(), 2));
    cli::connect(target);
    let app = Application::new().with_assets(ui::icons::Assets);
    if let Err(why) = theme::settle_fonts(&app.text_system()) {
        fail(why, 1);
    }
    app.run(|cx: &mut App| {
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

        open_window(cx);
        cx.activate(true);
    });
}
