# Usage

Code: `src/panels/usage.rs`, `src/panels/usage/`

The Usage sheet shows what agents did in cornercase (`cornercase stats --json`: how much they worked in parallel, the counters per window, a heatmap of the last 26 weeks, the totals per agent and the card link from `stats --share`) and how much of each plan is left (`cornercase usage --json`: Claude Code's and Codex's limits with their reset times), for this computer, one machine or all of them.

## Loading

- **It opens on Stats, like the TUI's Usage dialog, and asks for limits only on the Limits page**: each probe runs `claude` or `codex` for a few seconds. Opening the sheet reads the page shown; switching pages reads again when the answer is older than 30 s (stats) or 60 s (limits).
- **Limits are read cached first** (`usage --cached --json`) so the last answers show at once while the probes run; if the fresh answer arrives first, it wins and the cached one is dropped.
- **Ages and reset times are relative**, so the sheet redraws every 20 s while it shows.
- **The card link is made locally** by `stats --share`: nothing is uploaded. It is the link of what is picked (`stats --machine vps --share`), and under `all` of the sum, with the merged pull requests only when the server keeps them.
- **A partial answer is an answer**: `cornercase usage` exits 3 when it printed every plan but some agent could not be asked (#472 upstream), each failure in its plan's `error`, or with `--machine` when a machine could not be read, the rest being still the answer; `stats --machine` exits 0 and says so in its fields. The panel runs it through `store::run_partial` (`cli::run_partial`), which takes exit 0 and 3 as success and any other status as an error from standard error.
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

## Every machine

- **A row above the pages picks `this computer`, each machine or `all`** (upstream's picker, its `design/stats.md`), and every page shows what it picked; By agent reads the same stats as Stats. It starts on `all`, as in the TUI. With no machine, or only entries with a `problem` (no link, so nothing to ask), the row is not drawn and the sheet is what it was before. The names come from `Store::machines` (`design/machines.md`). A picked machine that leaves the list shows `all` without forgetting the pick, so one removed and added back is picked again, as in the TUI. On another server the first choice and the first part carry that server's name, as in the Machines sheet: `this computer` would name the wrong machine.
- **The row wraps where it does not fit**, so every name stays whole and clickable. The TUI cuts the names because a terminal row cannot wrap. A dropdown was rejected, as there: the row shows what can be picked at a glance, one click from each.
- **The pick stays while the sheet lives**: with the panel, which `Root` keeps until the window moves to another server. The TUI keeps it for the server's run.
- **One reading per pick** (`Readings`, its stats and its limits), not one reading asked again: an answer lands in the slot of the pick that asked, so one that comes after another pick was chosen cannot show under it, and going back shows the last numbers at once while newer ones come. A pick follows the rule of the pages: older than 30 s (stats) or 60 s (limits), it reads again.
- **Limits come cached first**, as for this computer (`usage --cached --machine …`), and the fresh answer replaces them. With nothing cached, `--cached --machine NAME` fails and the sheet waits for the fresh one; `--cached --machine all` still answers, each machine with why it has no numbers.
- **Stats come cached first too** (`stats --cached --machine …`, cornercase 0.13.5, #675 upstream), so a machine or `all` opens on numbers rather than waiting for the ask, at most the server's 15 s per machine. With nothing cached, `--cached --machine NAME` fails and the sheet waits for the fresh answer with a spinner; `--cached --machine all` still answers, a machine with no copy left out with `not read yet`. From the second read on, the slot's numbers stay up while it reads. This computer's stats never wait and `--cached` without `--machine` changes nothing, so `this computer` asks once.
- **A link that changes reads the page again** (`links`, compared whenever `Store` notifies): an answer says how each machine stood when it was read, and the sheet may stay open. Without this, one that dropped would show no `seen`, and one that came back would keep showing it until Refresh; at the window's start, `still connecting` would stay. Rejected: laying the feed's state over the answer without asking, since a machine back from being down would then give the ssh error of its last ask as the reason for its old numbers. The cost is this computer's probes running again on Limits, one read at a time.
- **The top right says `asked just now` when a machine in the answer is old** (not linked, failing or unread), rather than `updated just now` above a part that says `seen 5 min ago`. Ages use the TUI's words of this dialog, `just now` and `5 min ago`, not the Machines sheet's seconds, since the sheet redraws every 20 s.

### Stats of every machine

- **The numbers are the server's**: this window adds nothing, and it would be wrong to, since a machine that is down is summed from its copy with its blocks moved on by the server. The server adds counters field by field, which is hour by hour because every server's windows are whole epoch hours; busy time adds up too, so `× in parallel` under `all` is summed agent time over summed busy time; `since` is the oldest; merged pull requests are never added, and are dropped when the machines list different accounts.
- **Most at once is the largest peak of one machine**: whether two peaks fell in the same hour is not known, so a sum would be a guess. The sum says so below its rows, always.
- **Notes below the rows use the TUI's words**: `vps: seen 5 min ago` for a copy of a machine not linked, `vps: <why> · numbers read 5 min ago` for a linked one whose last ask failed, `down: not summed · <why>` for one with no numbers, which under the cached answer may be `vps: not summed · not read yet` until the fresh one comes. One machine picked says the same of its copy, without `not summed`. By agent shows these too, but not the peak, which it does not show. With no numbers at all, one machine picked gets the failure card, with the command's own sentence (`no stats from `down`: …`).

### Limits of every machine

- **Under `all`, one part per machine**, this computer first and then the answer's order (`config.json`'s), each headed by its name, each listing that machine's accounts with its labels and `●`. One machine picked shows its part alone, headed the same way; `this computer` picked shows the cards as before, with no heading.
- **Old numbers never pass for fresh ones**: a machine not linked shows its last copy with `seen N min ago` beside its name; a linked one whose last ask failed keeps its numbers under `<why> · numbers read N min ago`; one with no copy says why instead of rows. Why is red, what the other sheets use for what failed, except what is under way: `still connecting` takes a spinner, as does a linked machine with no copy while its fresh answer comes, where the cached answer says `not read yet`. Only a linked machine is asked, so the cards of one that is down keep their `updated 5m ago` while the others say `loading…`.
- **One machine with no copy**: `usage --machine NAME` fails, exit 1, and its message is the only place that says why. The failure card shows it whole (`no usage from `down`: …`, without `cornercase usage:`): cutting out the reason would be reading text. Under `all`, the same reason comes in a field.
- **Rows of two machines with the same `identity` become one**, only under `all`, under the first machine that gave it: it keeps that machine's label and `●`, with `on this computer, vps` on a dimmed line of its own. Its numbers are the latest reading's, whole, never added: a limit is the provider's and both machines read the same one. A row without an identity never merges, since two equal rows are better than one summed wrongly (upstream's rule, its `design/plan-usage.md`); nor do two rows of one machine, though each can join its own row of another. The server never merges: it sends the identity, and the client that shows several machines decides. A machine whose rows all went up keeps its heading and says so.
- **Readings are compared on the server's clock**: a machine's row was read at the machine's `read_at`, when this server read the answer, minus the row's `age`; this computer's at its own `read_at`. A row's `read_at` under a machine is that machine's clock, so one whose clock is off would win or lose by it, and it is not used. Its `updated 5m ago` comes from the same time. The TUI counts ages on its clock instead; here no clock of this window enters the comparison, so a window on another computer changes nothing.

### Checked live

On 2026-11-06 with cornercase 0.13.1, on Linux x86_64 under Xvfb with lavapipe, with a fake `ssh` between two servers on one box (no real sshd, no macOS, no Windows).

- What the server adds, from a `stats.json` written on each server: agents 2 + 3 = 5, prompts 6 + 10 = 16, agent time 1 h + 1 h 30 = 2 h 30, busy 30 + 30 = 1 h (so 2.5×), peaks 3 and 5 = 5, `since` the older, and merged pull requests this computer's, the other server having none.
- A fake `claude` on each server answering the probe's `initialize` (a first-party account) and `get_usage`, so the real servers worked out `identity`, `age` and the limits; the fixtures in `tests/fixtures/usage/` are those answers, with a neutral login. The real row merged: `on this computer, vps`, `vps` left with its Codex.
- In the window: `this computer` with no heading, `vps`, `all` and `down`, on Limits, Stats and By agent, with the cached answer first (slow fakes: `loading…`, `Asking vps…` and `down`'s reason at once). Ssh refused with the link up, then Refresh: `<why> · numbers read just now`. The link cut: `seen just now`, read again without a press, and back. `down` picked: its failure card, Try again.
- `vps` picked and removed from `config.json`: `all`, the row without it; added back: `vps` again. No machines left: no row, this computer's numbers. Eleven machines at 1400 and 760 px: the row wraps. Closing and reopening the sheet keeps the pick and the page. Copy card link under `vps`. A window with `--remote vps-host`, whose server lists a machine: `vps-host` first in the row and on the first part.
- Not checked: a machine `older` or `connecting` through this sheet (the words come from the same fields), a real ssh failure other than the fake's line, a real Claude Code account or Codex's limits on two machines, and this sheet on macOS or Windows.

On 2026-11-07 with cornercase 0.13.19, the same bench, stats cached first.

- With each new ssh connection to `vps` delayed by 6 s, `stats --cached --machine vps` took 0.07 s and `stats --machine vps` 6.1 s. In the window, `all` and then `vps` showed the last numbers at once with `Refreshing…` beside them, and the next count when the answer came.
- `vps` held after one read: `all` and `vps` with its numbers and `vps: seen just now`. `down` picked alone: the failure card, `no stats from `down`: ssh: …`.
- The server restarted, so it held no copy, with `vps` delayed by 13 s: `all` with `vps: not summed · not read yet` and `down`'s reason, and `vps` picked with `Reading the stats…` until its numbers came.
