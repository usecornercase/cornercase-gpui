# Recap

Code: `src/panels/recap.rs`, `src/panels/recap/`

The Recap panel shows what `cornercase recap` answers: a summary of what your agents did, written by an agent, for the three ranges of the TUI's panel (upstream's `design/recap.md`, #656, cornercase 0.12.292). Everything that decides what a recap holds, when it is current and who writes it is the server's; the panel asks and draws.

## A docked panel

- **Docked, not a sheet**, as upstream's sits in the column of Changes, Files and TODO: it stays open while you click from one line of work to the next, and a link leaves it where it is. It takes the docks' width and their drag (`DOCK`), and belongs to no workspace, so it shows in any (`PanelKind::shows`).
- **⇧⌘H, the H of history** (`keymap.rs`, Ctrl+Alt+Shift+H on Linux and Windows). The docked panels' keys are ⇧⌘ and a letter, but R is not free: Ctrl+Alt+Shift+R is GNOME's and Cinnamon's screencast, which `no_linux_shortcut_takes_a_key_the_desktop_keeps` keeps out of the table (`design/terminal.md`). The rail's icon is Lucide's `history`, as upstream's is codicon's; it sits after TODO, with the panels that read text, before the wide ones.
- **What the panel reads on opening**: the shown range, each time the dock opens or a range is chosen, as the TUI's `show_recap` does; a current answer comes at once and an out-of-date one starts the agent, so opening the panel is what spends tokens, as there. `↻` and Try again add `--fresh`. It never asks with `--agent`, `--model` or the like: Settings › Agents › Recap holds those, and the form builds that section from `config --json` like any other (no code here).
- **Not restored after a restart**, as upstream's: the dock is not saved, and the answers are the server's to give again.

## Running

- **The command only answers when the agent has**: there is no way to read the last text while a run goes (no `--cached`, unlike `usage`), so a window that has no text for that range yet shows only that it is writing, and one that has keeps its text under the line that says so. `recap` runs beside the queue as a job per range (`Choice::busy`, `store::run_partial_busy`), so a minute of an agent holds up no other command and the status bar names it.
- **Exit 3 is an answer** (`cli::run_partial`): a run that failed prints the last good text and `error`. Any other status is a refusal, whose words are shown as they come: `the running cornercase server has no recap; update cornercase and restart the server`, or another client's run with other options (`busy`).
- **"Writing" shows after the job's own 300 ms** (`Jobs::shown`, the time the status bar waits), so a current answer, which comes in milliseconds, flashes nothing: no spinner, no line, not a word that an agent is at work when none is.
- **`progress` comes only in an answer with `loading` true.** The server answers a waiting request when the run ends, so that happens only when another client starts a new run just then. The panel then shows its steps and asks again after 2 s, a request the server holds until that run ends. `cornercase events` says a run started or ended, not its steps, so it would add nothing here; the panel does not follow it.
- **One run per range**: while the job of a range runs, asking again does nothing and `↻` is a spinner, as upstream's "a second click only shows the progress". Other ranges can run at once.
- **Leaving loses nothing**: the entity stays alive in `Root` (`design/architecture.md`), and each range keeps its own `Slot`. A failure while the panel is closed or on another range is also a toast, so it is not lost until someone comes back. Switching servers drops the panel with the rest of the window; the run goes on there and fills that server's cache.

## The answer

- **`Slot::read` is the whole of what an answer does**, tested on the bench's real answers (`tests/fixtures/recap/`):
  - `text` is kept as the page while the rest of the answer does not change, so its scroll survives asking again; a recap written again starts from the top.
  - `error` shows above the text (a callout, with Try again), hidden while a new run goes, as upstream's status line.
  - `empty` with no text is a page of its own, with what a recap is; beside a text (nothing new to sum up, the last recap still there) it is a note above it, since upstream's window hides it and the line then reads as if that recap were current.
  - A failure with no text keeps what the panel had; `empty` with no text drops it, since the server then holds nothing.
- **The heading is the range's `label`** (`yesterday · wed 4 nov`, the real date), then who wrote it and what it read: `Claude Code · claude-sonnet-4-5 · read 3 conversations · written 5m ago`, redrawn every 20 s while open. Tokens, which upstream's line also has, are not shown (see Left out).
- **The text is drawn by Issues' Markdown** (`issues/markdown.rs`, `issues/text.rs`, `design/issues.md`), in a gpui `list` as there, since a week's recap can run long. The prompt asks for `##` headings and `-` bullets, which it reads; a block's link goes to an `OnLink` the panel gives, where Issues' opens the address. Moving both files to `ui/` was left for when a third user comes.

## Names and links

- **Ids are named after parsing, not before** (`names.rs`). Rewriting `[c3]` to `[name](c3)` first looks simpler, but a name holding `]` or `|` would break the syntax, and inside code the link's source would show. So the Markdown is read first, `[c3]` stays in its text since this reader knows no reference links, and each known id is then cut out of the `Inline` and replaced, its marks and links moved along. A code block and a table's cell, which hold plain text, get the name without a link.
- **Only what the reader would misread is touched before**: an id followed by `(` or `[` gets a space (`[c1]([p1])`), or `[c1](…)` would be an inline link to `[p1]`.
- **`[c9]` with no place stays as written**, as the TUI's and `cornercase recap`'s text do.
- **The names are the CLI's**: `project › workspace › tab`, `#212`, the id for a place with no name.
- **A link's rule is upstream's `recap_go`, less what no command says** (`links::go`, tested without a window):
  1. A pull request opens its `url`, as asked for on 2026-11-06 (upstream opens the workspace's dialog while the workspace is open, else copies the address); it works the same for one merged days ago in a worktree long gone, and `is_web` keeps an agent's text from opening other schemes.
  2. A conversation goes to its `pane` when `boot` is this server's and the pane still exists: the number means nothing on another boot.
  3. Else to the workspace whose path matches, in the project whose path matches; else to that project. A worktree removed or a project closed since still lands near the work, and a toast says the pane is gone.
  4. Else a toast: `api › login › claude cannot be reached: it is closed`.
- **Upstream's first step needs #664**: the TUI looks first for the live pane whose agent and conversation match, which also finds a conversation that moved (`restart`, `agents reopen`). `status --json` and `view` do not name a pane's conversation, so here a conversation that moved falls to step 3; the toast says so, with the issue's number (`design/upstream-gaps.md`). Once it lands, matching `agent` and `conversation` goes before step 2, and `Place` reads them.
- **Following a link gives the keys to the terminal** (`Store::show_in_terminal`): the click left the focus in the dock, and the reason to follow a link that says "waits on you" is to answer. `focus` runs with `--view`, so only this window moves.
- **A link in the text that is neither a place nor a web address** says so in a toast rather than doing nothing.

## Left out

- Tokens in the heading, and copying a recap: gpui's text cannot be selected, so the TUI's terminal selection has no equal here, and `cornercase recap` prints the same text. Not asked for on 2026-11-06; a small button when someone wants it.
- A notice when a run ends while the panel is closed: the status bar's job goes, and the text is there on coming back.

## Checked live

On 2026-11-06, with cornercase 0.13.1, Linux x86_64 under Xvfb with lavapipe, on a private server, the window opened at 1400 × 880 and 1280 × 800. No real agent ran: `agent_commands` pointed `claude` at a script that answers as upstream's `tests/fixtures/claude/2.1.318/recap-*.jsonl` do, and the server's journal was written by hand as upstream's `tests/e2e/recap.rs` writes it, with a live pane, so the dossier, `places`, the cache and the exit statuses were the server's own.

- The rail's icon, Ctrl+Alt+Shift+H both ways and `CORNERCASE_GPUI_OPEN=recap`; the three real `empty` answers on a server with no journal.
- A run of 14 s: the line and the spinner, the status bar's job, the panel closed meanwhile and the text there on coming back. `↻` over a text: the text stays under the line, then is replaced.
- A live conversation's link: the view moved to its tab and typed keys reached the agent in that pane. A conversation with another boot: its workspace and the toast with #664. A closed project: the toast. A pull request and a web link in the text: the window asked to open each address (`gio` said so in the window's log; the bench has no browser).
- A failed run (exit 3, `Not logged in`): the callout above the last text, Try again. A server without `recap`, played by a wrapper around the binary that prints the refusal upstream's `use_recap` words and exits 1: the same callout, with and without a text.
- Settings › Agents shows its Recap section with all six rows, and nothing here had to change.
- **A link needs a frame between press and release**: gpui's `InteractiveText` adds its mouse-up listener only when it draws after the mouse-down, so `xdotool click`, which sends both at once under a slow software GPU, is lost, and a press held for a moment works. Issues' links are the same; a person's click lasts longer than a frame.

Not checked: macOS, Windows, a real Claude Code or Codex writing the text, a real `ssh`.
