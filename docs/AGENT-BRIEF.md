# Brief for agents building cornercase-gpui

cornercase-gpui is a native GPUI (gpui 0.2.2, crates.io) window for cornercase. **It is only a UI**:
cornercase's server owns shells, agents, worktrees and state; this app drives it exclusively through
the `cornercase` command line (installed at /opt/homebrew/bin/cornercase, 0.12.189). The cornercase
source is at `../cornercase-terminal` (read `design/*.md` and `src/` to understand behaviour; never
edit it). Every command's help is in `docs/cornercase-cli.txt`; run `cornercase <cmd> --help` too.

Goal: reach the TUI's features in the GUI, with a polished, consistent look.

## Ground rules

1. **No workarounds.** If cornercase does not expose something your feature needs (no command, no
   field, a bug, a missing `--json`, a stream that does not exist), do not scrape text output,
   read cornercase's private files, or reimplement server logic. Instead open an issue in
   `usecornercase/cornercase-terminal` (see "Issues" below) and make the UI say plainly what is
   missing (e.g. a muted note "needs usecornercase/cornercase-terminal#123"). Exception, by design
   (#345): the Files panel reads the disk itself.
2. Use `--json` output wherever a command has it; parse with `serde_json::Value` or local serde types.
3. Stay in your own files. Each feature has its module (listed in your task). **Do not edit shared
   files** (`src/app.rs`, `src/store.rs`, `src/cli.rs`, `src/ui/*`, `src/main.rs`, `src/model.rs`,
   `src/theme.rs`, `src/render.rs`, `src/panels/mod.rs`, `src/dialogs.rs`) unless your task says so.
   If you truly need a small shared helper, prefer a private one in your module. If you must touch
   a shared file, keep the change minimal and additive and list it in your final report.
4. Match the surrounding code: comment density, naming, idioms; doc comment at top of each module.
   No `unwrap()` on anything that can fail at runtime.
5. Work in your git worktree; commit your work on your branch with clear messages when done
   (`git add -A && git commit`). Do not push. Do not touch other branches.

## Building and running

- First thing in your worktree: reuse the main checkout's build cache so you do not rebuild gpui:
  `cp -Rc /Users/sergiozam/Documents/projects/cornercase-gpui/target ./target` (APFS clone, instant).
- `cargo build` must pass with no warnings from your module; `cargo test` must pass.
- Run the app: `./target/debug/cornercase-gpui --tab 6 & echo $!` — remember the PID and kill only
  that PID when done (other agents run their own copies; never `pkill -f cornercase-gpui`).
  Tab 6 is a zsh tab in project cornercase-terminal; never type into pane 1 (that is the agent
  coordinating you).
- Screenshot to check your UI: bring it to front with
  `osascript -e 'tell application "System Events" to set frontmost of (first process whose unix id is <PID>) to true'`
  then `screencapture -x <scratch>/shot.png` and look at it with the Read tool. Synthetic key
  presses (osascript keystroke) are NOT permitted on this machine; clicks are not either. To test
  interactions, factor logic so it can be unit-tested, and/or temporarily open your panel at start
  (e.g. call `store.open(...)` behind an env var like `CORNERCASE_GPUI_OPEN=changes`) — that env
  var hook already exists in `main.rs`/`app.rs` if listed in your task; otherwise keep any debug
  hook out of the final code.
- Several agents share one cornercase server. Do not do destructive things to real data: do not
  close projects/workspaces/tabs you did not create, do not discard real changes, do not remove
  worktrees you did not create. Create throwaway things (e.g. a temp git repo opened as a project,
  `cornercase open /tmp/...`) to test, and clean them up.

## Architecture (read these files first)

- `src/app.rs` — `Root`, the window: connection (`view`, `surface` streams), layout, and the hosts
  for the docked panel, sheets, dialogs and toasts. `src/terminal.rs`, `src/sidebar.rs`,
  `src/tabs.rs` hold the rest of `Root`.
- `src/store.rs` — `Store`, a global gpui entity (`Store::get(cx)`): `session` (the view's
  snapshot, see `src/model.rs`), `view_id`, `app` (`status --app --follow` JSON), open `dock`/
  `sheet`/`dialog`, `toasts`. Helpers:
  - `Store::act(args)` runs a command **with `--view ID`** (for commands that take a view: focus,
    new-tab, split, start, send, keys, read, wait, mark, rename, open, new-workspace, go…) through
    an ordered queue; `Store::cmd(args)` the same without the view. Errors become toasts.
  - `store::run(cx, args, |this, Result<String>, cx| ..)` / `store::run_json(..)` — run a command
    off the main thread and get its output back on your entity.
  - `store::follow(cx, args, |this, line, cx| ..) -> Option<Stream>` — follow a streaming command
    (one line per change). Keep the `Stream` in your struct; dropping it stops the process.
  - `store.toast(text, ToastKind, cx)`, `open(kind)`, `toggle_dock(kind)`, `close_sheet()`,
    `show_dialog(AnyView)`, `close_dialog()`.
- `src/panels/` — one module per panel; `PanelKind` in `mod.rs`. A panel is a gpui entity
  (`impl Render`) built lazily by `build(window, cx) -> AnyView` and kept alive. Docked panels
  (Changes, Files, TODO) sit right of the terminal (420px); the others are sheets over the window
  (the host draws the title bar and close button; you draw the content, scrolling with
  `.id(..).overflow_y_scroll()`).
- `src/ui/` — widgets: `button`, `primary_button`, `danger_button`, `icon_button`, `badge`,
  `section`, `row`, `note`, `dim`, `field`, `input_box`, `checkbox`, `segment(s)`, `title`, and
  `TextField` (an entity: `cx.new(|cx| TextField::new("placeholder", cx))`, subscribe to
  `TextFieldEvent::{Changed, Submit, Cancel}`, `.text()`, `.set_text()`; focus it with
  `window.focus(&field.read(cx).focus_handle(cx))`).
- `src/dialogs.rs` — `frame(..)` for dialogs, `confirm(title, message, button, command, with_view,
  window, cx)` for anything that cannot be undone, `new_workspace`, `start_agent`.
- `src/theme.rs` — `theme::colours::*` (Tokyo Night chrome colours), `status_badge`.
- Keys: the terminal only takes keys while its own focus handle is focused, so text fields and
  focused panels get their keys. ⌘ shortcuts live in `terminal.rs::shortcut`.
- gpui 0.2.2 source: `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/gpui-0.2.2/`
  (examples/ are useful). Edition 2024: helper fns returning `impl IntoElement` from `&self`
  methods need `+ use<>` if they must not capture lifetimes.

## Issues in cornercase-terminal

Only for **generic** gaps or bugs (something any client would hit), not for things specific to
this app. Before creating one: `gh issue list --repo usecornercase/cornercase-terminal --state all
--search "<keywords>"` — other agents work in parallel and may have filed it; if so, reuse it.
Follow the repo's templates in `../cornercase-terminal/.github/ISSUE_TEMPLATE/`:
- bug → `gh issue create --label bug` with sections `### What happened`, `### Steps to reproduce`,
  `### Commit` (090b152f, installed release 0.12.189), `### System` (macOS 26.6.2, arm64),
  `### Outer terminal` (none: a GUI client driving cornercase through its CLI), `### server.log`
  (relevant lines of `cornercase logs`, no tokens/private paths), `### Anything else`.
- feature → `--label enhancement` with `### Problem`, `### Idea`, `### Alternatives`. Phrase the
  idea in cornercase's terms (a command, a `--json` field, a stream), reference #345 (external UIs).
Write them in English, concrete, with a reproduction from the shell. Known already: #461 (`--view`
breaks feature checks: `surface --view --patches`, `view-set --hidden/--visible`).

## Final report

End with: what you built (with a screenshot path), how you verified it, any shared files you
touched and why, the issues you opened (URLs) and what in the UI waits on them, and what is left.
