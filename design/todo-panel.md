# TODO panel

Code: `src/panels/todo.rs`, `src/panels/todo/`

- **One global list every client shares**, docked beside the terminal: items come live from `status --app` (its `todos`), and change only through `cornercase todo` (`add`, `done`/`undone`, `edit`, `move`, `rm`, `clear-done`, `restore`, `send`). Pending items come first, then done ones, as the server orders them; malformed items are skipped.
- **Text goes after `--`** in `add` and `edit`, so an item starting with a dash is text, not an option.
- **`undone` is idempotent, `done` is a toggle**: the check sends `done` only for a pending item and `undone` only for a done one, so a double click or a stale view cannot flip it back.
- **Both text fields wrap** (`TextField::wraps`, `design/widgets.md`): a long item breaks into rows inside its box, as it reads in the list, instead of running out past the panel's edge.
- **Editing in place commits on blur**: clicking anywhere else saves, so nothing typed is lost; an empty text keeps the old one; Escape cancels.
- **Removing asks nothing**: `rm --json` and `clear-done --json` answer an `undo` number, and the panel offers Undo (`todo restore N`) for 6 seconds, as the TUI's undo toast does. A later removal replaces the bar; the timer only clears the bar it was started for.
- **Moving stays within an item's kind** (pending or done), as the server keeps it. The arrows step one place (`--before` the neighbour, or `--last`); drag and drop puts an item before the one it is dropped on when that is above, after it when below, as lists usually reorder; dropping on the space under the list sends it to the end of its kind.
- **The row's buttons show on hover**, but their column is always there so the text never reflows.
- **Sending an item to an agent** (`todo send ID --pane P --json`) types it into the agent; the server checks it off once the agent took it and refuses, with its reason, when the agent cannot take a prompt now. An unconfirmed send or an item that changed meanwhile stays to do, and the toast says which.
- **The send menu labels agents as the TUI's**: every agent pane in the sidebar's order, `project › workspace · agent` with the workspace only when the project has several; labels that would read the same get the tab's name, then the pane's id.
- **The send menu says which account each agent runs on** (a chip, `AccountsInfo::mark`, under `account_mark` and only once there is an account, `design/accounts.md`): `todo send` goes to a running pane and takes no `--account`, so there is no account to pick here, and choosing the target is what chooses it.
- **An item removed elsewhere closes what was open on it** (its edit field or its send menu).
- **Sending an item to an agent is a job**: the item's buttons give way to a spinner until `todo send` answers, and the status bar names it. Adding, ticking, editing, moving and removing answer in milliseconds and show at once.

## Drawn cached while docked

- **The panel is a cached view while docked** (#134; the rule, the measure and the method are in `design/widgets.md`). It reads its own fields (`editing`, `sending`, `undo`; `remove_item` now notifies when it closes an edit), its text fields and `Store` (`DockKey`).
- **The send menu is drawn uncached while it is open** (`data::drawn_cached`, read by `todo::drawn_cached`, which `PanelKind::drawn_cached` calls): its agents' working badges are `ui::badge`s, whose orbit turns only when they are drawn, and a cached view would freeze them. Opening or closing the menu notifies the panel, which marks `Root`, which switches the wrapper. Rejected: probes and an orbit layer over the menu (a `Spots` handle reaching the panel and cleared on every draw, and the leaving dock's orbits to keep or drop, for a menu open for seconds), and a still badge (a working agent's badge turns everywhere else).
