# Changelog

Every pull request that changes the app adds a section here for its new version. The section becomes the notes of the GitHub Release and shows up in the window's update dialog, so write it for users.

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
