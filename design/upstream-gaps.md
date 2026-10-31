# Upstream gaps

Code: every panel

- **No workarounds**: this app is one more client of cornercase's documented command line. Scraping text output, reading cornercase's private files or reimplementing server logic would break on any cornercase release and hide the gap from other clients. A missing command, field, `--json` or stream is filed upstream, and the UI says what waits ("needs usecornercase/cornercase-terminal#123"), matched where possible by a stable field rather than by error text.
- **The Files panel reads the disk itself** (#345 upstream): a file viewer through the command line would copy every file through a process; cornercase documents this exception for external UIs.
- **Filing an issue**: only for generic gaps or bugs (something any client would hit). Search first (`gh issue list --repo usecornercase/cornercase-terminal --state all --search "<keywords>"`). Follow the repository's templates in `../cornercase-terminal/.github/ISSUE_TEMPLATE/`: a bug (`--label bug`) has `### What happened`, `### Steps to reproduce`, `### Commit`, `### System`, `### Outer terminal` (none: a GUI client driving cornercase through its CLI), `### server.log` and `### Anything else`; a feature (`--label enhancement`) has `### Problem`, `### Idea` (in cornercase's terms: a command, a `--json` field, a stream; reference #345) and `### Alternatives`. In English, concrete, with a reproduction from the shell.
- **A stopgap kept until a fix lands** is listed under "To remove once these land" below, with its files, and removed when the release that fixes it is the one the app needs. The README stays about using the app, so this list for maintainers lives here (moved on 2026-10-31).
- **Where it stands** (2026-10-31): every issue filed so far has landed (#456, #461–#472, #475–#480) except #549. Two limits wait on commands cornercase does not have yet, and the UI says so: removing the worktrees of closed projects and the leftovers git no longer knows (`design/worktrees.md`), and managing plugins on another machine (`design/plugins.md`).

## To remove once these land

- **#549**: showing a fold before the session confirms it (`src/sidebar/folds.rs`, `Folds` in `SideState` and `Msg::Folded` in `src/app.rs`); `set_folded` goes back to queueing the command alone.
