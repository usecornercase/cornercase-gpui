# Theme

Code: `theme.rs`, `theme/`, `appearance.rs`, `appearance/`, `app/looks.rs`, `panels/settings/appearance.rs`, `panels/settings/appearance/`, `ui/colour_picker.rs`, `ui/slider.rs`, `ui/track.rs`, `assets/themes/`

The window's colours, glass and corners are one theme the person can change (issue #83). The look in `design/widgets.md` (Tokyo Night's terminal, usecornercase.dev's chrome, glass in layers) is **cornercase dark, the default theme**; this file holds what a theme is and how it travels.

## What a theme holds

- **Chrome tokens**, one list in `theme/tokens.rs` (a macro generates `Chrome`, `get`/`set` and `TOKENS`, which the editor and the contrast check read, so a new token is one line): the ground and surfaces (`glass`, `surface`, `panel`, `raised`, `field`, `veil`, `inset`), four texts (`text`, `soft`, `muted`, `faint`), lines and states (`border`, `edge`, `hover`, `selected`), the accent (`brand`, `brand_hover`, `on_brand`, `accent`, `accent_soft`, `selection`) and five statuses. `inset` is what `colours::sidebar()` was (black at 25 %, used by chips and table headers, never the sidebar).
- **Derived unless set**: `brand_hover` is the brand × 0.847, `accent_soft` and `selection` the brand at 0x2e and 0x47 of 255, so a theme that changes only `brand` stays coherent. Always derived, never in a file: `brand_light`, `raised_hover` (the card colour, opaque), `ink` (the ground, opaque), the floating card's sheen (`card_edges`) and the flows' `sunk`.
- **Fixed, never themed**: the QR code's black on white (`design/settings.md`) and the platform's close-button red.
- **Terminal tokens**: foreground, background, cursor (the foreground until set) and the 16 ANSI colours, opaque because `theme -` takes `[r, g, b]`. The 256-colour cube and grey ramp are built from the 16 as before (`Terminal::palette`). There is **no selection token**: cornercase draws the selection as `inverse` cells in the `surface` frames, so the window cannot tell selected cells from a program's inverse text. A muted line in the Terminal group says so (`design/upstream-gaps.md`).
- **Opacity**, five numbers, clamped on reading and as the sliders' ranges:
  - `ground` 0.55–1: 50 % lost the status bar's faint text over a bright desktop and 55 % kept it, so today's value is the floor. Where the ground is opaque (Linux, Windows without acrylic) the row shows a note instead of a slider.
  - `panes` 0.3–1, and on a translucent ground at least the ground's: a pane cannot be more see-through than the ground (`over_ground`). `CORNERCASE_GPUI_OPACITY` wins when set and the slider rests, saying so.
  - `cards` 0–0.8: 0 hides the cards; above 0.8 they become the opaque blocks the glass design rejected.
  - `floating` 0.8–1: gpui 0.2.2 does not blur the window's own content, so text under a card would show sharp.
  - `veil` 0–0.9.
- **Shape**: `radius` (compact 3/5/8/10, default 5/8/12/16, round 7/11/16/22 for chip, control, card and sheet; `theme::radius::chip()` and the others read it) and `glow` (off makes `glass::glow` draw nothing, and the cobalt gradient stays).
- `glass`, `surface` and `veil` take their alpha from `opacity`: an alpha written on them is dropped, with a warning.

## Where it lives

- Beside `settings.json` (`prefs::path()`'s parent), so `CORNERCASE_GPUI_SETTINGS` moves the themes too and a manual check stays off the user's real files: `theme.json` (which theme, whether to follow the system, the light and dark slots) and `themes/<id>.json`, one file per user theme. The presets are built in (`assets/themes/`, embedded with `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), …))`).
- Rejected: cornercase's config (`config set`), which would need an upstream section the server has no use for, and a window on another machine (`--remote`) must keep the local look; `~/.config/cornercase-gpui`, since the window's files already live in `app_dir()`; the choice inside `settings.json`, whose only writer is `Store::set_pref`, while the theme's state outlives the `Store` across server switches (two owners of one file would race).
- A theme file is a delta: `name`, `extends` (a preset or another user theme, up to 8 deep, default cornercase dark) and the sections `terminal`, `chrome`, `opacity`, `shape`, each optional. Colours are `#rrggbb` or `#rrggbbaa` and nothing else. Unknown keys are kept on edit, so a theme from a newer version loads.
- A user file named like a preset is listed with a problem ("named like a preset: rename it") and ignored.

## Presets

Cornercase dark (the root, today's colours exactly), cornercase light, Tokyo Night, Catppuccin Mocha and Latte, Dracula, Gruvbox Dark, Nord, Rosé Pine, Solarized Dark and Light.

- **Terminal palettes** were compared on 2026-11-09 against `alacritty/alacritty-theme` (95615789c5): Mocha, Latte, Gruvbox Dark, Nord, Rosé Pine and Solarized Dark match it. Dracula follows `dracula/dracula-theme`'s spec (black `#21222c`, bright black `#6272a4`, brights that differ from the normals), which alacritty-theme's older port does not. Solarized Light's foreground is the spec's base00 (`#657b83`), alacritty-theme's is base01. Tokyo Night is cornercase's own fixture palette, kept on purpose: the preset differs from cornercase dark only in its chrome. Gruvbox's palette is `morhetz/gruvbox`'s, with `gruvbox-community/gruvbox`'s MIT licence text, since the original carries none.
- **Chrome** of the terminal presets was computed from each palette (the recipe is in the commit that added them): the ground and surfaces from the background, the text greys from the foreground, `brand` from the palette's signature colour (darkened until white or the background reads 4.8:1 over it), the status colours from the palette's yellow, orange, green, red and magenta (darkened in light themes). `every_preset_passes_the_contrast_check` guards readability. They are first values, tuned by the numbers and not by eye.
- **cornercase light** is usecornercase.dev's light theme (`site/src/styles/landing.css`, `:root`) with upstream's "paper" terminal (`tests/fixtures/theme/light.json`). Its ground is 0.82, panes 0.85 and floating 0.94: a light ground over a dark desktop needs more ground than a dark one. Glow is off.
- Licences: `assets/themes/LICENSE` holds each source's text; the README's License section says so.

## Reading it

- `colours::*` read a process-wide current theme (`theme/current.rs`, a `RwLock<Arc<Theme>>`), because the 1,186 call sites sit in plain builders with no `cx`. Rejected: a gpui `Global` read at each site, a `thread_local!` (`start_server` builds `theme -` off the main thread) and `arc-swap` (a dependency for no measured gain).
- Only `Appearance` sets it. **Tests never set it**: the static is process-wide and tests run in parallel, so they build a `Theme` and call its functions (`Painter` takes the theme it paints with).
- `tests/layers.rs::colours_come_from_the_theme` fails when code outside `theme.rs` and `theme/` makes a colour with `rgb(` or `rgba(`; its list of exceptions is empty. Black shadows go through `theme::shade`, which softens them in a light theme.
- Flows' role colours are the palette's ANSI 4, 5, 6, 2, a mix of 2 and 6, and 3 (Tokyo Night's `#73daca` has no ANSI slot).

## Giving it to cornercase

- `start-server --theme -` and a machine tab's first view send `Terminal::json()` of the theme in use.
- After a change, `theme -` goes to the window's server and to a shown machine tab that was themed, once changes **settle for 300 ms** and only when the JSON differs from the last one given. Dragging a colour must not start one `cornercase theme -` per mouse move, and a change to the chrome alone sends nothing. Mode 2031 (telling programs the scheme flipped) is upstream's.
- Quitting, `theme --clear` and the relaunch skip are unchanged.

## Following the system

- With `follow_system` on, the theme in use is the `light` or `dark` slot by `window.appearance()`. gpui 0.2.2 reports it on every platform: macOS's `effectiveAppearance`; Linux's xdg-desktop-portal `color-scheme` (asynchronous, `NoPreference` reads as light, so the first frame can show the dark theme for a moment); Windows' `AppsUseLightTheme`, refreshed on `WM_SETTINGCHANGE`.
- Choosing a light theme while following fills the light slot, a dark one the dark slot (`Theme::is_light`: the terminal background's luminance). Turning following on puts the theme in use into its slot.

## The Appearance page

- The sheet's own pages are Appearance, Links, Projects and Share; Appearance works without the server, like Links.
- **Editing a preset makes a copy** (`My Gruvbox` in `themes/`, `extends` the preset), made the theme in use: a preset never changes, and a page that refuses to edit until Duplicate is pressed would be a dead control for the commonest wish. Duplicate stays as a button too.
- **Peek**: holding the button hides the sheet and its veil, leaving a transparent layer that turns it off on release (`Store::peeking`, `app/looks.rs`). The release never reaches the terminal. Rejected: an invisible sheet that catches `on_mouse_up_out`, which a release outside every hitbox could leave stuck.
- **The picker** (`ui/colour_picker.rs`): gpui gradients have two stops, so the hue strip is six segments side by side and the square is a white-to-hue gradient under a transparent-to-black one, which matches HSV exactly since gpui blends in the colours' own space. Drags use `on_drag` and `on_drag_move` with one drag type per part; the bounds come from `ui/track.rs`, which a canvas fills on every frame, because a mouse-down handler has none.
- **Sliders** (`ui/slider.rs`) carry the slider's key in the drag value, so a drag moves only its own.
- Global search lists the page as one row when every word matches its words, a preset's name or a token's label, before cornercase's settings and even when `config search` fails.
- Local file reads and writes (save, import, export) are small and run on the main thread, not as jobs (`store::run_busy`); the exception is recorded because an import from a slow or network path would block the window for that long, which no one asked to avoid yet.
- The palette given to cornercase is forgotten when the server is started again (`Root::given`), so a restarted or reconnected server gets it at the next change as well as through `start-server --theme -`.
- Not in Export and Import (`design/settings.md`): the setup is cornercase's; a theme is a file.

## Contrast

- Text needs 4.5 (`text`, `soft`) or 3 (`muted`, `faint`) over the ground, the cards, the panels and a menu, whichever is worst; `on_brand` needs 4.5 over `brand`. The desktop behind the glass cannot be known, so the ground counts as opaque, as Linux draws it; the ground's floor covers the rest.
- **Fix** keeps hue and saturation and bisects the lightness, towards white when the colour is lighter than the surfaces' mean, else towards black. When even the extreme fails, the chip stays red and Fix gives that extreme.

## Live reload

- Every second `Appearance` stats `theme.json` and `themes/` off the main thread and rereads what moved. A file whose text equals what was last read or written is ignored, so the window's own saves are never edits. Rejected: the `notify` crate, a dependency tree with three backends for three files.
- Saves are debounced (300 ms, the same settle as `theme -`), written beside the file and renamed over it (`prefs::write_atomic`), and flushed on quit.
- A problem (JSON that does not parse, a missing or circular `extends`, a `theme.json` naming a gone theme) leaves the window as it was, toasts once per new problem and shows a red callout on the page; a warning (an unknown key, a bad colour, a value kept within its limits) lists under the page.

## Not done

- The importers (the issue's phase 4): iTerm2 `.itermcolors`, Ghostty, Alacritty, WezTerm and Windows Terminal schemes.
- The terminal's font and size in a theme: the font is settled before the window opens (`theme/fonts.rs`).
- The terminal's selection colour (above).
- Renaming a user theme in the page (the file's `name` can be edited by hand).
- Linux and Windows were not run; following the system is covered by unit tests only.

## Checked live

See the report of the change that added this: nothing here was checked against a real desktop's appearance switch.
