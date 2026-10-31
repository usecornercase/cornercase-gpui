# Contributing

Thanks for helping. Bug reports, ideas and pull requests are all welcome. Everyone taking part is expected to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## Before you start

For anything bigger than a small fix, open an issue first so we can agree on the approach. cornercase-gpui has a few deliberate choices that are easy to break by accident:

- **It is only a UI.** cornercase's server owns the shells, agents, worktrees and state, and this window drives it only through the `cornercase` command line. When cornercase does not expose something a feature needs, the fix belongs upstream: open an issue in [cornercase-terminal](https://github.com/usecornercase/cornercase-terminal/issues) and make the UI say what is missing, instead of scraping text output, reading cornercase's files or reimplementing its logic ([`design/upstream-gaps.md`](design/upstream-gaps.md)).
- **Keys go to the program in the pane.** Only ⌘ shortcuts belong to the window: the table in `src/keymap.rs`, and the app menu's system keys in `src/main.rs`. ⌥ and Ctrl combinations belong to the program and to macOS's input method; please do not bind them.

The rules for every change are in [AGENTS.md](AGENTS.md), and the design decisions and the reasons behind them in [`design/`](design/), one file per area, listed in [DESIGN.md](DESIGN.md). Read the file for the area you touch.

## Setup

You need macOS with the Xcode Command Line Tools (`xcode-select --install`), [rustup](https://rustup.rs) (it installs the pinned Rust version by itself) and [cornercase](https://github.com/usecornercase/cornercase-terminal) 0.12.234 or newer on your `PATH` (or named by `CORNERCASE_BIN`). Clone cornercase-terminal next to this checkout too: its `design/` and `src/` explain the behaviour this window draws. `docs/cornercase-cli.txt` holds the help of every `cornercase` command.

```sh
cargo run                  # the first build compiles every dependency optimised, a couple of minutes
cargo run -- --tab 6       # opens on a tab, workspace or project id (`cornercase status`)
```

The window drives the same server as your everyday cornercase, and starts it if none runs. Closing, discarding and removing act on that real session, so try them on a throwaway project (`cornercase open /tmp/…`). A server of its own keeps your session and settings out of it entirely:

```sh
CORNERCASE_SOCKET=/tmp/cc-gpui/server.sock cargo run
```

Quitting the window runs `cornercase theme --clear`, which also takes back the colours your own cornercase window gave the terminals; stop a window you started for a test with `kill <pid>` instead.

## Checks

CI runs all of these; please run them before opening a pull request:

```sh
cargo fmt
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --locked
cargo-machete              # cargo install cargo-machete
npx -y jscpd@4.3.0         # copy-paste detector
```

Tests never run the real `cornercase`, open a window or sleep: logic is factored out of `render` into plain functions and tested with the JSON fixtures in `tests/fixtures/`.

## Code style

- Everything in English: identifiers, test names, messages, UI text.
- No code comments, including doc comments. If a decision needs explaining, add it to its area's file in `design/`.
- No `unwrap()` outside tests. Errors use `anyhow`, and a failed command becomes a toast, never a panic.
- Small functions: `clippy.toml` sets the limits, and they are never silenced.
- Tests are named as sentences (`a_closed_tab_leaves_the_view`) and test one behaviour each. A test added as a safety net must fail without the code it protects.
- **Where code goes** is in [`design/code-layout.md`](design/code-layout.md): one module per panel under `src/panels/`, widgets in `src/ui/`, tests in files of their own and at most 1,000 lines per file. `cargo test` checks it through `tests/layers.rs`, whose exception lists only shrink.

## Pull requests

Keep them focused: one change per pull request. Describe what changed and why, how you tested it, and add a screenshot or a recording for anything visible. If it changes a design decision, update the area's file in `design/` in the same pull request.

Code or assets taken from another project keep their licence: add its licence file and a line under the README's License, and only take them from projects under a licence compatible with MIT and Apache-2.0.

By contributing, you agree that your contributions are dual licensed under MIT and Apache-2.0, like the rest of the project.
