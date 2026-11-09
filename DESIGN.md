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
- [Releases](design/releases.md) (`.github/workflows/release.yml`, `.github/workflows/version.yml`, `.github/release/`, `tools/macos/`, `tools/icon.py`, `assets/macos/`, `.cargo/config.toml`, `CHANGELOG.md`): what each platform's download is and how it is built, the macOS bundle, its signature and notarization, and its icon, Windows' C runtime, the Linux build image, checksums, licences and notices, the notes, releasing every merge that changes the app, what waits on signing and packages.
- [Website](design/website.md) (`site/`, `.github/workflows/pages.yml`): the landing page, its GPU field, the window drawn in the page, the scroll story, real captures, GitHub Pages and the domain.
- [Linux](design/linux.md) (`Cargo.toml`, `platform.rs`, `assets/linux/`, `tools/linux/`): the second platform, platform code as plain logic, building and running it, checking it from a Mac in Docker.
- [Windows](design/windows.md) (`Cargo.toml`, `build.rs`, `platform.rs`, the `win32.rs` modules, `assets/windows/`, `tools/icon.py`, `.cargo/config.toml`): the third platform, a GUI program whose children get no console window, Win32 calls beside the logic that takes the platform, building and checking it from a Mac, CI, what was checked on Windows.
- [Updating the window](design/self-update.md) (`self_update.rs`, `self_update/`, `main.rs`'s `--version` and quit path, `release.yml`'s `CORNERCASE_GPUI_RELEASE`): the check against GitHub's releases, the chip and dialog, installing in place on Windows and Linux, the relaunch, what waits on signing and packages.
- [Window](design/window.md) (`app.rs`, `app/frame.rs`, `app/keeper.rs`, `app/placement.rs`, `app/widths.rs`): the connection's two streams, reconnecting, visibility, focus, panels kept alive, the layout kept across launches.
- [Connection](design/connection.md) (`main.rs`, `cli.rs`, `store.rs`): the command line as the only contract, streams, the ordered queue, `Store`.
- [Remote servers](design/remote.md) (`cli/remote.rs`, `dialogs/remote.rs`, `store/machine_read.rs`, `panels/on_machine.rs`): another machine's server over ssh, the shared connection, quoting, streams that stop there, switching servers, what only works on this Mac, what Changes and Files read on a machine.
- [Code layout](design/code-layout.md) (`tests/layers.rs`, `clippy.toml`): no comments, where code goes, 1,000 lines per file, small functions, `pub(crate)`.
- [Upstream gaps](design/upstream-gaps.md): no workarounds, filing issues upstream, where things stand, what the window cannot do because cornercase lacks it, stopgaps until a fix lands.
- [Notifications](design/notifications.md) (`notify/`): agent notices, toasts, macOS notifications, sounds, bells, updates.
- [Theme](design/theme.md) (`theme.rs`, `theme/`, `appearance.rs`, `appearance/`, `app/looks.rs`, `panels/settings/appearance.rs`, `panels/settings/appearance/`, `ui/colour_picker.rs`, `ui/slider.rs`, `ui/track.rs`, `assets/themes/`): what a theme holds, its files beside the settings, presets and their sources, `extends` and derived colours, reading it without a `cx`, giving it to cornercase, following the system, the Appearance page, contrast, live reload, the limits of glass, what waits.
- [Widgets](design/widgets.md) (`ui/`, `theme.rs`, `dialogs.rs`): shared widgets, the text field, selecting in a text that is no field, Markdown, dialogs, the look (usecornercase.dev's colours, glass on one ground, rounded shapes and air, cobalt, the mark), motion.
- [Session model](design/session-model.md) (`model.rs`, `model/`): the `view` and `surface` JSON, applying frames and patches.
- [Terminal](design/terminal.md) (`terminal.rs`, `terminal/`, `render.rs`, `render/`, `keys.rs`, `tabs.rs`, `context_menu.rs`): keys and the input method, ⌘ shortcuts, the panes' floating cards, drawing the grid, mouse and selection, moving panes by their handle, tabs, the context menu.
- [Sidebar](design/sidebar.md) (`sidebar.rs`, `sidebar/`): the panel rail, the floating card, rows, drag and drop, menus and inline forms.
- [Pull requests](design/pull-requests.md) (`model/pull_request.rs`, `dialogs/pull_request.rs`, `dialogs/pull_request/`): a workspace's pull request on its row and in its dialog, its checks, conflicts and stack, Ask agent, how soon a merge shows.
- [Context dialog](design/context-dialog.md) (`model/agent_context.rs`, `dialogs/context.rs`, `dialogs/context/`): what fills an agent's context, reading it without touching the agent, the bar and its lists, other agents.
- [Shared files and the merge check](design/shared-files.md) (`model/shared.rs`, `dialogs/merge_check.rs`, `dialogs/merge_check/`): `⇄` on a workspace's row and a file of the Changes panel, what they say, the dialog that checks a workspace's merges, Ask agent.
- [Changes panel](design/changes-panel.md) (`panels/changes.rs`, `panels/changes/`): the workspace's diff, staging, discarding, review, selecting lines and characters, a machine's workspace.
- [Files panel](design/files-panel.md) (`panels/files.rs`, `panels/files/`, `ui/selection/code.rs`, `app/dock.rs`): the tree, git marks, the viewer, long lines, full screen, selecting and finding in it, search, the ⇧⌘P finder, a machine's workspace; the one panel that reads the disk.
- [Browser panel](design/browser.md) (`panels/browser.rs`, `panels/browser/`, `assets/browser/`): a headless Chrome drawn in the dock, why not a native web view, its profile and port, tabs per workspace, input, devices, the console, handing elements and screenshots to agents, agents driving it, ports and ssh tunnels.
- [TODO panel](design/todo-panel.md) (`panels/todo.rs`, `panels/todo/`): the TODO list and sending an item to an agent.
- [Recap](design/recap.md) (`panels/recap.rs`, `panels/recap/`): an agent's summary of what your agents did, its three ranges, a run beside the queue, naming ids, what a link goes to.
- [Queue](design/queue.md) (`panels/queue.rs`, `panels/queue/`): scheduled agent starts, their times and form, routines and their runs.
- [Issues](design/issues.md) (`panels/issues.rs`, `panels/issues/`): the issues browser, starting an agent on an issue, a partial answer from the trackers.
- [Accounts](design/accounts.md) (`panels/accounts.rs`, `panels/accounts/`): agent logins and profiles.
- [Machines](design/machines.md) (`model/machines.rs`, `model/machines/`, `panels/machines.rs`, `panels/machines/`, `store/machines_there.rs`, `store/run_on.rs`, `dialogs/run_on.rs`, `sidebar/machine_rows.rs`, `sidebar/machine_acts.rs`, `terminal/machine_card.rs`, `terminal/machine_menu.rs`, `dialogs/closing.rs`, `ui/counting.rs`, `app/machine.rs`, `app/connection.rs`): the computers the server links to, the list in `Store`, each link's state in words and its count to the next try, adding, editing, removing, retrying and updating them, why ⇧⌘K is another thing; a machine's workspaces here: ids of another server kept apart, acting through `--remote`, the picked place and its card, what part 3 fills in; a machine's tab drawn live on a connection of its own: its streams, keys, losing and regaining it, its size and colours, closing its panes and tabs, its pane menu.
- [Usage](design/usage.md) (`panels/usage.rs`, `panels/usage/`): plan limits and stats, of this computer, one machine or all of them, a partial answer.
- [Flows](design/flows.md) (`panels/flows.rs`, `panels/flows/`): teams of agents, their runs, the plan's diagram and its look, import and start.
- [Settings](design/settings.md) (`panels/settings.rs`, `panels/settings/`, `dialogs/pairing.rs`, `dialogs/pairing/`, `ui/qr.rs`): the form built from `config --json`, its layout (navigation, rows that keep their place, dropdowns, the agents table), search, landing on a row from the global search, saving, tokens, pairing a phone and its QR code, the Discord forum's server, asking first, export and import.
- [Search](design/search.md) (`panels/search.rs`, `panels/search/`): names, settings and what agents and shells said.
- [Tour](design/tour.md) (`tour.rs`, `tour/`): cornercase's guided tours drawn here, the catalogue, the card, what a step opens and completes.
- [Plugins](design/plugins.md) (`panels/plugins.rs`, `panels/plugins/`): installed plugins, linking and running them.
- [Processes](design/processes.md) (`panels/processes.rs`, `panels/processes/`, `panels/poll.rs`): what every pane runs, CPU and memory, stopping a program, sampling while open.
- [Ports](design/ports.md) (`panels/ports.rs`, `panels/ports/`): the ports a workspace listens on, opening and stopping them.
- [Worktrees](design/worktrees.md) (`panels/worktrees.rs`, `panels/worktrees/`): every worktree on disk, what makes it safe to remove, removing and freeing builds, the sidebar's Remove worktree….
