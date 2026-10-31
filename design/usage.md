# Usage

Code: `src/panels/usage.rs`, `src/panels/usage/`

The Usage sheet shows what agents did in cornercase (`cornercase stats --json`: how much they worked in parallel, the counters per window, a heatmap of the last 26 weeks, the totals per agent and the card link from `stats --share`) and how much of each plan is left (`cornercase usage --json`: Claude Code's and Codex's limits with their reset times).

## Loading

- **It opens on Stats, like the TUI's Usage dialog, and asks for limits only on the Limits page**: each probe runs `claude` or `codex` for a few seconds. Opening the sheet reads the page shown; switching pages reads again when the answer is older than 30 s (stats) or 60 s (limits).
- **Limits are read cached first** (`usage --cached --json`) so the last answers show at once while the probes run; if the fresh answer arrives first, it wins and the cached one is dropped.
- **Ages and reset times are relative**, so the sheet redraws every 20 s while it shows.
- **The card link is made locally** by `stats --share`: nothing is uploaded.
- **A partial answer is an answer**: `cornercase usage` exits 3 when it printed every plan but some agent could not be asked (#472 upstream), each failure in its plan's `error`. The panel runs it through `store::run_partial` (`cli::run_partial`), which takes exit 0 and 3 as success and any other status as an error from standard error.
- **A probe's error is shown as cornercase words it**, with no note of our own: the Codex limits bug that once needed one is fixed upstream (#462, checked with cornercase 0.12.225).

## Numbers as the TUI prints them

- **Parsing is lenient** (every field defaults), so a newer cornercase with more fields or fewer still reads.
- **The formatting rules copy the TUI's** so both clients print the same numbers: durations in minutes, then hours and minutes; counts whole below 10,000, then `12.3k`, `4.5M`; tokens in k, M or B; `tokens in` is uncached input plus cache writes; reset times round up to the minute.
- **A limit's severity is the server's**; a name this client does not know falls back to the percentage, 75 and 90, as cornercase does.
- **The parallel factor needs a minute of busy time** in the last 24 h; below that the hero says so instead of showing a meaningless ratio.
- **Merged pull requests marked `+` are a lower bound**: some account could not be asked, and the table lists which.
- **Agents keep cornercase's order** (Claude Code, Codex, Gemini CLI, opencode, Grok Build, Cursor CLI, Copilot), unknown ones last by name; idle agents are left out. cornercase keeps no totals per project or model, so the panel shows none.
- **The heatmap** shows the last 26 weeks, one block per rolling 24 hours, seven to a column, newest bottom right. A block's level is 0 without agent time, else 1 to 4 by the nearest-rank quartiles of the positive blocks shown, ties sharing their level. The levels use the TUI's blues (palette 24, 26, 33, 75); agents' colours come from the terminal palette.
- **Fractions for bars are `f32`** from `f64` seconds and percentages: the precision lost is far below a pixel.
- **Refresh and Copy card link read `Refreshing…` and `Copying…`** with a spinner while they run, and the cards waiting for their first answer show one.
