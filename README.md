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

## Keys

Everything goes to the program in the active pane, except ⌘ shortcuts:
⌘C copy selection · ⌘V paste · ⌘T new tab · ⌘D / ⇧⌘D split right / down · ⌘O open project ·
⌘N new workspace · ⌘G start an agent · ⌘1…9 tab · ⇧⌘[ / ⇧⌘] previous / next tab · ⌘+ / ⌘- / ⌘0 zoom · ⌘Q quit.

Text goes through macOS's input method, so dead-key accents and ⌥ characters (`@` on a Spanish
keyboard) work. Mouse: click focuses a pane, drag selects (or goes to the program when it reads the
mouse; ⇧ forces selection), wheel scrolls, ⌘-click opens links and paths, drag a divider to resize.

## Known limits (cornercase 0.12.189)

- `surface --view --patches` and `view-set --visible/--hidden` are refused by the CLI's feature check
  (it sends the view along with `contract`). The app falls back to complete frames and stops
  reporting visibility; patches are implemented and tested for when it is fixed.
- No images (kitty/sixel), no changes/files/issues panels, no settings, no notifications yet.
