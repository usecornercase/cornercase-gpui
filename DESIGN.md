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
- [Remote servers](design/remote.md) (`cli/remote.rs`, `dialogs/remote.rs`): another machine's server over ssh, the shared connection, quoting, streams that stop there, switching servers, what only works on this Mac.
- [Code layout](design/code-layout.md) (`tests/layers.rs`, `clippy.toml`): no comments, where code goes, 1,000 lines per file, small functions, `pub(crate)`.
- [Upstream gaps](design/upstream-gaps.md): no workarounds, filing issues upstream, stopgaps until a fix lands.
- [Notifications](design/notifications.md) (`notify/`): agent notices, toasts, macOS notifications, sounds, bells, updates.
- [Widgets](design/widgets.md) (`ui/`, `theme.rs`, `dialogs.rs`): shared widgets, the text field, dialogs, the look (usecornercase.dev's colours, glass on one ground, rounded shapes and air, cobalt, the mark), motion.
- [Session model](design/session-model.md) (`model.rs`, `model/`): the `view` and `surface` JSON, applying frames and patches.
- [Terminal](design/terminal.md) (`terminal.rs`, `terminal/`, `render.rs`, `render/`, `keys.rs`, `tabs.rs`, `context_menu.rs`): keys and the input method, ⌘ shortcuts, the panes' floating cards, drawing the grid, mouse and selection, moving panes by their handle, tabs, the context menu.
- [Sidebar](design/sidebar.md) (`sidebar.rs`, `sidebar/`): the panel rail, the floating card, rows, drag and drop, menus and inline forms.
- [Pull requests](design/pull-requests.md) (`model/pull_request.rs`, `dialogs/pull_request.rs`, `dialogs/pull_request/`): a workspace's pull request on its row and in its dialog, its checks, conflicts and stack, Ask agent, what waits on #567.
- [Changes panel](design/changes-panel.md) (`panels/changes.rs`, `panels/changes/`): the workspace's diff, staging, discarding, review.
- [Files panel](design/files-panel.md) (`panels/files.rs`, `panels/files/`): the tree, the viewer, search, the ⇧⌘P finder; the one panel that reads the disk.
- [Browser panel](design/browser.md) (`panels/browser.rs`, `panels/browser/`, `assets/browser/`): a headless Chrome drawn in the dock, why not a native web view, its profile and port, tabs per workspace, input, devices, the console, handing elements and screenshots to agents, agents driving it, ports and ssh tunnels.
- [TODO panel](design/todo-panel.md) (`panels/todo.rs`, `panels/todo/`): the TODO list and sending an item to an agent.
- [Queue](design/queue.md) (`panels/queue.rs`, `panels/queue/`): scheduled agent starts, their times and form.
- [Issues](design/issues.md) (`panels/issues.rs`, `panels/issues/`): the issues browser, starting an agent on an issue, the #461 and #472 stopgaps.
- [Accounts](design/accounts.md) (`panels/accounts.rs`, `panels/accounts/`): agent logins and profiles.
- [Usage](design/usage.md) (`panels/usage.rs`, `panels/usage/`): plan limits and stats, the #462 and #472 stopgaps.
- [Flows](design/flows.md) (`panels/flows.rs`, `panels/flows/`): teams of agents, their runs, the plan's diagram and its look, import and start.
- [Settings](design/settings.md) (`panels/settings.rs`, `panels/settings/`): the form built from `config --json`, its layout (navigation, rows that keep their place, dropdowns, the agents table), search, saving, tokens, asking first, export and import.
- [Search](design/search.md) (`panels/search.rs`, `panels/search/`): names and what agents and shells said.
- [Tour](design/tour.md) (`tour.rs`, `tour/`): cornercase's guided tours drawn here, the catalogue, the card, what a step opens and completes.
- [Plugins](design/plugins.md) (`panels/plugins.rs`, `panels/plugins/`): installed plugins, linking and running them.
- [Processes](design/processes.md) (`panels/processes.rs`, `panels/processes/`, `panels/poll.rs`): what every pane runs, CPU and memory, stopping a program, sampling while open.
- [Ports](design/ports.md) (`panels/ports.rs`, `panels/ports/`): the ports a workspace listens on, opening and stopping them.
- [Worktrees](design/worktrees.md) (`panels/worktrees.rs`, `panels/worktrees/`): every worktree on disk, what makes it safe to remove, removing and freeing builds.
