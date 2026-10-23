# Changes panel

Code: `panels/changes.rs`, `panels/changes/`

The workspace's git changes, through `cornercase changes`, with the actions of the TUI's panel (cornercase's `design/changes-panel.md`).

## Following

- **One stream, `changes list --follow`**, for the workspace our view shows, in one of the TUI's modes (Uncommitted, Commits, All), or `--every` for every project's uncommitted changes. Every project takes no `--mode`: it is only ever uncommitted.
- A closed panel follows nothing; opening it starts the stream again. A stream that stops says why (its standard error) instead of going quiet.
- Each restart bumps a generation, so lines from an older stream are dropped.
- Unknown values from a newer cornercase (a status, a fold class, new fields) read as `Other` or are ignored, so a summary never fails to parse.

## The selected file

- **A file's digest decides when to read its patch again**: cornercase changes it whenever the file's change does, so a new summary rereads only the files that changed, and a file that left the summary closes.
- Each read bumps a generation, so a late answer for an older version is dropped.
- While the same file keeps the same hunks, the gaps already read stay open and the scroll position is kept.
- **Gaps**: only the unchanged lines between two hunks can be read, as in the TUI; `changes gap` has none above the first hunk or below the last. `--hunk` takes the index from 0 of the hunk after the gap, whatever the help says (#471 upstream).
- Line runs: the highlighting's segments are cut again where the changed words start and end; emphasis comes in characters, not bytes, and segments that do not add up to the line are ignored rather than drawn wrong.

## Actions

- **Viewed** is shown at once and confirmed by the next summary.
- **Stage** stages while anything is unstaged and unstages once everything is, as the TUI's `+`. The mark is blank when nothing is staged (a dim `+` on a hovered row), `◐` partly, `●` fully. Staging and discarding exist only in Uncommitted and Every project; `staged` is absent elsewhere.
- **Discard** always confirms, in cornercase's own words per kind of change (an untracked or added file is deleted; a rename brings the old name back). It discards the file as it is now, even if it changed since the diff was read, because the command line cannot name the version seen (#470 upstream); the confirmation says so.
- **Choosing a base** (`changes base BRANCH` or `--default`) is shared with every window, so the stream answers it rather than the panel.
- **Open in editor** opens a new tab of the file's workspace running `${VISUAL:-${EDITOR:-vi}} +LINE 'PATH'`, as the TUI's hunk `open`. `--view` goes before `--`: after it, it would be typed into the shell.
- **Ask agent** pastes `path:a-b ` (the new side's lines the hunk changed, or the line where lines were removed) into the workspace's agent, the shown tab's first, and focuses it, as the TUI's `ask agent`. Without an agent it copies the reference.
- **Copy** copies the hunk as a patch, as the TUI's hunk copy.
