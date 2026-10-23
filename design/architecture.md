# Architecture

Code: `src/`

## The window and the connection

- `main.rs` starts the app, binds ⌘Q and the text fields' keys, and opens the window on `Root`.
- `app.rs` holds `Root`, the window: the connection (the `view` and `surface` streams), the layout, and the hosts for the docked panel, sheets, dialogs and toasts. `terminal.rs` (keys, ⌘ shortcuts, the surface; `terminal/` holds `mouse`, `selection`, `pane_menu` and `input`), `sidebar.rs` (the sidebar's frame and filter; `sidebar/` holds `rows`, `workspace` (workspace and agent rows), `menus`, `actions`, `forms` with `forms/style` and `forms/checked`, and `order` for drag ordering), `tabs.rs` (the tab bar) and `context_menu.rs` hold the rest of `Root`'s methods.
- `cli.rs` is everything that talks to cornercase: one-shot commands (`run`), long-running streams (`stream`, a `Stream` stops its process when dropped) and an ordered `Queue` for input. The window never speaks the socket protocol itself.
- `model.rs` holds the serde types of `view` (the session: projects, workspaces, tabs, agents); `model/surface.rs` those of `surface` (frames, patches), and `model/grid.rs` the grid they build up.
- `render.rs` draws a grid of cells with `Metrics` measured from the font. `keys.rs` encodes gpui keystrokes for `cornercase input`. `theme.rs` holds the terminal palette (also given to cornercase with `start-server --theme`), the chrome colours (`theme::colours`, Tokyo Night) and `FONT_FAMILY`.
- `notify.rs` follows `cornercase events` (`notify/feed.rs` parses them): agent notices as toasts with a "Go" button, macOS notifications, sounds, OSC 52 copies, the bell and the window title. `notify/update.rs` offers updates and restarts.

## Store

`store.rs` holds `Store`, a global gpui entity (`Store::get(cx)`) every part of the window shares: `session` (the view's snapshot), `view_id`, `app` (what `status --app --follow` prints), the open `dock`, `sheet` and `dialog`, and the `toasts`.

- `Store::act(args)` runs a command **with `--view ID`** (focus, new-tab, split, start, send, keys, read, wait, mark, rename, open, new-workspace, go…) through the ordered queue; `Store::cmd(args)` does the same without the view. Errors become toasts.
- `store::run(cx, args, |this, Result<String>, cx| …)` and `store::run_json(…)` run a command off the main thread and give its output back to the entity.
- `store::follow(cx, args, |this, line, cx| …) -> Option<Stream>` follows a streaming command, one line per change. The entity keeps the `Stream`; dropping it stops the process.
- `toast(text, ToastKind, cx)`, `open(kind)`, `toggle_dock(kind)`, `close_sheet()`, `show_dialog(AnyView)`, `close_dialog()`.

## Panels

`src/panels/` has one module per panel, and `PanelKind` in `mod.rs`. A panel is a gpui entity (`impl Render`) built lazily by `build(window, cx) -> AnyView` and kept alive by `Root`, so it keeps its state while closed. Docked panels (Changes, Files, TODO) sit right of the terminal (420 px); the others are sheets over the window: the host draws the title bar and the close button, the panel draws its content and scrolls it with `.id(…).overflow_y_scroll()`. A panel's file keeps its struct, its state changes and `render`; its folder holds the model and parsing (`data.rs`, `model.rs`, `wire.rs`), the commands it runs (`actions.rs`, `load.rs`, `follow.rs`) and the named parts of its drawing (`*_view.rs`, `list.rs`, `detail.rs`, `widgets.rs`).

## Widgets

- `ui/mod.rs`: `button`, `primary_button`, `danger_button`, `icon_button`, `badge`, `section`, `row`, `note`, `dim`, `field`, `input_box`, `checkbox`, `segment(s)`, `title`.
- `ui/text_field.rs`: `TextField`, an entity (`cx.new(|cx| TextField::new("placeholder", cx))`) that emits `TextFieldEvent::{Changed, Submit, Cancel}`, with `text()` and `set_text()`; focus it with `window.focus(&field.read(cx).focus_handle(cx))`.
- `dialogs.rs`: `frame(…)` for dialogs, `confirm(…)` for anything that cannot be undone, `new_workspace`, `start_agent`.

gpui 0.2.2's sources and examples are in `~/.cargo/registry/src/index.crates.io-*/gpui-0.2.2/`.
