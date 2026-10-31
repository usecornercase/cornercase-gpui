# cornercase-gpui

A native macOS window for [cornercase](../cornercase-terminal), drawn with GPUI. It is only a UI:
cornercase's server keeps owning the shells, agents and worktrees, and this app drives it through
the `cornercase` command line, the contract cornercase documents for programs that draw it. The Browser panel
is the one part that runs programs of its own: a headless Chrome it draws, and ssh tunnels to another machine's
dev servers (`design/browser.md`).

| What | cornercase command |
| --- | --- |
| Session (sidebar, tabs, agent states) | `cornercase view --cols C --rows R` (one JSON line per change) |
| Terminal cells, cursor, splits | `cornercase surface --view ID [--patches]` |
| Window resize | `cornercase view-set --view ID --cols C --rows R` |
| Typing, paste, mouse for programs | `cornercase input --follow --json -` (ordered queue, one process) |
| The ordered queue's commands | `cornercase requests --follow -` (one process, a command per line) |
| Wheel over history, selection, ⌘C | `cornercase scroll`, `cornercase selection [--copy]` |
| Dragging a divider | `cornercase ratio --tab T --path …` |
| Dragging a pane onto another | `cornercase move-pane --pane P --target T --place …` |
| Tabs, splits, projects, workspaces, agents | `focus`, `new-tab`, `split`, `close`, `open`, `new-workspace`, `start` (all with `--view`) |
| Colours programs see | `cornercase start-server --theme -` (cleared with `theme --clear` on quit) |
| Hovering, clicking or ⌘-clicking a path | `cornercase link --pane P --cell X,Y --json` |
| Processes, ports, worktrees | `processes`, `ports`, `worktrees list/free-builds` (`--json`), sampled while their sheet is open |
| The Browser panel's dev servers | `cornercase ports --workspace W --json`, sampled while the panel shows |
| An element, a screenshot or the console's errors to an agent | `cornercase paste-file --pane P FILE`, an `input` paste, `cornercase send --pane P --enter` |
| Another machine's server | every command as `cornercase --remote DESTINATION …` |

## Run

```sh
cargo run --release                 # opens on what cornercase's window shows
cargo run --release -- --tab 6      # or on a tab, workspace or project id (`cornercase status`)
cargo run --release -- --remote devbox                   # or on the server of another machine, over ssh
cargo run --release -- --remote me@10.0.0.5 --remote-command '~/bin/cornercase'
```

Needs `cornercase` 0.12.234 or newer on the PATH (or `CORNERCASE_BIN`). It starts the server if none runs.
With `--remote`, it runs `cornercase` on that machine through your own `ssh` (config, keys, agent and jump hosts apply), as
`cornercase remote` does, and starts the server there; it never asks for a password, so use a key or an agent.
`CORNERCASE_GPUI_FONT_SIZE` sets the terminal font size (Menlo, 13 by default), and `CORNERCASE_GPUI_OPACITY`
the opacity of its background (0.65 by default; 1 for an opaque terminal; no lower than the window's own 0.55).
The Browser panel needs Chrome, Chromium, Edge or Brave (`CORNERCASE_GPUI_CHROME` names another binary); it runs
it headless on a profile of its own (`CORNERCASE_GPUI_BROWSER_PROFILE` moves it) and on port 9222 when free.

## What it has

- **Terminal**: live panes from `surface`, each on a floating card; splits (drag the gap between cards, double-click to even), selection
  (drag, double/triple-click), ⌘C/⌘V, wheel, mouse for programs that read it, ⌘-click links and paths,
  right-click pane menu (split, move, close agent, mark); drag a pane by the handle that shows near its top onto
  another to swap them or put it on a side, with the spot lit, a live miniature under the pointer and the panes
  sliding to their new places (a right-button drag moves it too, drawn as it lands).
- **Sidebar**: groups (icon, colour, fold), projects, workspaces (worktree, PR and checks, behind,
  setup, changes), agents as a tree with status/model/context, inline rename, drag to reorder,
  context menus, remove-worktree check, closing a workspace's agents to reopen them later.
  It follows cornercase's Settings → UI: the four arrangements of `sidebar` (one tree, projects on top, workspaces
  on top, side by side), `tabs` (under their workspace, or in the bar above the panes) and `agents_section` (every
  running agent of every project in one list). The line between stacked lists and the one above the agents section drag.
- **Paths in panes**: a path a program printed (`src/main.rs:12`, `./shot.png`, `~/Movies/demo.mov`) is
  underlined under the pointer, and resting there shows the file in a card beside it: code coloured at its lines,
  pictures (GIFs move), videos playing without sound, sounds with a play button, and PDFs, documents and anything
  else Quick Look can draw. A click opens it in the Files panel at its lines (a click on a code line in the card,
  at that line); ⌘-click opens it in its default app. On another machine the card shows text and pictures.
- **Tabs**: floating pills; rename, reorder, close with confirmation, new shell/agent menu.
- **Search** (⌘F): everything open, groups, projects, workspaces, tabs and agents, listed in the sidebar's order
  from what the window shows, then found as you type by name, branch, folder, pull request, agent kind or model;
  ↩ goes there, an agent's pane included. ⇥ searches what agents and shells said instead.
- **Panel rail**: a column of icons left of the sidebar opens every panel; hover one for its keys.
- **Resizing**: drag the border of the sidebar, the docked panel, the lists of Issues, Flows and Settings, the
  queue's form or the Changes file list; a double click on it restores the default.
- **Docked panels**: Changes (⌘⇧C), Files (⌘⇧E), TODO (⌘⇧L), Browser (⇧⌘B).
- **Browser** (⇧⌘B, the rail's globe): a real Chrome drawn in the dock, beside the agent changing the page. It
  shows the workspace's dev servers and opens a new one on its own; each workspace keeps its own tabs. Toolbar:
  back, forward, reload or stop, the address (a bare port such as `5173` opens on localhost; a click selects it),
  open in the system browser, devices (Responsive, iPhone 15, Pixel 8, iPad Air, Laptop, Desktop, rotated),
  new tab; tabs show from two, a middle click closes one. Pointer, keys, the input method, ⌘C/⌘X/⌘V and
  `<select>` menus work as in Chrome. Pick an element (the pointer button) or take a screenshot (the camera)
  and hand it to the workspace's agent, with what should change; pages reload when an agent finishes. The
  agents menu shows the DevTools address and tells an agent how to drive the same Chrome
  (`chrome-devtools-mcp --browser-url`, Playwright's `connectOverCDP`), whose tabs show here. On another machine
  (`--remote`) its ports open through ssh tunnels and agents there reach this browser through a reverse tunnel;
  the Ports sheet's Open shows a port in the panel. The console button (with the page's error count) opens a
  drawer of console messages, uncaught errors and failed requests, and sends them to the agent. A page's alerts,
  confirms and prompts show as cards, file inputs open the system's picker, downloads go to `~/Downloads`, and a
  crashed page offers Reload.
- **Go to a file** (⇧⌘P): the shown workspace's files ranked as you type, as editors' quick open; ↑ ↓ and ↩
  (or a click) open one in the Files panel, revealed in its tree. It works on another machine too.
- **Sheets**: Search (⌘F), Issues (⌘I), Queue (⇧⌘S), Flows (⇧⌘G), Usage (⌘U), Accounts (⇧⌘A), Processes (⇧⌘M),
  Ports (⇧⌘O), Worktrees (⇧⌘T), Plugins (⇧⌘X), Settings (⌘,). The same keys close them.
- **Remote servers**: ⇧⌘K, or a click on the server's name in the status bar, connects the window to cornercase
  on another machine (a host from `~/.ssh/config`, or `user@host`) through `cornercase --remote`, and back to this
  Mac; it reconnects by itself when the link drops. The Files panel lists, reads and searches the files there
  through `cornercase files`; images and files pasted or dropped on a pane go there with `paste-file`. Open in
  Finder only works on this Mac, ⌘-click on a path copies it, and Plugins there only show and run.
- **Plugins**: install and update with a preview of what they run, services' states, actions, and popups drawn
  over the terminal that take the keys while open.
- **Flows**: new and edited flows, checked as you type and saved with `flow save`; imports checked first.
- **Queue**: agents that start later (at a time, in a while, when a plan resets) on a prompt or an issue, in every
  project: what did not start first, then what is scheduled and what started; a click edits an entry or shows its
  pane. Empty, it says what it is for.
- **Settings** (⌘,): cornercase's settings page by page, or found by typing, since the search field takes the
  keys as it opens. Each change is saved at once, tokens are checked as they are sent, and dangerous modes and
  removals ask first. Projects holds each repository's setup and teardown commands; Share exports your setup and
  shows what a colleague's would change before importing it.
- **Tours**: the rail's "?" lists cornercase's guided tours and runs one as a card over the window.
- **Processes, ports, worktrees**: what every pane runs and its CPU and memory, filtered and sorted from the table's
  header, busy processes never folded away; the ports a workspace listens on;
  every worktree on disk with what makes it safe to remove, its size and its builds to free.
- **App-wide**: agent notices as toasts with "Go" and macOS notifications, sounds, OSC 52 clipboard,
  bell, window title, update/restart, away, keep awake.

## Keys

Everything goes to the program in the active pane, except ⌘ shortcuts; ⌘/ (or the keyboard icon in the
rail) lists them all:
⌘C copy selection · ⌘V paste (an image as a file the agent can open) · ⌘T new tab · ⌘W close the dialog, sheet or panel on top, else the pane ·
⇧⌘W close the tab · ⌘D / ⇧⌘D split right / down · ⌘[ / ⌘] previous / next pane · ⌘1…9 tab (⌘9 the
last) · ⇧⌘[ / ⇧⌘] previous / next tab · ⌘O open project · ⌘N new workspace · ⌘G start an agent ·
⇧⌘P go to a file · ⇧⌘K connect to another server · ⌘+ / ⌘- / ⌘0 zoom · ⌘H hide · ⌘M minimise · ⌃⌘F full screen · ⌘Q quit.
While the Browser has the focus: ⌘L go to the address · ⌘R reload · ⌘[ / ⌘] back / forward · ⌘C / ⌘X / ⌘V
copy, cut and paste on the page; every other key, Escape included, goes to the page.

Esc closes the menu, dialog or sheet on top (a click outside does too), or brings the keys back to the
terminal from a panel; in a search field it first clears the text. Tab and ⇧Tab move between a dialog's
fields, and ↑ ↓ ↩ choose in a menu. Text goes through macOS's input method, so dead-key accents and ⌥
characters (`@` on a Spanish keyboard) work. While a plugin's popup is open every key, Escape included, goes
to it, and ⌘W closes it. Dropping files on a pane pastes their paths (on another machine, the files themselves).

## Waiting on cornercase

Gaps found while building this are filed upstream instead of worked around, and the UI names the issue where a
feature waits. Every one filed so far has landed (#456, #461–#472, #475–#480) except #549. Two limits wait on
commands cornercase does not have yet, and the UI says so: removing the worktrees of closed projects and the
leftovers git no longer knows, and installing, turning on or off and reading the logs of plugins on another machine.

### To remove once these land

- **#549:** showing a fold before the session confirms it (`src/sidebar/folds.rs`, `Folds` in `SideState` and `Msg::Folded` in `src/app.rs`); `set_folded` goes back to queueing the command alone.

## Development

The rules for every change are in [AGENTS.md](AGENTS.md) (also loaded by Claude Code through `CLAUDE.md`), the design decisions and their reasons in [`design/`](design/), indexed by [DESIGN.md](DESIGN.md). `docs/cornercase-cli.txt` is every cornercase command's help. Before a change is done:

```sh
cargo fmt
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --locked
cargo-machete              # cargo install cargo-machete
npx -y jscpd@4.3.0         # copy-paste detector
```
