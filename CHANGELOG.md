# Changelog

Every pull request that changes the app adds a section here for its new version. The section becomes the notes of the GitHub Release and shows up in the window's update dialog, so write it for users.

## 0.1.15

- **Choose the account an agent starts on.** Once an agent has more than one login, whatever starts it offers a dropdown of its accounts: an issue's Start and Schedule, the Agent button, the Queue's form and Start a flow, one for each agent of the team. It rests on the account in use, named as such; pick another and it applies to that start only. A queued entry you leave alone takes whichever account is in use when it starts, and one whose account has since been removed says so rather than moving to another. If you never pick, nothing changes, and without a second login nothing shows. Settings › UI's account picker turns it off.
- The TODO panel's send menu says which account each agent runs on.
- In the Accounts sheet, the Escape after the one that closes the add form, a rename or a new profile closes the sheet. It used to do nothing until a click.

## 0.1.14

- The window now says what only cornercase's own window can do for now, where you would look for it, and which cornercase issue it waits on: pairing a phone's browser or a Telegram or Discord chat (Settings › Remote), removing the worktrees of closed projects and deleting branches (the Worktrees sheet and the Remove questions), **Run setup** (a worktree's menu), importing a VS Code workspace (the `+` beside Projects) and a plugin's window processes (its card in Plugins).

## 0.1.13

- **Select lines and characters in the Files viewer and the Changes diff.** A click selects a line, a drag or ⇧-click a range of lines (a drag on the numbers keeps to whole lines), a drag on the text selects characters and a double click a word. Open, Ask agent and Copy float beside the selection: ask the workspace's agent about `path:12-30`, copy the text, or open it at that line. In Files, Copy keeps the file's own tabs and a long line's hidden end; in a diff, tabs may come as spaces until cornercase gives the window a line's own text. ⌘C (Ctrl+Shift+C on Linux and Windows) copies the selection too, and Esc clears it.
- **Find in the open file.** ⌘F (Ctrl+Shift+F on Linux and Windows) while the Files viewer has the focus, or its Find button, opens a field over the file: every match lights up, with a count such as `3 of 12`, and Enter and Shift+Enter move between them. A big file stays smooth. Elsewhere ⌘F still opens Search. Opening a result of a text search opens the field with your search in it, so you can go on through the file.
- After Ask agent the keys go to the agent's pane, so you can type your question right away.

## 0.1.12

- **Plugins reach the menus and the rail.** What a plugin offers for a project, a workspace, a tab or a pane shows at the bottom of that row's `⋯` and right-click menu, and of a pane's right-click menu, under the plugin's name. A plugin with a button gets a slot in the rail, after the docked panels: with one action a click runs it, with several it opens a menu of them. How it ended comes back as a toast and a popup opens over the terminal, as from the Plugins sheet. For now a button acts on what cornercase's own window shows and a popup needs that window open (usecornercase/cornercase-terminal#666); the slot's tooltip and the sheet say so.
- A plugin's action that fails now says how (`exit status: 3`) and points at its log, where it only said `cornercase plugin: failed`. Closing a plugin's popup with its `✕` no longer shows an error.

## 0.1.11

- **Changes filters by path.** A field over the file list keeps the files you name: a piece of a path (`order`), a pattern as in `.gitignore` (`*.test.js`, `src/api/**`), `!` to leave files out (`!*.lock`). The summary says how many are left (`3 of 31 files`); Enter gives your keys back to the pane, Escape clears it.
- **Open in…** on a project, a workspace or a pane takes its folder to your file manager, VS Code, Cursor, Zed or Sublime Text, whichever you have. A pane opens the folder its program is in.
- **A project's menu reaches more of the project.** Project settings… opens Settings on it, where a repository can now change the folders Free builds deletes, and Worktrees… lists that project's alone, with a button for every worktree.
- **Import only what you want.** Each section of a shared setup has a tick in the preview; what you untick stays as it is.
- **Processes closes a pane.** It asks first when a program runs there, and an agent's pane goes through Close agent.
- **Ports stops several at once.** Tick the ones to stop, or use Stop all for a workspace's; it asks once, naming each program.
- **A file of a run opens beside the Flow panel.** In the Flow panel a click shows the file in the Files panel, and ‹ Flow goes back, scrolled where you left it.
- **Help.** The rail's `?` now has the docs and the bug report beside the tours, and can reset the tours' progress.
- **The Discord forum's state in Settings › Remote**: ready, or the step it waits for. Choosing among several servers still takes cornercase's own window.

## 0.1.10

- **Usage of every machine.** Once you have machines, the Usage sheet has a row that picks this computer, one machine or all of them, for Limits, Stats and By agent alike.
- Under all, Stats adds the machines up and says that **most at once** is the largest peak of one machine, not a sum.
- Under all, Limits lists each machine's accounts under its name. A Claude Code account logged in on two of them shows once, with `on this computer, vps` under it, and its numbers are the latest reading, never added.
- Old numbers say so: a machine that is down shows the numbers it gave last with `seen 5 min ago`, one whose last answer failed says why and how old they are, and one that never answered says why instead of its rows.

## 0.1.9

- Once an agent has more than one login, the window says which account each agent runs on: after its model in the sidebar and the tab bar, and on a small button at the bottom of its pane. That button, or Switch account in a tab's or a pane's menu, moves the agent to another account in the same conversation, after its turn if it is busy, and tells you what happened. On a shell, the buttons say which account an agent typed there would start on. If you never added an account, nothing changes.

## 0.1.8

- **Search finds settings.** Type in ⌘F and the settings that match are listed after your projects, workspaces, agents and tabs, so those keep their places. Each shows where it lives and its value, such as `UI › sound  [x] on`. Click one or press Enter and Settings opens on that row, scrolled to it and marked for a moment, with nothing changed. A row that doesn't show yet is listed muted with what it needs. Results about accounts, machines, flows and plugins open their own sheets.
- Settings' own search now also finds a setting by the value it shows and by its page, such as `hidden` or `ui sound`.
- Typing while a sheet with no text field is open, such as Accounts, no longer types into the terminal behind it. Keys also keep working after a sheet opens from another one's text field: Escape used to do nothing until a click.

## 0.1.7

- Routines in the Queue: schedule an agent or a flow to start again and again, every so many minutes, hours, days or weeks, every day, every weekday or on a day of the week. Each run gets a fresh worktree from the up-to-date default branch and can remove its workspace when it finishes, if nothing would be lost. A routine shows its next time and its last runs, with why a run failed or kept its workspace, and you can pause it, resume it, start it now, edit it, or remove it with its runs and undo that. It only runs while cornercase's server runs, and only git repositories can have routines.

## 0.1.6

- **See what fills an agent's context.** Right-click a tab running an agent, or an agent's row in the sidebar, and pick **View context**. For Claude Code a dialog shows how full its window is and how that splits between the system prompt, the tools, MCP servers, memory files such as a long `AGENTS.md`, skills and the conversation, with each memory file and MCP server, the skill count and the tools that took the most. Nothing is typed into your agent and no tokens are spent, though its MCP servers start a second time while it reads; **Refresh** reads again. The other agents show their model and how full their context is.

## 0.1.5

- **Machines, a new sheet** (⇧⌘N; Ctrl+Alt+Shift+N on Linux and Windows): the other computers your agents can run on, which cornercase links to through your own `ssh`. It shows this computer and each machine with its link in words, live: `connecting…`, `linked · 0.13.1`, `unreachable · seen 2 min ago` with what `ssh` said under it, or `older` with **Update…**. **Add machine** opens a form that says at once what it would refuse and offers the hosts of your `~/.ssh/config`. Each machine can be edited, its link tried again, or removed, which leaves its server running there. To move the whole window to another server, ⇧⌘K still does that.
- Escape and the shortcuts keep working in a sheet after you answer a question over it; they used to stop until the next click.

## 0.1.4

- **See what would conflict before you merge.** Merge check, in a workspace's menu, says whether its work, committed or not, merges cleanly into its base branch and with the workspaces it shares files with, and which files would conflict and how. It works in a scratch copy, so nothing in your repository changes, and Ask agent hands the conflicts to the workspace's agent.
- **See when two workspaces change the same files.** Turn on Settings › Worktrees › shared files and both rows show `⇄` and how many files they share, and those files carry a `⇄` in the Changes panel. Rest the pointer on a mark to see which files and who else changes them; a click on a row's mark opens the merge check.

## 0.1.3

- Editing a flow in the Flows sheet: a prompt or the notes now take several lines, and a long one scrolls inside its field.
- Stages move up and down with ↑ and ↓, and moving or removing one keeps every loop pointing at the same stage. A loop that would no longer go back to an earlier stage is dropped, and a message says which.
- Removing a role also removes a stage it leaves empty.
- A role's and the orchestrator's arguments and variables can be edited in the form.
- The form ends with the plan of what you are editing, redrawn as you change it.
- A click past the end of a token typed in Settings, followed by a key, no longer closes the window.

## 0.1.2

- **Recap: an agent sums up what your agents did**, in every project, in a panel beside the terminal: the new icon on the rail, or ⇧⌘H (Ctrl+Alt+Shift+H on Linux and Windows). **Now** says where things stand and what waits on you first, **Yesterday** is for your daily (the last day you worked with agents, so on Monday it is Friday) and **7 days** goes through the week, project by project. Every conversation and pull request it names is a link: to the agent's pane, ready for you to answer, or to the pull request on the web. An agent writes it in the background, which can take a minute or more, so you can close the panel meanwhile; ↻ writes it again. Settings › Agents › Recap chooses the agent, its model and the language.
- When the window is too short for every icon on the rail, the panels' icons scroll, so Settings always shows.

## 0.1.1

- The window now needs cornercase 0.13.1 or newer on every platform, the release that brings machines and routines; run `cornercase update` if yours is older, and the window says so when it is.

## 0.1.0

- The window tells you when a new version of it is out and shows what changed; on Windows and Linux it installs it and opens again on the tab you were on.
- First release: a native window for cornercase, drawn on the GPU with GPUI. Downloads for macOS (universal, 12 or later), Windows (x64) and Linux (x86_64). The macOS one is signed and notarized by Apple, so macOS only asks once whether to open an app downloaded from the internet; the Windows and Linux ones are not signed yet. It needs cornercase 0.12.234 or newer, 0.12.287 or newer on Windows.
