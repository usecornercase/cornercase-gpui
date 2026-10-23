# Flows

Code: `src/panels/flows.rs`, `src/panels/flows/`

The Flows sheet shows teams of agents run by an orchestrator (`cornercase flow`): the flows (built in, yours and the project's) and their runs, live. A run shows its team, its plan lit up where the work stands, the messages the team sent each other and the files of its folder; a flow shows its plan, its roles and the actions `cornercase flow` offers.

## Reading

- **Runs follow `flow runs --follow`**; when the stream ends (the server restarted or went away) the panel says why and follows again after the poll interval, rather than leaving the list frozen.
- **Only the selected run's messages are followed** (`flow messages --workspace … --follow --json`), and only while the run is live: a closed run keeps no feed, and the page says to reopen the agents to follow it again. Switching to another run clears which messages were expanded; the same run keeps its messages across a resync.
- **A `dropped` frame means the command fell behind**: the stream is restarted, which replays what the server keeps. Messages are appended only when newer (`seq` counts from 1 per run), so the replay does not duplicate them. The panel keeps the last 256, as the server does.
- **The list and the selected run's files are read again every 3 s while the sheet shows** (`flow list --json`, `flow files --json`); there is no stream for either. The list is read with the shown project, so a project's own flows appear too, and a read in flight is not started twice.
- **After a start, the new run is selected once `flow runs` lists it**: the panel retries every 500 ms for 10 s, since `flow run` answers before the runs stream reports it.
- **Selection falls back** to the first run, else the default flow, else the first flow, whenever what was selected disappears.

## The model

- **A flow's key is origin, name and path**: a project's flow may share a name with yours, so the name alone does not tell listed flows apart.
- **Names compare trimmed and ignoring case**, as cornercase's `store::same_name` does. Duplicating `Quick` gives `Quick 2`, then 3 and on, counting built-in names (reserved even while hidden) and deleted built-ins.
- **Ordering sends each name once**: `flow order` refuses a name twice, and yours and a project's flow of the same name share one place in the order.
- **Stages**: the contract's `back.to` counts stages from 1; run positions and edges (`stage`, `Edge`) count from 0. The orchestrator has no colour slot; roles take the group presets without red (failed) and orange (waiting), in their order.
- **Tags show only the uncommon cases** (default, this project, yours, edited), as the TUI's lists do.
- **A run is `closed`** when its agents are closed (it can be reopened or forgotten) and **ended** once the orchestrator marked its pane for review. "Stop" closes the orchestrator, else any member still open: closing either closes the whole team.
- **Message text is cut at 32 KiB by the server**; `bytes` is the real size, and the page says so when a message shown whole was cut. Collapsed messages show 1,200 characters, four lines.
- **Times are parsed locally** (`epoch`, Howard Hinnant's days-from-civil algorithm, shared with the Usage panel) since cornercase writes UTC stamps and the app takes no date crate for this.

## The diagram

- Drawn as the TUI's Flow panel draws it: the orchestrator on top, apart; stages top to bottom with parallel roles side by side; each loop beside its stage with its label; the end at the bottom.
- For a run it lights up: a working role's box gets a heavy border in its colour, the holder's box is highlighted, the arrow the work last took takes the colour of the role it left, loop rounds count (orange past their limit), a step outside the plan shows dashed, and the end turns blue once reached. A click on a live box goes to its pane.

## Actions

- **Edit opens the flow's TOML in the user's editor.** A built-in is customised first (exported and imported over yours with `--replace`), as editing one in Settings › Flows does. Editing a flow in a form needs a command that saves a flow from a draft: usecornercase/cornercase-terminal#465, which the page names.
- **Share copies the flow's TOML** (`flow export --json`) to paste to a teammate or commit; `flow import -` or Import… adds it.
- **Delete depends on the flow**: a built-in is hidden (it keeps its name, Restore brings it back); your edited built-in is deleted and the built-in hidden; a file is deleted from disk. Everything that cannot be undone (delete, hide over an edit, stop, forget, restoring a built-in over yours) asks first in the panel's own dialog, which runs the command and reads the flows again.
- **Import** takes a file, an https:// address, a GitHub `owner/repo` or the TOML on the clipboard, into yours or the project's, under its name or another, replacing on request. The CLI asks before importing a flow that sets variables or comes from the web, which a window cannot answer, so the dialog says what the question would and passes `--yes`. Showing the flow's own risks first waits on usecornercase/cornercase-terminal#466.
- **Start** works as the TUI's start dialog: the project, the flow (the default one when it can start, else the first built-in that can, else the first that can), the task or an issue, and an optional branch. Changing the project reads that project's flows, keeping the chosen flow by name. The dialog stays open and busy until `flow run` answers, and shows a refusal in red with the task kept.
