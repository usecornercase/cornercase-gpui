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
- **Gaps**: only the unchanged lines between two hunks can be read, as in the TUI; `changes gap` has none above the first hunk or below the last. `--hunk` takes the index from 0 of the hunk after the gap, as the help says since #471 upstream (checked live with cornercase 0.12.225: asking for hunk 0 is refused, the first gap is hunk 1's).
- Line runs: the highlighting's segments are cut again where the changed words start and end; emphasis comes in characters, not bytes, and segments that do not add up to the line are ignored rather than drawn wrong.
- **Shared files**: a yellow `⇄` before a file's tags and counts says another workspace of the project changes it too, with who in its tooltip, in the project scope and in Every project. The panel only reads `view`'s `shared`, so the mark costs it nothing and follows the setting (`design/shared-files.md`).
- **The file list and the diff share the panel**: until their border is dragged (`design/widgets.md`), the list fits its rows up to about a third of the room, so a few files leave the diff the rest; once dragged it keeps that height whatever the rows, always leaving the diff some lines, and a double click makes it fit again. The handle belongs to the diff's header, which is drawn after the list, so it lies over both and sits on the border even while the list fits its rows, whose height the panel does not know.

## The filter

- **A field over the file list filters by path** (`filter.rs`, plain functions tested without a window), the TUI's filter from cornercase 0.5.1 (its `design/changes-panel.md`) with the same words: each word keeps the files holding it, `!` leaves files out, `*` and `?` make it a pattern as in `.gitignore` (no `/`: any part of the path; with `/`: the whole path and what is under it; `**` crosses folders; a trailing `/` only folders), a capital makes that word case-sensitive, and a renamed file is found by either path. It filters only the list on screen, which the window already holds, so nothing goes through the command line: `changes list` has no such option and needs none.
- **It is always there while the panel holds a file**, one row of the header, under the summary. The TUI opens it with ` ⌕ ` because a row costs it a line of a terminal; here the header has the room, and a field hidden behind a button is a field nobody finds. With nothing to filter it goes, and its text stays for when files come back.
- **The summary says how many are left** (`3 of 31 files`); the lines added and removed, and a project's header under Every project, still describe every file, as in the TUI, and a header whose files all left the list hides.
- **Enter gives the keys back to the pane and keeps the filter** (`Store::give_keys_back`), Escape clears it, and on an empty field goes on to close the panel, as every search field here. A file open in the diff stays open when the filter hides its row: its review is not what the filter is about. The filter lives on the panel, so changing scope or workspace keeps it; typing goes back to the top of the list.
- Checked live on 2026-11-06 (debug build, cornercase 0.13.1, Linux x86_64 under Xvfb with lavapipe, a private server; no macOS, no Windows): `order !*.js` over six changed files, `1 of 6 files`, Enter and then typing in the pane, its text kept; Escape clearing a query and, once more, closing the panel. Every project's headers and a filter over a long list were not tried.

## Actions

- **Every action on a file names the version shown** (`--digest`, #470 upstream): the diff's digest when the file is open, the row's otherwise, so a file that changed since is left alone. A refusal is toasted in cornercase's words and the diff is read again; the stream brings the new row. The actions still go through the ordered queue (`store::queued`, or a confirmation's `on_reply`), so a quick stage then unstage keep their order.
- **Viewed** is shown at once and confirmed by the next summary. Marking takes `--digest` and no `--mode` (cornercase refuses both together); clearing takes `--mode` and no digest (`--clear` refuses one). A mark for a version that is gone is dropped silently, with no error to show, so only the next summary undoes the tick.
- **Stage** stages while anything is unstaged and unstages once everything is, as the TUI's `+`. The mark is blank when nothing is staged (a dim `+` on a hovered row), `◐` partly, `●` fully. Staging and discarding exist only in Uncommitted and Every project; `staged` is absent elsewhere.
- **Discard** always confirms, in cornercase's own words per kind of change (an untracked or added file is deleted; a rename brings the old name back). Only the version shown is discarded; the confirmation says a file that changed since is left alone.
- **Choosing a base** (`changes base BRANCH` or `--default`) is shared with every window, so the stream answers it rather than the panel.
- **Open in editor** opens a new tab of the file's workspace running `${VISUAL:-${EDITOR:-vi}} +LINE 'PATH'`, as the TUI's hunk `open`. `--view` goes before `--`: after it, it would be typed into the shell.
- **Ask agent** pastes `path:a-b ` (the new side's lines the hunk changed, or the line where lines were removed) into the workspace's agent, the shown tab's first, and focuses it, as the TUI's `ask agent`. Without an agent it copies the reference. The pull request dialog asks the same way (`Store::ask_agent`).
- **Copy** copies the hunk as a patch, as the TUI's hunk copy.
- **Choosing a base is a job**: the base button reads `switching…` until `changes base` answers, since git recomputes the diff; the diff's `updating…`, the branch list and the empty states show a spinner.
- **Staging, unstaging and discarding are jobs** keyed by the file (`actions::file_busy`): the row's stage mark turns into a spinner and the open file's buttons into `Staging NAME…` (or `Unstaging`, `Discarding`) until git answers; they keep their place in the queue (`store::queued_busy`). Marking a file viewed stays optimistic, as it was.
