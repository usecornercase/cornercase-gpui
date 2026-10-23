# Search

Code: `src/panels/search.rs`, `src/panels/search/`

- **Two modes, as the TUI's search**: "Names" finds a group, project, workspace or tab by name as you type, from the session already in memory; "In sessions" finds where some text was said (`cornercase search --json`): agents' transcripts and every pane's history, grouped by pane. `Tab` switches, the arrows move, `Enter` or a click goes there.
- **The query is not kept once the sheet closes**: each opening starts afresh in Names, as the TUI does.
- **Names follow the sidebar's order**: projects gathered under their group where the group first shows. A group's status is its loudest project's; choosing a group expands it and shows its first project.
- **Ranking**: exact, prefix, a word's prefix, anywhere, then the query's letters in order (fuzzy, each as early as it comes); ties by kind (group, project, workspace, tab), then the sidebar's order. An empty query finds nothing, as in the TUI. At most 200 names are listed.
- **A workspace is also found by its branch, and a tab by its workspace's name and branch**, but the highlight lands on the name only when the name itself matched; a worktree's `⎇ ` prefix shifts it.
- **Case folding keeps byte ranges**: each character is lower-cased one for one (a character whose lower case is longer keeps its first), so matches map back onto the original text for highlighting.
- **Sessions searches are debounced** (220 ms) because each one reads every pane; a generation counter makes an older answer moot once the query or mode changes. `Enter` before the answer arrives goes to the first match when it does.
- **Matches are grouped by pane**, panes in the order their first match comes (the sidebar's), matches in the server's order (newest first). The output caps matches (20 a pane, 300 in all); `capped` says some were left out.
- **Opening a match opens its pane only**: scrolling to the match needs #467 upstream, and the status line says so.
- **Times are RFC 3339** (`2026-10-23T17:48:54.840Z` or with an offset), parsed by hand with the queue's days-from-civil, so no date crate is needed; fractions of a second are dropped.
