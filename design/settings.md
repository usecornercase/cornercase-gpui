# Settings

Code: `src/panels/settings.rs`, `src/panels/settings/`

- **Where the code is**: `settings.rs` holds the sheet's state, loading and its fields; `settings/changes.rs` the commands that change things; `settings/pages.rs` the navigation, pages and setting rows; `settings/controls.rs` each setting type's control and the agents' rows; `settings/connection.rs` tokens and paired browsers; `settings/projects.rs` the Projects and Share pages; `settings/data.rs` what `config --json` answers and how it is grouped, tested without a window; `settings/widgets.rs` the pieces the Accounts sheet shares (navigation, cards of rows, a switch, an inline confirmation).
- **The form is built from `config --json`**: it describes pages, sections, each setting's type, choices and note, so a new cornercase setting shows up without a change here.
- **Each change is saved at once** (`config set`, `config secret`, `config remove`, `config project`, all with `--json`), and the answer, which carries the new revision, replaces what the sheet shows. Notices in the answer become a toast.
- **The sheet reads the settings again** when it opens and when the session's `config.revision` grows, which tells that they changed elsewhere (the TUI, an edit of `config.json`). A revision only compares within one server boot, so a new boot reloads too.
- **A field remembers the saved value it was filled with**: it counts as edited while its text differs, a reload never overwrites an edited field, and after a save the saved value replaces what was typed. Enter saves, escape reverts.
- **An empty value reads as what it means** (`per tracker`, `Plane Cloud`), as the TUI shows it, through the field's placeholder.
- **Tokens are never shown**: their field is masked, never pre-filled, and cleared as soon as the token is sent on standard input to `config secret KEY`. A token taken from an environment variable wins over a saved one, so it cannot be typed here; its status names the variable.
- **`config set` passes `--` before the key**, so values that start with `-` (an agent's extra arguments) are not read as options. `config project` names an open project by id and any other by folder.
- **Whole numbers are checked before sending**, to say what is wrong under the field rather than as cornercase's error; errors lose the `cornercase config: error:` around them.
- **Choices**: a few short ones (up to four, 18 characters, none dangerous) are segments; the others open a list under their row with each choice's note. A dangerous one (skipping permissions) asks first, inline, as do removing a token, forgetting every paired browser and clearing a script: a question under the row, not a dialog.
- **Lists** (`issue_tabs`) show their items in order, with `‹` `›` to move one and `✕` to drop it, then the ones that can be added.
- **An agent's mode and extra arguments** (`agents.<kind>.mode`, `agents.<kind>.extra`) share one row.
- **Pages follow the TUI's tabs** (Worktrees, Agents, Issues, UI, Remote); pages it does not know go after them, then the sheet's own Projects and Share. Sections keep the order they first appear in, connections first (the TUI lists the trackers' accounts before the sources), each connection after its section's settings, untitled ones opening the page.
- **The TUI's notes about keys** (`enter pastes one`) do not apply here and are dropped. Pairing a chat bot shows a code to scan, which this sheet cannot draw: it says to do it from cornercase's own window.
- **A script of several lines** does not fit a one-line field: it is shown, and can only be cleared.
- **Export** (`config export --json`) copies the setup as TOML or writes it to a file (`--to=PATH`); it never holds tokens, accounts or personal keys, and the text is dropped once copied. **Import** waits for a preview of what it would change and run (#464 upstream), so it is shown disabled with that issue.
