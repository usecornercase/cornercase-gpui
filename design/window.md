# Window

Code: `app.rs`

- **One connection, two streams**: `cornercase start-server --theme -` finds the running server or starts one and gives the terminals our colours (so programs pick a look that matches what we draw); only then does `view` open, and `surface --view` follows the view's id. A lost connection retries every 1.5 s; each new `view` bumps `generation`, so lines from an old stream are dropped.
- **Where the first view starts**: `--tab`, `--workspace` or `--project` on our own command line, else on what cornercase's window shows. Later views (after a reconnect) start on what the server's view settles on.
- **A surface that ends is opened again** after 400 ms: the view lives on (it settles on a neighbour when its tab closes), so the stream is reopened for the same view instead of opening a new view.
- **A window in the background does not count as seeing its tab**: `view-set --hidden` on deactivation and `--visible` on activation, so agents in the shown tab notify while the window is behind others.
- **Keys go back to the terminal** when a dialog or sheet closes: `Root` watches `Store` and refocuses its own handle on that transition, since gpui would otherwise leave the focus on the closed view.
- **Panels are built once** and kept in `Root::panels`, so a closed panel keeps its scroll, filters and fields when it opens again.
- **The bell** in the shown tab is a short flash of the terminal's edge, not a sound: the sound is for agents' notices (`design/notifications.md`).
- **Drags**: `Drag::Program` sends presses, motion and releases to a program that reads the mouse; `Drag::Select` is cornercase's own selection, in cells of the pane; `Drag::Divider` moves a divider of the tab's split.
- **`CORNERCASE_GPUI_OPEN=<panel>`** opens a panel at start, to check it by eye; `CORNERCASE_GPUI_FONT_SIZE` sets the terminal's font size (13 by default).
