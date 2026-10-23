# Connection

Code: `src/main.rs`, `src/cli.rs`, `src/store.rs`

- **One more client of the command line**: the app never speaks cornercase's socket protocol. Every command, stream and input event goes through the `cornercase` binary (`CORNERCASE_BIN` overrides it), the contract cornercase documents for programs that draw it (#345 upstream). A protocol change upstream then cannot break the window silently.
- **Commands run without the pane variables**: `cli::command` removes `CORNERCASE_PANE` and `CORNERCASE_SERVER`. Launched from inside a cornercase pane, they would make every command default to that pane instead of what the window's view shows.
- **Streams stop when dropped**: `view`, `surface`, `events`, `status --app --follow` and the panels' `--follow` commands are children kept in a global list; dropping their `Stream` kills and reaps the process, and `kill_all` stops the rest when the app quits (then `theme --clear` withdraws the palette the window gave cornercase). A stream's stdout is read with a 1 MiB buffer, since a full `surface` frame can be one long line; stderr is collected on its own thread and handed to `on_end`, which is how a refused or stopped stream reports why.
- **Input and the actions that must follow it go through one ordered queue** (`cli::Queue`): requests on different connections are not ordered by the server, so a key followed by a focus or a split could arrive in the wrong order. One worker thread runs them one after the other.
  - Consecutive input events for the same pane are sent in one `cornercase input` call, at most 256 at a time, so fast typing or a paste does not start a process per key.
  - A command with a `coalesce` key (a drag's selection, a divider being moved, the view's size) only sends the last of a run of jobs with the same key; the replaced ones' replies are dropped.
  - A job that ends a batch or a coalesced run is kept and runs next, so nothing is reordered.
  - Errors become toasts through the queue's `on_error`, unless the job asked for its reply (`cmd_then`).
- **`Store` is a global entity** every part of the window shares, so panels and widgets act without importing the window (`design/code-layout.md`). It holds the session, the view's id, `app` (what `status --app --follow` prints: todos, queue, away, update, keep awake, tour), the docked panel, the sheet, the dialog and the toasts.
  - `act` adds `--view ID` so a command acts in this window's view, not in the TUI's window; `cmd` is for commands that take no view.
  - `follow_app` is called again after a reconnect; the store keeps the stream only for its `Drop`.
  - At most five toasts are kept; each expires by kind (info 4 s, error 8 s, notices 15 s), checked by a timer 16 s after it appears. A toast's action is a button label and the command it runs (`go TOKEN` for an agent's notice).
- **`store::run`, `run_json`, `follow`, `follow_until`**: commands whose answer the UI needs run off the main thread and hand the output back to the entity; fire-and-forget actions go through `Store::act`/`Store::cmd`, which keep their order. A followed command's lines reach the entity until it ends; `on_end` gets what it wrote on stderr (or the spawn error) once, and the caller follows again to resume. Updates stop when the entity is gone.
