# cornercase-gpui

A native macOS window for [cornercase](../cornercase-terminal), drawn with GPUI. It is only a UI:
cornercase's server keeps owning the shells, agents and worktrees, and this app drives it through
the `cornercase` command line, the contract cornercase documents for programs that draw it.

| What | cornercase command |
| --- | --- |
| Session (sidebar, tabs, agent states) | `cornercase view --cols C --rows R` (one JSON line per change) |
| Terminal cells, cursor, splits | `cornercase surface --view ID [--patches]` |
| Window resize | `cornercase view-set --view ID --cols C --rows R` |
| Typing, paste, mouse for programs | `cornercase input --pane P <events>` (ordered queue) |
| Wheel over history, selection, ⌘C | `cornercase scroll`, `cornercase selection [--copy]` |
| Dragging a divider | `cornercase ratio --tab T --path …` |
| Tabs, splits, projects, workspaces, agents | `focus`, `new-tab`, `split`, `close`, `open`, `new-workspace`, `start` (all with `--view`) |
| Colours programs see | `cornercase start-server --theme -` (cleared with `theme --clear` on quit) |
| ⌘-click on a path | `cornercase link --pane P --cell X,Y` |

## Run

```sh
cargo run --release                 # opens on what cornercase's window shows
cargo run --release -- --tab 6      # or on a tab, workspace or project id (`cornercase status`)
```

Needs `cornercase` on the PATH (or `CORNERCASE_BIN`). It starts the server if none runs.
`CORNERCASE_GPUI_FONT_SIZE` sets the terminal font size (Menlo, 13 by default).

## What it has

- **Terminal**: live panes from `surface`, splits and dividers (drag, double-click to even), selection
  (drag, double/triple-click), ⌘C/⌘V, wheel, mouse for programs that read it, ⌘-click links and paths,
  right-click pane menu (split, move, close agent, mark), right-drag a pane onto another to move it.
- **Sidebar**: groups (icon, colour, fold), projects, workspaces (worktree, PR and checks, behind,
  setup, changes), agents as a tree with status/model/context, inline rename, drag to reorder,
  context menus, remove-worktree check, quick filter (⌘P).
- **Tabs**: rename, reorder, close with confirmation, new shell/agent menu.
- **Docked panels**: Changes (⌘⇧C), Files (⌘⇧E), TODO (⌘⇧L).
- **Sheets**: Issues (⌘I), Usage (⌘U), Settings (⌘,), Accounts, Queue, Flows, Search (⌘⇧F), Plugins.
- **App-wide**: agent notices as toasts with "Go" and macOS notifications, sounds, OSC 52 clipboard,
  bell, window title, update/restart, away, keep awake.

## Keys

Everything goes to the program in the active pane, except ⌘ shortcuts:
⌘C copy selection · ⌘V paste · ⌘T new tab · ⌘D / ⇧⌘D split right / down · ⌘O open project ·
⌘N new workspace · ⌘G start an agent · ⌘1…9 tab · ⇧⌘[ / ⇧⌘] previous / next tab · ⌘+ / ⌘- / ⌘0 zoom ·
⌘P filter the sidebar · ⌘Q quit. Text goes through macOS's input method, so dead-key accents and ⌥
characters (`@` on a Spanish keyboard) work.

## Waiting on cornercase

Gaps found while building this, filed upstream instead of worked around:
#461 (`--view` breaks feature checks: no surface patches, no hidden/visible, `start --issue --view`),
#462 (Codex limits), #463 (queue undo), #464 (config import preview), #465/#466 (flow save, import
check), #467 (reveal a search match), #468/#469 (plugin install preview, service states),
#470/#471 (changes digests, `gap --hunk` numbering). The UI names the issue where a feature waits.

## Development

`docs/AGENT-BRIEF.md` describes the architecture (`Store`, panels, widgets) and the rules;
`docs/cornercase-cli.txt` is every command's help. `CORNERCASE_GPUI_OPEN=<panel>` opens a panel at
start.
