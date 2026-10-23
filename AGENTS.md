# cornercase-gpui

A native macOS window for [cornercase](https://github.com/usecornercase/cornercase-terminal), drawn with GPUI (gpui 0.2.2 from crates.io). **It is only a UI**: cornercase's server owns the shells, agents, worktrees and state, and this app drives it only through the `cornercase` command line, the contract cornercase documents for programs that draw it (#345 upstream). The goal is to reach the TUI's features in a GUI with a polished, consistent look.

These rules apply to every contributor and coding agent, in `src/` and `tests/`. The design decisions and their reasons are in `design/`, one file per area, listed in [DESIGN.md](DESIGN.md): read the files for the areas you touch before changing them, and update them in the same change when a decision changes. How these files are loaded is under [Instruction files](#instruction-files).

## Commands

```sh
cargo run --release                         # opens on what cornercase's window shows
cargo run --release -- --tab 6              # or on a tab, workspace or project id (`cornercase status`)
cargo test --locked                         # unit tests + tests/layers.rs
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo fmt
cargo-machete                               # unused dependencies
npx -y jscpd@4.3.0                          # copy-paste detector, reads .jscpd.json
```

- **Running needs `cornercase` on the `PATH`** (or `CORNERCASE_BIN`); the app starts its server if none runs. `docs/cornercase-cli.txt` holds every command's help; `cornercase <cmd> --help` is the source of truth. The cornercase sources are in `../cornercase-terminal` (read its `design/` and `src/` to understand behaviour; never edit it from here).
- **The toolchain is pinned** in `rust-toolchain.toml` (version and components), so a new Rust release cannot bring new lints by surprise. Bump it by hand: change the version, then fix what clippy reports.
- **Before calling a task done, run fmt, clippy, tests, `cargo-machete` and jscpd, and fix what fails.** If one cannot pass, say so.
- `jscpd` fails above 1 % duplication (50-token clones) in `src/` and `tests/`. It is a ratchet: extract the shared code, do not raise the threshold. `cargo-machete` is a text search; a false positive goes in `[package.metadata.cargo-machete] ignored`.
- **Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/)**: `<type>: <summary>`, the summary in the imperative and in lower case, such as `feat: show the queue's next start`. Types: `feat` (a feature users see), `fix`, `docs`, `test`, `refactor`, `perf`, `ci`, `build`, `chore`; a breaking change adds `!`.
- **CI** (`.github/workflows/ci.yml`) runs the same checks on macOS: clippy and tests, then fmt, machete, jscpd and the size of `AGENTS.md`.

## Ground rules

- **No workarounds** (`design/upstream-gaps.md`). When cornercase does not expose something a feature needs (no command, no field, a bug, a missing `--json`, a stream that does not exist), do not scrape text output, read cornercase's private files or reimplement server logic. Open an issue in `usecornercase/cornercase-terminal` and make the UI say plainly what is missing (a muted "needs usecornercase/cornercase-terminal#123"). The one exception, by design (#345 upstream), is the Files panel, which reads the disk itself.
- Use `--json` output wherever a command has it; parse it with local serde types or `serde_json::Value`.
- A workaround kept until an upstream fix lands is listed in the README's "To remove once these land", with the files it lives in.

## Code conventions

- Everything in English: identifiers, test names, messages, UI text.
- **No comments at all**, including `///` and `//!`. Rationale goes in the area's file in `design/`. `tests/layers.rs` enforces it.
- Errors: `anyhow` (`anyhow::Result`, `bail!`, `.context(…)`); a command's failure becomes a toast, never a panic.
- No `unwrap()` outside tests (`clippy::unwrap_used`); in tests prefer `expect("what")`.
- Mutexes are `parking_lot::Mutex`.
- Lints live in `Cargo.toml` (clippy `all` deny, `pedantic` warn). Silence one only locally with `#[expect(clippy::x, reason = "...")]`.
- **Small functions**: `clippy.toml` limits a function to 40 lines, 5 arguments (`self` counts), two levels of nesting inside a method (`excessive-nesting-threshold = 4`) and a cognitive complexity of 10 (`cognitive_complexity`, turned on in `Cargo.toml`). Split the function, usually into named element builders; never silence these four (`design/code-layout.md`).
- Imports in three groups: `std`, external crates, then `super`/`crate`.
- `rustfmt.toml`: `max_width = 120`.
- **Where code goes** (the reasons are in `design/code-layout.md`): one module per panel under `src/panels/`, widgets in `src/ui/`, the window (`Root`) in `app.rs`, `terminal.rs`, `sidebar.rs`, `tabs.rs` and `context_menu.rs`. `impl Root` lives only there, and only they use `crate::app`; panels, widgets and models reach the window's state through `Store`. The tests of `foo.rs` go in `foo/tests.rs`; at most 1,000 lines per file. `tests/layers.rs` checks these, and its lists of exceptions only shrink.
- **Nothing is `pub`**: this is a binary, so every item is `pub(crate)` or narrower. `unreachable_pub` (in `Cargo.toml`) enforces it, so `dead_code` reports what nothing uses: delete it, or put it under `#[cfg(test)]` when only tests use it.
- In a `render` method, name the parts: a function per region (`header`, `rows`, `footer`) returning `impl IntoElement`, so the tree reads top-down and each builder stays within the limits. Edition 2024: a builder returning `impl IntoElement` from a `&self` method needs `+ use<>` when it must not capture the borrow.

## Interaction

- **Every key goes to the program in the active pane, except ⌘ shortcuts**, which live in one place, `terminal.rs::shortcut`, and are listed in the README's Keys. Text goes through macOS's input method, so dead keys and ⌥ characters (`@` on a Spanish keyboard) reach the program: never bind `⌥` or `Ctrl` combinations. The terminal takes keys only while its own focus handle is focused, so text fields and focused panels get theirs.
- Anything that cannot be undone asks first, through `dialogs::confirm`.
- At most one sheet and one dialog are open; a docked panel (Changes, Files, TODO) sits right of the terminal and keeps its state while closed.
- A command's error becomes a toast (`Store::act`, `Store::cmd` and `store::run` do it); never fail silently.

## Architecture

What each file of `src/` holds, `Store` and its helpers, panels and widgets are in [`design/architecture.md`](design/architecture.md). A change that adds, moves or removes a module updates it in the same change.

## Tests

- The tests of `foo.rs` (or `foo/mod.rs`) go in `foo/tests.rs`, declared as the last item with `#[cfg(test)] mod tests;`; a test file over 1,000 lines moves each group to `foo/tests/<group>.rs`. Tests are named as sentences (`a_closed_tab_leaves_the_view`) and test one behaviour each.
- Fixtures live in `tests/fixtures/` and are read with `test_util::fixture!("path")`, never with a relative `include_str!`.
- Logic is tested without a window: factor it out of `render` into plain functions or models (parsing `--json`, ordering, filtering, labels) and test those. No test runs the real `cornercase`, opens a window or sleeps.
- A safety-net test must fail without the code it protects.

### Manual check

- Run the app on a throwaway tab: `./target/debug/cornercase-gpui --tab <id> & echo $!`, and stop only that pid. `CORNERCASE_GPUI_OPEN=<panel>` opens a panel at start; `CORNERCASE_GPUI_FONT_SIZE` sets the terminal's font size.
- Screenshot it: `osascript -e 'tell application "System Events" to set frontmost of (first process whose unix id is <pid>) to true'`, then `screencapture -x <file>.png`.
- Never act on real data: create a throwaway project (`cornercase open /tmp/…`) to try closing, discarding or removing, and clean it up.

## Instruction files

- `AGENTS.md` (this file) is the entry point: Codex and other agents load it on their own. `CLAUDE.md` only imports it and `DESIGN.md` (`@AGENTS.md`, `@DESIGN.md`), so Claude Code loads both. `DESIGN.md` is only the index of `design/`, whose files nothing loads by itself: read them on demand, for the areas a change touches. Every rule and every decision lives in one place, here or in one file of `design/`; never copy text between them.
- Codex loads at most 32 KiB of project instructions and cuts the rest without a warning, so `AGENTS.md` holds the rules and `design/` the decisions. CI fails when `AGENTS.md` passes 32 KiB; move detail to `design/` rather than raising the limit.
