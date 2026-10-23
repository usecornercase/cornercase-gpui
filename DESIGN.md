# Design decisions

Why cornercase-gpui works the way it does, area by area. The rules every change follows are in [AGENTS.md](AGENTS.md), including when to read and update these notes. This file is their index: the notes are in [`design/`](design/), one file per area.

## How to use it

- Each line below gives an area's file, the code it covers and what it decides. Paths are under `src/` unless they name another folder; `#N` upstream is an issue of `usecornercase/cornercase-terminal`.
- A new area gets its file and a line here.

## What goes in an area file

- What was decided and why, the alternatives that were rejected and why, limits that are not obvious, what waits upstream, and what was or was not checked live, with the versions.
- Not how the change was built, raw measurements (keep the conclusion and its date), or names and values the code already states.

## Areas

- [Architecture](design/architecture.md): what each file of `src/` holds, `Store`, panels, widgets.
- [Code layout](design/code-layout.md) (`tests/layers.rs`, `clippy.toml`): no comments, where code goes, 1,000 lines per file, small functions, `pub(crate)`.
- [Upstream gaps](design/upstream-gaps.md): no workarounds, filing issues upstream, stopgaps until a fix lands.
