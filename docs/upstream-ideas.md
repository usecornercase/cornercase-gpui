# Upstream ideas for usecornercase/cornercase-terminal

Things that would make UIs on top of cornercase easier, found while building cornercase-gpui (2026-10-29, cornercase 0.12.225).

## Found by the orchestrator

1. **`processes --follow` and `ports --follow`**
   - Problem: the Processes and Ports dialogs refresh every 2 s while open. An external client has to spawn `processes --json` (which itself waits `--interval`) and `ports --json` in a loop: a new process every 2 s, plus the CLI's startup cost.
   - Idea: `--follow` on both, printing one JSON line per sample (processes: every `--interval`; ports: when the set changes), like `changes list --follow`.
2. **Removing worktrees that are not an open workspace**
   - Problem: the TUI's Worktrees dialog removes closed projects' worktrees (with their teardown), prunable entries and orphans after a rescan. `cornercase close --remove-worktree` covers only an open workspace, so an external client cannot clean up the rest. upstream design/worktrees-dialog.md already says this is left to an issue.
   - Idea: `cornercase worktrees remove --path PATH [--check] [--force] --json`, with the same rescan, the risks and the outcome per path.
3. **`worktrees list` that streams rows as each scan answers**
   - Problem: `worktrees list --json` answers only once every worktree is measured, which takes a while with big trees. A client has to ask twice (`--no-sizes`, then with sizes) to show something at once.
   - Idea: `worktrees list --follow --json`: one line per worktree as its scan and its size answer, then an end line.

## From the Changes panel (#470/#471 adoption)

4. **`changes viewed --digest` with a stale digest is silent**: exits 0 and marks nothing. Fail as stage does, or `--json` with `{"marked", "digest"}`.
5. **A stale-digest refusal is only an English sentence**: give it a distinct exit status or a `--json` error (`{"error":"stale","digest":"<current>"}`).
6. **Undocumented flag conflicts**: `viewed --digest` refuses `--mode`, and `--clear` refuses `--digest`; say so in the help, or accept `--mode` with a digest.
7. **`changes gap --hunk N` out of range says "the first has none"** for the last+1 hunk; say "has no hunk N" or print `{"lines": []}`.
8. **stage/unstage/discard print no JSON**: `--json` returning the file's new row (digest, staged) would let a UI update without waiting for the stream.

## From the Files panel on a remote server (#477 adoption)

9. **One stream for a panel's files**: `files watch --workspace W` taking commands on stdin (`list A B`, `read P`) and printing tagged JSON lines, instead of a stream per folder restarted on every expand.
10. **`files stat PATH… --json`** (bytes, modified, kind): checking an open file for changes means listing its whole folder.
11. **Partial answers**: `files list` exits 3 and `files read` exits 1 while printing JSON; a `"partial": true` field with exit 0, or one documented code across commands.
12. **Byte offsets in search results**: `indices`/`ranges` count characters; give byte offsets too, or name the unit in `--help`.
13. **SVG answers as `kind: text`** although the TUI shows it as an image: `kind: image, format: "SVG"`, or a `mime` field.

## From plugin popups (#456 adoption)

14. **Popups need cornercase's own window**: `plugin run` says "no window is attached" when only `view` clients are connected. Count an active `view` as a window, or `plugin run --view ID`.
15. **The popup takes the TUI's size**: let `surface --popup` take `--view ID` or `--cols/--rows`.
16. **The app feed's `popup` has no id**: add `pane` so a client can tell one popup closing and another opening.
17. **No display title**: add the action's `title` and plugin's `name` to the popup in frames and the app feed.
18. **Closing a popup with ✕ makes `plugin run --json` exit 1**: exit 0 for `ended: "closed"`.
19. **Batch popup input**: let `input --follow` lines carry `"popup": true`.

## From plugin install/update/status (#468/#469 adoption)

20. **`--commit SHA` should count as the confirmation**: today a program must also pass `--yes`.
21. **No JSON result after install/update**: `--json` with `--commit` printing `{id, name, version, commit, enabled, previous}`.
22. **Errors in `--json` mode carry the progress line on stderr** (`fetching …` then `Error: …`): print errors as JSON, no progress under `--json`.
23. **`plugin update --preview --json` without an id**: one call previewing every plugin.
24. **`plugin status --follow --json`**, or plugin/service changes in the `app` feed.
25. **`plugin list` vs `plugin status`**: give `list --json` a `services` field when a server answers, or document that `status` is a superset.
26. **`restart_in` is relative**: add an absolute `restart_at`.

## From the Flows editor and import check (#465/#466 adoption)

27. **Which agents, models and efforts a flow can use**: `config --json` lists agents flows refuse; add an `agents` list to `flow list --json` (installed, models, effort levels).
28. **Structured problems from `flow save --check`**: `{role, stage, key, text}` instead of sentences, so a form can mark the field.
29. **`flow list --follow`**, as `flow runs --follow` does (the sheet polls every 3 s).
30. **`flow save --json` returning the saved flow.**
31. **A `flow plan`** (or a field) with each role's stage and the resolved loops, so UIs do not recompute it to draw a draft.

## From the connection work (#461/#475/#476/#478/#479 adoption)

32. **Connection reuse for `--remote`**: every one-shot is a new ssh handshake now that the client cannot share one. `--ssh-option`/`CORNERCASE_SSH_OPTIONS`, or an opt-in shared ControlMaster scoped to the caller.
33. **ssh options suited to programs**: `LogLevel=ERROR`, `RemoteCommand=none`, `ClearAllForwardings=yes` in `--remote` mode (a host with its own RemoteCommand now refuses commands; its LocalForwards fight).
34. **One long-lived request channel**: `cornercase requests --follow --json -` (`{id, command, args, stdin_b64}` in, `{id, ok, stdout, error}` out), ordered with input.
35. **`paste-file` inside `input --follow`**, so a paste stays in order with typing without waiting for answers.
36. **`paste-file` and folders**: `--archive`, or a `--json` refusal kind (`is-directory`, `too-large`).
37. **`input --follow` says nothing until a line fails**: a first `{"ready": true}` line.
38. **`plugin` refuses `--remote` altogether** (list, enable, disable, link, install, remove, logs): a GUI on another machine's server can only read status and run actions. Let the server do them (requests), or forward `plugin …` through ssh as `update`/`logs` do.

## From queue, search, settings, issues (#461–#464/#467/#472 adoption)

39. **Which commands take `--view`**: `queue restore`/`todo restore` reject it (exit 2), while UIs add it everywhere. Accept and ignore it, or list per command in `contract --json`.
40. **`config import --yes` previews again**: what is applied can differ from what was shown. A preview `digest` and `config import --expect DIGEST`.
41. **Import preview sections have an `id` but no display `label`.**
42. **Which removals can still be restored**: `queue removals --json` (and todo), or an `undo_kept` count in the `remove` answer.
43. **`start` answers late**: `start --json --follow` printing `{"tab", "pane"}` once the tab exists, then the final answer, so UIs need not diff snapshots during a trust prompt.
44. **Search hits could carry their reveal**: a `reveal` object (`{query, line}`) in each `search --json` hit.

## From the guided tour (#400 adoption)

45. **Tour texts name the TUI's buttons** (`+ New`, `More`, `Help` at the bottom of the sidebar): a client drawing its own tour cannot reword them. Give each step a client-neutral `action` (`open-project`, `new-workspace`, `open-panel:todo`) besides the text, or let `tour list` take `--client gui` for texts without the TUI's places.
46. **`done` kinds only a TUI sees**: `panel` and `opened` are facts of cornercase's own window, so every client must watch itself and call `tour complete`. Let a view report what it shows (`view-set --panel todo`, `--dialog issues`), so the server completes steps for any client.
47. **The app feed's `tour` names steps by id only**: a client needs `tour list` for titles and texts; carry the active step's title, text, target and show in the feed.

## From the Settings redesign (2026-10-31, cornercase 0.12.234)

48. **The UI page has no sections**: its 24 settings come with `section: ""`, so a client draws one long list (the TUI does too). Sections in `config --json` (the sidebar, tabs, an agent's tab, notifications, sounds, updates, keys) would let every client group them without a table of its own.
49. **A number's unit only shows inside `shown`** (`5 min`, but `off` at 0): a `unit` field on number settings would let a client label the field whatever its value.
50. **Chat pairing needs cornercase's own window**: the code to scan (Telegram) or the link (Discord) is only drawn by the TUI. `config pair KEY --json` answering the code's text or the link would let a GUI draw it.
