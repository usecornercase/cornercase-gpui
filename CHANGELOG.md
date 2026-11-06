# Changelog

Every pull request that changes the app adds a section here for its new version. The section becomes the notes of the GitHub Release and shows up in the window's update dialog, so write it for users.

## 0.1.2

- **See what fills an agent's context.** Right-click a tab running an agent, or an agent's row in the sidebar, and pick **View context**. For Claude Code a dialog shows how full its window is and how that splits between the system prompt, the tools, MCP servers, memory files such as a long `AGENTS.md`, skills and the conversation, with each memory file and MCP server, the skill count and the tools that took the most. Nothing is typed into your agent and no tokens are spent, though its MCP servers start a second time while it reads; **Refresh** reads again. The other agents show their model and how full their context is.

## 0.1.1

- The window now needs cornercase 0.13.1 or newer on every platform, the release that brings machines and routines; run `cornercase update` if yours is older, and the window says so when it is.

## 0.1.0

- The window tells you when a new version of it is out and shows what changed; on Windows and Linux it installs it and opens again on the tab you were on.
- First release: a native window for cornercase, drawn on the GPU with GPUI. Downloads for macOS (universal, 12 or later), Windows (x64) and Linux (x86_64). The macOS one is signed and notarized by Apple, so macOS only asks once whether to open an app downloaded from the internet; the Windows and Linux ones are not signed yet. It needs cornercase 0.12.234 or newer, 0.12.287 or newer on Windows.
