# Terminal

Code: `src/terminal.rs` (keys, ⌘ shortcuts, the surface element), `src/terminal/` (`mouse.rs`, `selection.rs`, `pane_menu.rs`, `input.rs`), `src/render.rs`, `src/keys.rs`, `src/tabs.rs`, `src/context_menu.rs`

## Keys and text

- **Keys reach the terminal only while its own focus handle is focused**, never while a text field or a focused panel has the focus; ⌘ shortcuts are the exception and are read in `Root::shortcut` whatever has the focus (⌘C and ⌘V act on the terminal only when it is focused).
- **Printable text goes through the platform's input method** (`EntityInputHandler for Root`), which composes accents, emoji and ⌥ characters (⌥2 types `@` on a Spanish keyboard). `keys::key_event` takes only what is not text: named keys, and anything with Ctrl. Encoding text ourselves would break dead keys and every non-US layout.
- **Ctrl with a key** sends the key itself, lower case (ctrl+space, ctrl+letter, ctrl+[); cornercase encodes it for the program.
- **⌥ with a key that types nothing else is meta** (⌥← moves by words in the shell): the key goes as a named key with `alt` rather than as text. ⌥ with a key that types a character is left to the input method.
- **Control characters typed through the input method are dropped**; Enter, Tab and friends arrive as named keys instead, so they are not sent twice.
- **⌘0 resets the font size to 13**, the size the window opens with; ⌘+ and ⌘- step by one point between 8 and 32. A new size drops the measured `Metrics`, which are measured again on the next draw.

## Drawing

- **A canvas measures, then paints**: the prepaint step records the inner origin (the cells start `PADDING` inside the element), fits as many whole cells as the space holds (at least 1, at most 1000 columns and 500 rows) and, when that differs from the size asked last, asks cornercase for it on the next frame (`set_size` deferred), so a resize never re-enters the window's update.
- **Text sits on the grid whatever font a glyph falls back to**: words of single-width cells are shaped together with a forced advance of one cell; a wide cell (followed by an empty cell) or a composed one (several characters) is shaped alone at its column. Shaping whole lines without a forced advance would drift as soon as a glyph comes from a fallback font.
- **The block cursor goes under the text**, which is drawn over it in the pane's background colour; bar and underline cursors draw over the cell. An unfocused terminal, or the `block_hollow` shape, shows a hollow block.
- **Only the active pane shows its cursor** when the tab has several; the others are dimmed with a translucent black wash.
- **A divider is drawn as a terminal-coloured band with a one-pixel line** in its middle, so the gap between panes reads as one line whatever the cell size.
- **Positions past the surface are allowed**: `Metrics::cell_at` returns cells outside the grid (negative ones included), so a drag that leaves the terminal still reaches its program or the selection, clamped to the pane.

## Mouse

- **The left button selects**, with cornercase's own selection (`selection --from --to`), unless the program reads the mouse; Shift forces a selection even then. A finished selection is copied, as the TUI does; a click without a drag clears it.
- **A double click selects a word, a triple click the line**. A word is a run of cells of one class: blank, word (letters, digits and `_-./~:@%+=#?&`, so paths and links select whole) or other punctuation. The empty cell after a wide character belongs to it.
- **The right button is cornercase's in every pane**, even one whose program reads the mouse, since almost no program uses it: a click opens the pane's menu, a drag onto another pane moves it there (`move-pane`). The menu offers "Send the right click to the program" for the rare program that wants it.
- **Where a dropped pane goes**: the middle third of the target both ways swaps the two; elsewhere the nearest edge takes that side (`--place left|right|above|below`).
- **⌘-click opens a hyperlink the program printed** (OSC 8), or else a path cornercase recognises under the pointer (`link --cell`), opened with macOS's `open`. Nothing under the pointer is a toast.
- **Dividers**: a drag sets the split's ratio (kept between 0.1 and 0.9, coalesced so only the latest reaches cornercase); a double click evens the split out, as on the TUI's borders.
- **The wheel** accumulates fractional lines and acts per whole line, at most 25 at once. A program that reads the mouse gets wheel events; a full-screen program (alternate screen) with alternate scroll gets arrow keys, and without it nothing; anything else scrolls cornercase's scrollback (`scroll --up/--down`).

## Tabs

- The tab bar shows the shown workspace's tabs. A tab renames in place on a double click, drags to reorder (`move --tab`, landing before or after by the drag's direction; dropped on `+` it goes last), opens its menu on a right click, and shows its agent's model and context use.
- **Closing a tab that runs an agent asks first**, since closing it stops the agent; its worktree and branch stay. A shell tab closes at once, like closing a pane without an agent.
- `+` offers a shell, the default agent (`start --focus`) or the start dialog.

## Context menu

- **One menu for the whole window**: a right click or a row's `⋯` opens it at the pointer, drawn over everything (deferred, snapped inside the window). Escape, a click outside or any entry closes it, and focus returns to the terminal.
- **A submenu takes the menu's place**, as the TUI's *move to group* does, instead of opening beside it: no hover timing, and it fits a narrow window.
- **Separators come from optional entries left out**, so the menu drops separators at either end and doubled ones rather than every caller checking.
- **An entry runs on the window after the menu has closed**, so it can open a dialog or another menu.
- Entries that stop or remove something are red (`danger`); a `Note` is grey text that does nothing, used to say why something is missing.
