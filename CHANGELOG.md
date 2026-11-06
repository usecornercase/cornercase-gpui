# Changelog

Every pull request that changes the app adds a section here for its new version. The section becomes the notes of the GitHub Release and shows up in the window's update dialog, so write it for users.

## 0.1.3

- **See what would conflict before you merge.** Merge check, in a workspace's menu, says whether its work, committed or not, merges cleanly into its base branch and with the workspaces it shares files with, and which files would conflict and how. It works in a scratch copy, so nothing in your repository changes, and Ask agent hands the conflicts to the workspace's agent.
- **See when two workspaces change the same files.** Turn on Settings › Worktrees › shared files and both rows show `⇄` and how many files they share, and those files carry a `⇄` in the Changes panel. Rest the pointer on a mark to see which files and who else changes them; a click on a row's mark opens the merge check.

## 0.1.2

- **Recap: an agent sums up what your agents did**, in every project, in a panel beside the terminal: the new icon on the rail, or ⇧⌘H (Ctrl+Alt+Shift+H on Linux and Windows). **Now** says where things stand and what waits on you first, **Yesterday** is for your daily (the last day you worked with agents, so on Monday it is Friday) and **7 days** goes through the week, project by project. Every conversation and pull request it names is a link: to the agent's pane, ready for you to answer, or to the pull request on the web. An agent writes it in the background, which can take a minute or more, so you can close the panel meanwhile; ↻ writes it again. Settings › Agents › Recap chooses the agent, its model and the language.
- When the window is too short for every icon on the rail, the panels' icons scroll, so Settings always shows.

## 0.1.1

- The window now needs cornercase 0.13.1 or newer on every platform, the release that brings machines and routines; run `cornercase update` if yours is older, and the window says so when it is.

## 0.1.0

- The window tells you when a new version of it is out and shows what changed; on Windows and Linux it installs it and opens again on the tab you were on.
- First release: a native window for cornercase, drawn on the GPU with GPUI. Downloads for macOS (universal, 12 or later), Windows (x64) and Linux (x86_64). The macOS one is signed and notarized by Apple, so macOS only asks once whether to open an app downloaded from the internet; the Windows and Linux ones are not signed yet. It needs cornercase 0.12.234 or newer, 0.12.287 or newer on Windows.
