# cornercase-gpui

[![CI](https://github.com/usecornercase/cornercase-gpui/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/usecornercase/cornercase-gpui/actions/workflows/ci.yml?query=branch%3Amain)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)
[![Platform: macOS | Linux | Windows](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey)](#install)
[![Discord](https://img.shields.io/badge/chat-Discord-5865f2?logo=discord&logoColor=white)](https://discord.gg/aYqCa4xC4R)

A native window for [cornercase](https://github.com/usecornercase/cornercase-terminal), the terminal multiplexer for running coding agents in parallel, on macOS, Linux and Windows. It is drawn on the GPU with [GPUI](https://gpui.rs), the UI framework behind the Zed editor, and aims to do everything cornercase's own window does, with the look and feel of a native app.

It is only a window: cornercase's server keeps owning the shells, agents and worktrees, and this app drives it through the `cornercase` command line. Both windows show the same session, and closing either one stops nothing.

![cornercase-gpui showing the web-shop project's feat/dark-mode worktree: its theme open in Neovim, its tests passing below, the Changes panel with the checkout's diff, and agents at work in the other worktrees listed in the sidebar.](docs/screenshot.png)

**[usecornercase.dev](https://usecornercase.dev)** · [cornercase](https://github.com/usecornercase/cornercase-terminal) · [Discord](https://discord.gg/aYqCa4xC4R)

## Features

- **Terminal.** Every pane sits on a floating card: drag the gap between cards to resize, select with the mouse, click a web address to open it in your browser or the Browser panel (Settings › Links), ⌘-click links and paths to open them in their default app, drop files to paste them, and drag a pane by its handle onto another to move it. A project or workspace with nothing open says so, with buttons to start an agent, open a shell or reopen its closed agents.
- **Sidebar.** Groups, projects, workspaces and agents, with each agent's status, model and context, and each workspace's pull request with how its checks went: click it, or press ⌘P, for every check with its link, its stack and Ask agent. Drag to reorder, in any layout of cornercase's Settings → UI.
- **Previews.** Rest the pointer on a path a program printed and the file shows beside it: code at its lines, images, PDFs and documents (Quick Look on macOS, the desktop's thumbnailers on Linux, File Explorer's thumbnails on Windows), and videos and sounds that play on macOS. A click opens it in the Files panel. Rest it on a web address and the page shows, drawn by the Browser panel's Chrome: a local dev server's at once, others when you ask.
- **Docked panels.** Changes (the workspace's diff, to stage, discard and review), Files (tree, viewer and search), TODO, a Browser and, in a workspace that runs a flow, Flow: its plan lit up where the work stands, its team and their messages, beside the agents at work.
- **A browser beside the agent.** A real Chrome on the workspace's dev servers, with device sizes and the console. Pick an element or take a screenshot and hand it to the agent.
- **The rest of cornercase.** Search, Issues, Queue, Flows, Usage, Accounts, Processes, Ports, Worktrees, Plugins, Settings and the guided tours, each one shortcut away. A flow, a team of agents, starts from its sheet, from an issue or from a project's `+`, and opens its workspace with the Flow panel beside it (View flow in the workspace's menu opens it later); from an issue, its task comes filled in with the issue's, ready to add to.
- **Other machines.** Connect the window to cornercase on another machine over your own `ssh`; it reconnects by itself when the link drops.
- **Notifications.** Agent notices as toasts and desktop notifications, with sounds and the bell.

## Install

There are no prebuilt releases yet, so build it from source. You need:

- macOS with the Xcode Command Line Tools (`xcode-select --install`), Linux (X11 or Wayland) with a Vulkan driver and the libraries below, or Windows 10 or 11 (x64) with Visual Studio's [Build Tools](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022) and their "Desktop development with C++" workload (the MSVC compiler and the Windows SDK)
- [rustup](https://rustup.rs), which installs the pinned Rust version on the first build
- [cornercase](https://usecornercase.dev/docs/installation/) 0.12.234 or newer on your `PATH` (0.12.287 or newer on Windows, its first release there)

On Linux, install a C compiler, `pkg-config` and these libraries first. On Debian and Ubuntu:

```sh
sudo apt install build-essential pkg-config libxkbcommon-x11-dev libfreetype-dev libfontconfig-dev libwayland-dev mesa-vulkan-drivers
```

Other distributions name them alike: xkbcommon with its X11 part, FreeType, Fontconfig and Wayland's client library, plus the Vulkan loader and your GPU's Vulkan driver.

```sh
git clone https://github.com/usecornercase/cornercase-gpui.git
cd cornercase-gpui
cargo install --locked --path .
```

The first build takes a couple of minutes, and puts `cornercase-gpui` in `~/.cargo/bin`. On Linux, to start it from your desktop's app menu as well:

```sh
mkdir -p ~/.local/share/applications
sed "s|^Exec=.*|Exec=\"$HOME/.cargo/bin/cornercase-gpui\"|" assets/linux/cornercase-gpui.desktop > ~/.local/share/applications/cornercase-gpui.desktop
install -Dm644 assets/linux/cornercase-gpui.svg ~/.local/share/icons/hicolor/scalable/apps/cornercase-gpui.svg
```

An app started from the menu sees your session's `PATH`, not your shell's: if `cornercase` lives somewhere only your shell adds, put its path in `CORNERCASE_BIN` on the `Exec` line (`Exec=env CORNERCASE_BIN=/path/to/cornercase …`).

On Windows, run the same three lines in PowerShell after installing cornercase (`powershell -ExecutionPolicy Bypass -c "irm https://usecornercase.dev/install.ps1 | iex"`); the build puts `cornercase-gpui.exe` in `%USERPROFILE%\.cargo\bin`. A release build compiles gpui's shaders with `fxc.exe` from the Windows SDK, which the Build Tools install; if it is not found, set `GPUI_FXC_PATH` to it. To start it from the Start menu as well:

```powershell
$link = (New-Object -ComObject WScript.Shell).CreateShortcut("$env:APPDATA\Microsoft\Windows\Start Menu\Programs\cornercase.lnk")
$link.TargetPath = "$env:USERPROFILE\.cargo\bin\cornercase-gpui.exe"
$link.Save()
```

The Browser panel runs Chrome, Chromium or Brave if one is installed, else Microsoft Edge, which every Windows has. Opening files and File Explorer go through Windows' shell; desktop notifications show as "cornercase" in Settings › System › Notifications; a sound file of your own must be a WAV; and videos and sounds in a preview play only on macOS.

## Usage

```sh
cornercase-gpui                   # opens on what cornercase's window shows
cornercase-gpui --tab 6           # or on a tab, workspace or project id (see `cornercase status`)
cornercase-gpui --remote devbox   # or on another machine's server, over ssh
```

It starts cornercase's server if none runs. `--remote` takes a host from `~/.ssh/config` or `user@host` and runs `cornercase` there through your own `ssh`, so your config, keys, agent and jump hosts apply. It never asks for a password: use a key or an agent. Add `--remote-command '~/bin/cornercase'` when `cornercase` is not on that machine's `PATH`. From a checkout, `cargo run --release --` takes the same flags.

These environment variables change its defaults:

| Variable | What it sets | Default |
| --- | --- | --- |
| `CORNERCASE_BIN` | The `cornercase` binary to drive | `cornercase` on the `PATH` |
| `CORNERCASE_GPUI_FONT_SIZE` | The terminal's font size (Menlo on macOS; on Linux the first monospace font installed of DejaVu Sans Mono, Noto Sans Mono and the like; Cascadia Mono, else Consolas, on Windows) | `13` |
| `CORNERCASE_GPUI_OPACITY` | The opacity of the terminal's background; `1` makes it opaque | `0.65` |
| `CORNERCASE_GPUI_CHROME` | The browser the Browser panel runs, headless (on Windows, set it for good with `setx`) | Chrome, Chromium, Edge or Brave; on Windows Edge comes last |
| `CORNERCASE_GPUI_BROWSER_PROFILE` | Where that browser keeps its profile | `~/Library/Application Support/cornercase-gpui/browser`; on Linux `~/.local/share/cornercase-gpui/browser`, or `~/snap/chromium/common/cornercase-gpui-browser` for a snap; on Windows `%LOCALAPPDATA%\cornercase-gpui\browser` |

## Keys

Every key goes to the program in the active pane, except ⌘ shortcuts. ⌘/ lists them in the app.

| | Keys |
| --- | --- |
| **Tabs** | ⌘T new · ⇧⌘W close · ⌘1…9 go to one (⌘9 the last) · ⇧⌘[ ⇧⌘] previous, next |
| **Panes** | ⌘D split right · ⇧⌘D split down · ⌘[ ⌘] previous, next · ⌘W close what is on top, else the pane |
| **Projects** | ⌘O open a project · ⌘N new workspace · ⌘G start an agent · ⌘P the workspace's pull request · ⇧⌘P go to a file · ⇧⌘K connect to another server |
| **Panels** | ⇧⌘C Changes · ⇧⌘E Files · ⇧⌘L TODO · ⇧⌘B Browser · ⇧⌘F Flow (in a flow's workspace) |
| **Sheets** | ⌘F Search · ⌘I Issues · ⇧⌘S Queue · ⇧⌘G Flows · ⌘U Usage · ⇧⌘A Accounts · ⇧⌘M Processes · ⇧⌘O Ports · ⇧⌘T Worktrees · ⇧⌘X Plugins · ⌘, Settings |
| **Terminal** | ⌘C copy · ⌘V paste (an image as a file the agent can open) · ⌘+ ⌘- ⌘0 text size |
| **In the Browser** | ⌘L address · ⌘R reload · ⌘[ ⌘] back, forward · ⌘C ⌘X ⌘V on the page; every other key goes to the page |
| **Window** | ⌘H hide · ⌥⌘H hide the others · ⌘M minimise · ⌃⌘F full screen · ⌘Q quit |

The key that opens a panel or sheet also closes it. Esc closes the menu, dialog or sheet on top, or gives the keys back to the terminal. Text goes through macOS's input method, so dead keys and ⌥ characters (`@` on a Spanish keyboard) reach the program.

On Linux, ⌘ is Ctrl+Shift and ⇧⌘ is Ctrl+Alt+Shift, as in Linux terminals: ⌘T is Ctrl+Shift+T, ⇧⌘C is Ctrl+Alt+Shift+C, ⌘C ⌘V are Ctrl+Shift+C Ctrl+Shift+V, and a symbol is the key where it sits on a US keyboard (⌘+ is Ctrl+Shift+=). The exceptions: ⌘1…9 is Alt+1…9, Usage is Ctrl+Alt+Shift+U (IBus takes Ctrl+Shift+U), a page in the Browser also takes Chrome's keys (Ctrl+C Ctrl+X Ctrl+V, Ctrl+L, Ctrl+R or F5, Alt+← Alt+→), the window's keys are Ctrl+Shift+Q quit, F11 full screen and Ctrl+Shift+M minimise, and ⌘-click is Ctrl+click. Selecting text puts it in the primary selection and a middle click pastes it (Shift+middle click where a program takes the mouse), while Ctrl+Shift+C and Ctrl+Shift+V keep to the clipboard. AltGr characters reach the program, and Ctrl+Shift+/ lists every key as Linux spells it.

On Windows the keys are Linux's, as Windows Terminal's own are Ctrl+Shift: ⌘T is Ctrl+Shift+T, ⇧⌘C is Ctrl+Alt+Shift+C, ⌘1…9 is Alt+1…9, Usage is Ctrl+Alt+Shift+U, the Browser takes Chrome's keys, and ⌘-click is Ctrl+click. The window's keys are Windows' own: Alt+F4 quits, Alt+Space opens the window menu, F11 is full screen and Ctrl+Shift+M minimises; the title bar moves, snaps and maximises the window as any other's. There is no primary selection: a finished selection is copied as on a Mac. AltGr characters reach the program, also on layouts where Windows reports AltGr as Ctrl+Alt, and Ctrl+Shift+/ lists every key as Windows spells it.

## How it works

The window never speaks cornercase's socket protocol. It follows `cornercase view` for the session and `cornercase surface` for what the panes show, types through `cornercase input --follow`, and does everything else with `cornercase` commands and their `--json`: the contract cornercase documents for programs that draw it ([#345](https://github.com/usecornercase/cornercase-terminal/issues/345)). On another machine, every command runs as `cornercase --remote DESTINATION …`.

When cornercase lacks something a feature needs, the gap is filed [upstream](https://github.com/usecornercase/cornercase-terminal/issues) instead of worked around, and the window names the issue the feature waits on. The two exceptions are by design: the Files panel reads the disk itself, and the Browser panel runs a headless Chrome and its ssh tunnels. The decisions behind each area, and why, are in [`design/`](design/), indexed by [DESIGN.md](DESIGN.md).

## Contributing

Bug reports, ideas and pull requests are welcome: [CONTRIBUTING.md](CONTRIBUTING.md) explains how to build, run and check a change. Ask questions on [Discord](https://discord.gg/aYqCa4xC4R), and report vulnerabilities privately, as [SECURITY.md](SECURITY.md) explains. The rules for every change, for people and coding agents alike, are in [AGENTS.md](AGENTS.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

Some parts come from other projects and keep their licences:

- The text field (`src/ui/text_field.rs`) is adapted from gpui 0.2.2's `examples/input.rs`, copyright Zed Industries, under Apache-2.0 ([LICENSE-GPUI](LICENSE-GPUI)).
- The icons in `assets/icons/` are [Lucide](https://lucide.dev)'s, under ISC ([assets/icons/LICENSE](assets/icons/LICENSE)), with thinner strokes and two of them filled; the `status-*` and `window-*` icons are this project's own.
- The file-type icons in `assets/file-icons/` are glyphs of [Seti UI](https://github.com/jesseweed/seti-ui) and [Nerd Fonts](https://github.com/ryanoasis/nerd-fonts), under MIT ([assets/file-icons/LICENSE](assets/file-icons/LICENSE)).
