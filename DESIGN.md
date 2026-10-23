# Design decisions

Why cornercase-gpui works the way it does, area by area. The rules every change follows are in [AGENTS.md](AGENTS.md), including when to read and update these notes. This file is their index: the notes are in [`design/`](design/), one file per area.

## How to use it

- Each line below gives an area's file, the code it covers and what it decides. Paths are under `src/` unless they name another folder; `#N` upstream is an issue of `usecornercase/cornercase-terminal`.
- A new area gets its file and a line here.

## What goes in an area file

- What was decided and why, the alternatives that were rejected and why, limits that are not obvious, what waits upstream, and what was or was not checked live, with the versions.
- Not how the change was built, raw measurements (keep the conclusion and its date), or names and values the code already states.

## Areas

- [Architecture](design/architecture.md): what each file of `src/` holds, `Store`, panels, widgets.
- [Window](design/window.md) (`app.rs`): the connection's two streams, reconnecting, visibility, focus, panels kept alive.
- [Connection](design/connection.md) (`main.rs`, `cli.rs`, `store.rs`): the command line as the only contract, streams, the ordered queue, `Store`.
- [Code layout](design/code-layout.md) (`tests/layers.rs`, `clippy.toml`): no comments, where code goes, 1,000 lines per file, small functions, `pub(crate)`.
- [Upstream gaps](design/upstream-gaps.md): no workarounds, filing issues upstream, stopgaps until a fix lands.
- [Notifications](design/notifications.md) (`notify/`): agent notices, toasts, macOS notifications, sounds, bells, updates.
- [Widgets](design/widgets.md) (`ui/`, `theme.rs`, `dialogs.rs`): shared widgets, the text field, dialogs, the palette.
- [Session model](design/session-model.md) (`model.rs`, `model/`): the `view` and `surface` JSON, applying frames and patches.
- [Terminal](design/terminal.md) (`terminal.rs`, `terminal/`, `render.rs`, `keys.rs`, `tabs.rs`, `context_menu.rs`): keys and the input method, ⌘ shortcuts, drawing the grid, mouse and selection, tabs, the context menu.
- [Sidebar](design/sidebar.md) (`sidebar.rs`, `sidebar/`): rows, drag and drop, the quick filter, menus and inline forms.
- [Changes panel](design/changes-panel.md) (`panels/changes.rs`, `panels/changes/`): the workspace's diff, staging, discarding, review.
- [Files panel](design/files-panel.md) (`panels/files.rs`, `panels/files/`): the tree, the viewer, search; the one panel that reads the disk.
- [TODO panel](design/todo-panel.md) (`panels/todo.rs`, `panels/todo/`): the TODO list and sending an item to an agent.
- [Queue](design/queue.md) (`panels/queue.rs`, `panels/queue/`): scheduled agent starts, their times and form.
- [Issues](design/issues.md) (`panels/issues.rs`, `panels/issues/`): the issues browser, starting an agent on an issue, the #461 and #472 stopgaps.
- [Accounts](design/accounts.md) (`panels/accounts.rs`, `panels/accounts/`): agent logins and profiles.
- [Usage](design/usage.md) (`panels/usage.rs`, `panels/usage/`): plan limits and stats, the #462 and #472 stopgaps.
- [Flows](design/flows.md) (`panels/flows.rs`, `panels/flows/`): teams of agents, their runs, import and start.
- [Settings](design/settings.md) (`panels/settings.rs`, `panels/settings/`): the form built from `config --json`, saving, tokens, export.
- [Search](design/search.md) (`panels/search.rs`, `panels/search/`): names and what agents and shells said.
- [Plugins](design/plugins.md) (`panels/plugins.rs`, `panels/plugins/`): installed plugins, linking and running them.
