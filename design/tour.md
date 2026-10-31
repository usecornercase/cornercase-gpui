# Tour

Code: `src/tour.rs`, `src/tour/`

- **cornercase's guided tours, drawn here** (#400 upstream): the rail's "?" opens the catalogue (`tour list --json`: every tour with its progress) and **Start** or **Again** runs `tour start ID --no-card`, so cornercase's own window does not draw its card too. The moves (`tour next`, `back`, `skip`, `end`) keep that choice.
- **The state comes from the app feed** (`status --app --follow`'s `tour`: the active tour and step, the progress), so a move made by another client shows here. The list of tours and steps is read once (and again when the catalogue opens); the feed only says where one is.
- **A card at the bottom centre** of the window, over the terminal: the tour's title and `n/N`, the step's title and text, **Docs** (the tour's docs page, in the browser), **End**, **Back** after the first step, **Skip** on a step that waits for an action, **Next** (or **Finish** on the last) on a step that only explains. It occludes only its own rect; keys keep going to the pane. At the top right it hid a docked panel's header.
- **What a step shows opens here**: a docked panel, Issues, the Queue, Search, Settings or Flows, mapped from the step's `show`, and only when nothing of that kind is open, so it never replaces what the user has open; moving on closes only what the tour opened. The Help and Remote menus and the keys menu have no counterpart opened by a step.
- **What a step points at is ringed in the rail** when it names a panel there (`target`); other targets (tabs, the pane, the workspaces) are not pointed at.
- **The GUI reports what only it sees**: a step completed by opening a panel or a dialog (`done` `panel` or `opened`) is marked with `tour complete TOUR STEP` when the matching panel or sheet opens here, which also moves on. Steps completed by what happens in the session (a project opened, a workspace made, a tab, a TODO, a prompt) are counted by cornercase itself.
- **The first prompt step offers "Start an agent…"** (the start dialog), since the TUI's card has its own prompt field.
- **Limits**: the texts are cornercase's and name the terminal window's buttons (`+ New`, `More`, `Help` at the bottom of the sidebar), which sit elsewhere here; the catalogue says so. Noted for upstream.
- Checked live on 2026-10-29 with cornercase 0.12.225: the catalogue with real progress, starting the TODO tour, the rail's ring, opening TODO completing the first step, End; the tour's progress was reset afterwards.
