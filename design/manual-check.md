# Manual check

Code: `tools/manual-check.swift`, `tools/machine-bench.sh`, `tools/machine-bench-test.sh`, `.github/workflows/ci.yml` (the shells of the bench tests); the window reads the variables below at start (`prefs.rs`, `app.rs`, `self_update.rs`, `theme/current.rs`, `panels/browser/chrome.rs`)

How to check a change in the running window on macOS, the way that worked (#113). The rules are in `AGENTS.md`'s Manual check. Linux in Docker is in `design/linux.md`; running the Windows window needs Windows (`design/windows.md`). Every command here was run as written on 2026-11-10 except what is marked untried (the last section says with what); `<pid>` and `<id>` are yours to fill.

## A server of its own

- **The folder and the environment.** One short folder, and a different one for each check running at the same time. Every later command starts with `. /tmp/cc-check/env`, because an agent's shell keeps no variable from one command to the next. Put the path of the `cornercase` to run on the `CORNERCASE_BIN` line (the next bullet says how to get another release than the installed one).

  ```sh
  mkdir -p /tmp/cc-check/proj && git -C /tmp/cc-check/proj init -q
  cat > /tmp/cc-check/env <<'EOF'
  export CORNERCASE_SOCKET=/tmp/cc-check/server.sock
  export CORNERCASE_GPUI_SETTINGS=/tmp/cc-check/settings.json
  export CORNERCASE_GPUI_BROWSER_PROFILE=/tmp/cc-check/browser
  export CLAUDE_CONFIG_DIR=/tmp/cc-check/claude
  export CORNERCASE_BIN="$(command -v cornercase)"
  unset CORNERCASE_PANE CORNERCASE_SERVER CORNERCASE_REMOTE
  EOF
  ```

- **Why each line.**
  - **The socket** isolates the server: cornercase's `config.json`, its session and its flows sit beside it, and so do `server.log` and `server.lock` from the first start. Only the socket isolates a server; `CORNERCASE_ROOT` does not (`design/machines.md`).
  - **A short path**, since a socket's path must fit (`design/remote.md`).
  - **`CORNERCASE_GPUI_SETTINGS`** moves this window's settings, themes, saved layout and relaunch note (`design/settings.md`, `design/theme.md`, `design/window.md`), so the user's are never written. In the run, Settings showed `/tmp/cc-check/config.json` and the window wrote `theme.json` and `layout.json` there.
  - **The throwaway git folder** becomes the server's first and only project: `status` listed that one.
  - **`CORNERCASE_GPUI_BROWSER_PROFILE`** moves the Browser panel's Chrome profile. It is not under the settings file's folder, and a second window attaches to the Chrome its profile already runs (`design/browser.md`), so without it a Browser panel here would drive the Chrome profile that the user's own cornercase-gpui window runs (not their everyday browser).
  - **`CLAUDE_CONFIG_DIR`** is the agent's account in an agent's shell; left as it is, anything started on the private server writes into it, as a fake agent once did (`design/machines.md`), and unset, into the user's default Claude folder. A folder of its own is removed with the rest. This replaces the unset in `design/machines.md`'s bench note for #95, which is kept as the record of that check.
  - **The unsets.** An agent's shell inside cornercase names its own pane and server, which commands must not aim at.
  - **`CORNERCASE_BIN`** is the binary the window and every command run (`design/connection.md`), so both are one release.
- **Another release than the installed one, without installing it.** Download it under the checkout's `target/` (ignored by git), check its checksum and unpack it. The archive is per platform (`cornercase-aarch64-apple-darwin` on Apple silicon), and one glob fetches the archive and its `.sha256`. `shasum` prints `OK` and then `WARNING: 1 line is improperly formatted`, from the checksum file's empty last line; it is harmless.

  ```sh
  v=0.13.32
  mkdir -p target/cornercase-$v && (cd target/cornercase-$v &&
    gh release download v$v --repo usecornercase/cornercase-terminal --pattern 'cornercase-aarch64-apple-darwin.tar.gz*' --skip-existing &&
    shasum -a 256 -c cornercase-aarch64-apple-darwin.tar.gz.sha256 &&
    tar -xzf cornercase-aarch64-apple-darwin.tar.gz &&
    ./cornercase-aarch64-apple-darwin/cornercase --version)
  ```

  `target/cornercase-$v/cornercase-aarch64-apple-darwin/cornercase`, as an absolute path, goes in `CORNERCASE_BIN`. `tools/cornercase-cli.sh` takes the same variable to make `docs/cornercase-cli.txt` from that release.
- **A released build as the "before"** (2026-11-13, 0.1.72, 0.1.73 and 0.1.78): `gh release download vX --repo usecornercase/cornercase-gpui --pattern 'cornercase-gpui-universal-apple-darwin.dmg*'`, `shasum -a 256 -c` its `.sha256`, `hdiutil attach -nobrowse -readonly -mountpoint <mnt> <dmg>`, copy `<mnt>/*.app/Contents/MacOS/cornercase-gpui` to `<folder>/vX/target/debug/cornercase-gpui`, `hdiutil detach <mnt>`, then `codesign --force -s -` on the copy (see "A copy of a release's binary" under "A frame's cost"). The helper accepts it by that path, and `CORNERCASE_GPUI_OPEN` and `CORNERCASE_GPUI_SETTINGS` work in a release's binary. Running 0.1.72, 0.1.73 and the branch on one input tells which merge changed a drawing.
- **Build and start.** `$!` must be the window's own pid, so the `cd` is a statement of its own, never `cd … && program &`. The first build compiles every dependency, a few minutes.

  ```sh
  cargo build --locked
  swiftc -O -o target/manual-check tools/manual-check.swift
  repo=$PWD; . /tmp/cc-check/env; cd /tmp/cc-check/proj
  CORNERCASE_GPUI_OPEN=Settings "$repo/target/debug/cornercase-gpui" > /tmp/cc-check/window.log 2>&1 &
  echo $! > /tmp/cc-check/window.pid
  ```

  Then, from the checkout, check that the pid is the window's and that it shows a window (retry for a few seconds): `ps -o comm= -p "$(cat /tmp/cc-check/window.pid)"` names `target/debug/cornercase-gpui`, and `target/manual-check window "$(cat /tmp/cc-check/window.pid)"` prints a line. `. /tmp/cc-check/env; "$CORNERCASE_BIN" status` lists one project, the throwaway folder, and `"$CORNERCASE_BIN" --version` the release you chose.
- **Driving that server from the shell needs no press.** `"$CORNERCASE_BIN" status` lists the ids; `send --pane <id> --enter "<text>"` types into a pane, and `status` showed the new tab after a ⌘T.

## Looking at the user's server

`AGENTS.md` says when. The window opens on a throwaway tab of that server (the id from `cornercase status`), with its settings and Browser profile in a folder of its own and a pid kept to stop it by. **Untried**, as the end of this section says:

```sh
repo=$PWD; mkdir -p /tmp/cc-real; cd /tmp/cc-real
env -u CORNERCASE_SOCKET CORNERCASE_GPUI_SETTINGS=/tmp/cc-real/settings.json CORNERCASE_GPUI_BROWSER_PROFILE=/tmp/cc-real/browser "$repo/target/debug/cornercase-gpui" --tab <id> > /tmp/cc-real/window.log 2>&1 &
echo $! > /tmp/cc-real/window.pid
```

Stop it with `kill "$(cat /tmp/cc-real/window.pid)"` and remove the folder.

It costs more than it looks: the window gives the server its colours as it starts (`start-server --theme -`), and upstream applies that to a server that already runs (its design note on colours, under `design/` in the clone). The user's terminals then wear the debug window's theme until their own window gives theirs again: at its next start, a reconnect or a theme change (`design/theme.md`). Copying the user's `theme.json` and `themes/` into `/tmp/cc-real` first gives the same colours.

This was not tried for this file, command included: a team's checks run inside the user's server.

## What needs no press

These reach a screen without a press:

- `CORNERCASE_GPUI_OPEN=<panel>`, the panel's title in any case (`design/window.md`).
- `--tab`, `--workspace` or `--project` with an id at start.
- `CORNERCASE_GPUI_FONT_SIZE` and `CORNERCASE_GPUI_OPACITY` (the README's table).
- A theme without a press (2026-11-13): `theme.json` beside `CORNERCASE_GPUI_SETTINGS`, `{"theme":"cornercase-light"}`, switches a running window live, and `cornercase-dark` back (`design/theme.md`).
- `CORNERCASE_GPUI_RELEASES_URL=<url>` makes a debug build ask that fake release, for the update chip and dialog (`design/self-update.md`).
- A `cornercase` command to that server.
- A ⌘ key with `key`, below, which posts no mouse event.

## Focus, keys, presses and scrolls

`tools/manual-check.swift` does what no stock command does. It compiles with `swiftc`, and whoever runs it needs Accessibility in System Settings › Privacy & Security (and Screen Recording for captures); it says so when it may not post. In every call `<pid>` is the window's own: never System Events (`AGENTS.md`).

```sh
target/manual-check window <pid>                  # <window id> <x> <y> <width> <height> <scale>, in points
target/manual-check front <pid>                   # brings that process to the front
target/manual-check size <pid> <width> <height>   # the window's size, in points
target/manual-check key <pid> <key> [cmd] [shift] [alt] [ctrl]
target/manual-check hold <pid> <key> [ms]         # a held key (below, Secure event input)
target/manual-check paste <pid> <text>            # ⌘V of made-up text, the clipboard given back
target/manual-check text <pid> <text>             # made-up text, a key down and up per grapheme carrying it
target/manual-check press <pid> <x> <y> [right]   # a point from the window's top-left, in points
target/manual-check drag <pid> <x0> <y0> <x1> <y1> [right] [hold <ms>]   # press at the start, move to the end, release
target/manual-check scroll <pid> <x> <y> <dy> [dx]
```

- **Exit status.** 0 done; 1 an error (no window, no permission, refused); 2 a wrong word, a point outside the window, or a pid that is not a `target/debug/cornercase-gpui` (the user's own window has the same name, so every command but `window` refuses it); 3 the process is not in front, another window covers the point or the point is under the Dock or the menu bar, with nothing posted. Only the Dock's own full-screen backdrop is ignored when looking for a covering window.
- **Drag.** `drag` runs the checks of `press` at both points (inside the window, not under the Dock or the menu bar, the process in front and topmost there) before any button is pressed (the hover's mouse move is posted first, as for `press`), hovers the start for 150 ms (a pane's handle fades in) and checks it again, presses, moves in 12 steps over about 250 ms, waits 250 ms (a drop zone glides in 170 ms), releases, and prints the release's time as seconds since the epoch with three decimals, so a capture taken after it can be placed against a timer such as the move's 1.5 s. Exit 3 then means no button was pressed. The pointer goes back as for a press. `hold <ms>` (0 to 10000, a bad one exits 2 with nothing posted) adds that to the wait before the release, to capture a drag's preview: `target/manual-check drag <pid> … hold 1500 & sleep 0.9; screencapture -x -o -l <id> f.png; wait`. Press a tab at 31 to 34 pt from the top: at its middle (21 pt) a drag moved the window on macOS and the tab stayed (2026-11-13, #176).
- **Points and pixels.** A capture's pixels divided by the scale `window` prints are points: the 2560×1600 capture of a 1280×800 window at scale 2 puts a pixel (60, 1532) at the point (30, 766).
- **⇧⌘K** is posted as `key <pid> k cmd shift` and opens "Connect to a server" from the terminal, from a page and from the address (2026-11-13, #184); the posted flags were never at fault. What #182 saw, a key that did nothing with a machine's place picked and the Browser open, was the window's: see Checked, #184.
- **Keys** are `a` to `z`, `return`, `escape`, `tab`, `space`, `delete`, `quote` and the arrows only, posted down and up with `CGEvent.postToPid`: key codes are positions on a US keyboard, so punctuation would type another character on another layout, and the user's is Spanish (`design/browser.md`). ⌘F opened Search (`key <pid> f cmd`) and ⌘T made a tab. `text` posts a text's characters as keys carrying them, for letters no key here makes (made-up text only), and `quote` is a position: `´` (a dead key) on Spanish ISO, `'` on ABC. **A key needs no front**: with another debug window in front, ⌘F reached the window behind it (grey traffic lights) and opened its Search. Only presses and scrolls need `front`.
- **Why the HID tap, with dates and versions.** Posted to the pid, mouse events only moved the hover in gpui 0.2.2 on macOS 26 (#102, #103, #104; the notes in `design/issues.md` and `design/remote.md`). Posted to `.cghidEventTap` with the debug window in front they press (#109's captures and #95's bench, `design/machines.md`, 2026-11-10), and pixel scrolls scroll what was sent. Run again on 2026-11-10: a press on the rail's Settings icon opened the sheet and a press on Appearance changed its page; `scroll <pid> 720 400 -300` over the page moved it up, so **a negative `dy` shows later rows**.
- **Its cost.** A press or a scroll moves the real pointer, needs the window in front (keys the user types meanwhile go to it), and anything covering the point would take it. So the helper checks that the process is in front and that its window is the topmost at the point, posts a move, waits for the hover, checks both again just before the button goes down (exit 3 then, with only the move posted), does one per call and puts the pointer back. The pointer was where it started after each one here.
- **Refusals seen.** A point outside the window, `press … middle`, `scroll … abc`, an unknown key or command, and a pid that is not a debug window (`front` on a shell, `press` on pid 1) each exit 2 with a sentence and nothing posted. With two debug windows on two private servers and the first in front, `press` and `scroll` on the second exit 3 (not in front) with nothing posted, and `front` on the second then a `press` on it worked.
- **`size`** worked: 1100×700 and back to 1280×800, and `window` showed it.
- **A press that does not focus a field** (2026-11-13) tells a field laid out at no width from a colour problem: its hit area is its own bounds (#189). The viewer's Find button opens the find bar with no ⌘F in the viewer (`design/files-panel.md` says where ⌘F goes).

## Captures

- `screencapture -x -o -l <window id> <file>.png`, the id from `window`; it needs no focus. At 1280×800 and scale 2 the file is 2560×1600 pixels.
- **A plain prompt first**, in the pane behind the window: `"$CORNERCASE_BIN" send --pane <id> --enter "clear; exec env PS1='\$ ' /bin/sh"`. Plain sh reads no `.zshrc`, and a prompt theme rewrites a zsh `PS1` on every prompt. The tab then reads `bash` (macOS's sh), the status bar shows only the throwaway folder and the prompt is `$ `.
- **Crop** in pixels: `sips -c <height> <width> --cropOffset <y> <x> in.png --out out.png`.
- **Before and after side by side** (2026-11-13): `ffmpeg -i before.png -i after.png -filter_complex vstack pair.png`.
- A user, host or home folder can still be in another pane's prompt, a tab's title or the sidebar. A capture with the glass's blur needs `-R` of the window's rectangle (`design/website.md`).

## Secure event input

A masked field turns macOS's secure event input on while it has the focus (`design/widgets.md`, #188). It is process-wide and the user is working on the same Mac, so turn it on only in your own debug window, for the seconds a check needs, and look after every run.

- **Who holds it**: `ioreg -l -w 0 | grep -o '"kCGSSessionSecureInputPID"=[0-9]*'` prints the holder's pid, twice, and nothing when nobody does (seen on 2026-11-13, macOS 26.6.2). A pid that is not yours may be the user's own app: look for yours only.
- **A key posted with `postToPid` still reaches a window that holds it** (2026-11-13): `text` and `key` typed into the masked field with secure input on.
- **The window losing the front without a press**: `key <pid> h cmd`, then `front <pid>` (it refused for a moment after ⌘H and then worked). `front` is also flaky while the user works in another app: chain it with the call that needs it (`front <pid> && press …`).
- **A crash**: `kill -KILL <pid>` stands in for it (no crash dialog, unlike `kill -ABRT`); `ioreg` then names nobody.
- **`hold <pid> <key> [ms]`** posts a key down, auto-repeats (800 ms by default) and a key up. It opened the press-and-hold accent popup only in a masked field with secure input on, before that field turned press-and-hold off (2026-11-13); in a plain field, ⌘G's prompt, the terminal and every field of 0.1.80 it inserts one letter and opens nothing, so "no popup" is not a regression, and a real held key may differ. A masked field now repeats the letter. A popup is another process's window: capture it with `screencapture -x -R<x,y,w,h>`; Escape closes it. **`paste <pid> <text>`** is ⌘V with made-up text: it reads every item of the clipboard into memory (and exits 1 with nothing done if a type cannot be read back), puts the text on it marked transient (clipboard-history apps skip it), posts ⌘V, waits 300 ms, writes the items back and prints only the counts; if it cannot give them back it exits 1 and says so, and the person running it tells whoever owns the clipboard at once.
- A capture shows the frame before the last key: take it a second after, and twice.

## Cleaning up

```sh
. /tmp/cc-check/env; pid=$(cat /tmp/cc-check/window.pid)
server=$(lsof -t "$CORNERCASE_SOCKET" 2>/dev/null | grep -vx "$pid" | head -1)
left=$( { echo "$server"; pgrep -P "$pid"; [ -z "$server" ] || pgrep -P "$server"; } | grep . | sort -u | tr '\n' ,)
kill "$pid" 2>/dev/null
"$CORNERCASE_BIN" kill-server
"$CORNERCASE_BIN" status
[ -z "${left%,}" ] || ps -o pid=,command= -p "${left%,}"
rm -rf /tmp/cc-check
```

`left` is the server, its panes' shells and the window's own children. `status` must now fail ("no cornercase server is running"; it never starts one) and `ps` list nothing. If some remain, end them by pid.

## Beyond one window

Their notes are in the area files, except what the bench does (**Another machine**, below):

- a fake agent: the bench's `--agent`; fakes that answer probes or write session files: `design/flows.md`, `design/context-dialog.md`;
- a `CORNERCASE_BIN` wrapper that logs: the bench's `--log`; one that answers commands: `design/flows.md`, `design/pull-requests.md`;
- two private servers and an `ssh` stand-in for another machine: **Another machine** below;
- Linux under Xvfb (`design/linux.md`);
- a fake `gh` whose pull request merges (#126): `design/flows.md`.

## Another machine

`tools/machine-bench.sh` makes the bench for checking another machine and takes it down (#119): two private servers, "here" (the window's) and "there", an `ssh` stand-in that runs "there", "there" linked as machine `vps` of "here", an `env` file, a copy of a cornercase release per side and, on request, a fake agent and a log of what the window ran. `tools/machine-bench-test.sh` tests its refusals and safety lines against a stub, never a real cornercase (`sh tools/machine-bench-test.sh`, or `SH=dash dash …`). CI's Linux job runs it on every push and pull request (#123, `design/linux.md`): under dash, which is Linux's `sh`, and under bash started as `sh`, which puts bash in POSIX mode as macOS's `sh` (bash 3.2) is, so a line that one of the two reads its own way fails there; `sh` alone would be dash twice on the runner, and zsh, which the env test also tries when one is installed, is not on it. On a Mac, run it by hand when the script changes. What was run is in Checked; the rest is untried.

From the checkout, with the 0.13.32 archive downloaded as in "A server of its own" and `cargo build --locked` done:

```sh
repo=$PWD
"$repo/tools/machine-bench.sh" up /tmp/cc119 --here "$repo/target/cornercase-0.13.32/cornercase-aarch64-apple-darwin/cornercase" --agent --log
. /tmp/cc119/env; there status --panes; here machines
. /tmp/cc119/env; cd /tmp/cc119/here/here-proj; CORNERCASE_GPUI_OPEN=Machines "$repo/target/debug/cornercase-gpui" > /tmp/cc119/window.log 2>&1 &
echo $! > /tmp/cc119/window.pid
kill "$(cat /tmp/cc119/window.pid)"; "$repo/tools/machine-bench.sh" down /tmp/cc119
```

`up` prints the summary with the window's line for this checkout, which sources the `env` itself. One folder per bench, named after the issue. Every later command starts with `. FOLDER/env`; `here` and `there` also work by their full path (`FOLDER/bin/here`). Both run from their project folders, so give them absolute paths.

- **Words and exit statuses.** `up FOLDER [--here BIN] [--there BIN] [--agent] [--log]`, `status FOLDER` (read-only) and `down FOLDER`; `-h` says the rest. 0 done, 1 failed on the way (a command failed, the link did not come up, processes left at `down`), 2 wrong usage or a refused folder or release, with nothing done. A failed `up` stops its servers and keeps the folder for its logs.
- **A release per side.**
  - The side under test is "here": a fold's release is this computer's (#120), so a 0.13.21 "here" (`--here` the installed binary: `up` copies it, never pointing at the user's server) with a 0.13.32 "there" is the bench for the older behaviour. `--there` defaults to `--here`'s release; an older "there" makes the link `older`, which `up` says and accepts.
  - The releases are copies, so an `update` there (the older page's Update…, `machines update`) replaces the bench's copy with a download, never the installed one (`update` replaces the binary it runs).
  - The installed cornercase is only asked `--version`. For another release, see "A server of its own".
- **Why each line.**
  - **Each side is a server of its own**: its socket, its `CLAUDE_CONFIG_DIR` and the unsets are "A server of its own"'s, set by the script on every command it and its files run, so the caller's `CORNERCASE_SOCKET`, `CORNERCASE_REMOTE`, `CORNERCASE_PANE` and `CORNERCASE_SERVER` never reach one.
  - **The folder is new, under `/tmp` and short**: a plain name (letters, digits and `. _ / -`, no `.` or `..` part), not `/`, the home folder or what holds it, not inside a git work tree, with no symbolic link taking it elsewhere, and short enough for a socket. That keeps the bench off the user's files.
  - **Private folders (700)**, which cornercase requires of a socket's folder.
  - **The mark**, `.machine-bench`, written first and naming the folder: `status` and `down` refuse a folder without it, or one copied from another (`rm -rf` runs only after that).
  - **The projects** are throwaway repositories with one empty commit, made with no global git config, so the user's hooks and signing stay out; each server starts from its own, so it is its first project.
  - **`worktrees_dir`** is inside the bench: its default is `~/.cornercase/worktrees`, where the user's real worktrees are.
  - **Every agent is pinned** to `/nonexistent/<name>` unless it is the fake `claude`, as upstream's e2e does (upstream #739): a built-in flow or probe would otherwise run the user's real `codex` on their login.
  - **"There" is another machine**: a fresh environment (`env -i`) and a `HOME` of its own, as upstream's e2e machine has, so its `~` defaults (the clone folder, worktrees) stay inside the bench and nothing of the caller's environment reaches it. "Here" keeps the user's `HOME`, so its other agents' default homes (`~/.codex`…) are the user's, and an Accounts capture on "here" can show them: a capture that leaves the computer must not.
  - **The destination is `vps.invalid`**, which never resolves: a command run without the env fails at the real ssh's name lookup.
  - **A second name, `there.invalid`, for the same server (#184).** The stand-in answers to `vps.invalid` (the machine) and `there.invalid`, so the window itself can be sent to the machine's server (⇧⌘K, type `there.invalid` with `target/manual-check text <pid> there.invalid`, `key <pid> return`; or start it with `--remote there.invalid`; the dialog's This Mac row brings it back, one press) and `here machines edit vps --destination there.invalid` points the machine at the same server by another name, without the link going down. Each name has its own `.held`, `.slow` and `.missing` files, so "the window on there" and "here's link to vps" are told apart, and `down` and `status` handle both. A fixed second name, not an option: it keeps the safety line by construction (`.invalid` never resolves; a dotless name such as the hand-edited `vpsbench` of 2026-11-12 can resolve through a search domain or the user's ssh config), needs no refusal rules, and one name can be written here. The dot is no obstacle since #190: `text` types it.
  - **`down`** (it also looks for processes holding a server's socket, with `lsof` when there is one, so a wrapper that is not a copy under the folder cannot leave a server unseen) touches `ssh/vps.invalid.held` first (a stand-in sleeping in `.slow` would otherwise start "there" again), stops "here" before "there" (its link would start "there" again) with `kill-server --yes` on each side's own socket, waits 10 s and removes the folder. It never kills by name: it refuses (2, nothing done) while a window is on the bench (kill it by its pid first) and when its caller runs inside the bench's server, and when processes remain it lists them, keeps the folder and exits 1. A window is a process named `cornercase-gpui`, matched by file name: this checkout's path holds that name too.
- **The stand-in** (`bin/ssh`) takes the command lines upstream builds and runs the command there through `sh -c`. It answers `-G` with `hostname <destination>` and never reads the user's ssh config. It **forwards** `-N` with `-L` or `-R` (#106) and refuses, with exit 255, a master (`-O`), `-f`, `-D`, `-W`, a forward with a command or `-N` without one, no command and any other host. A forward is `exec python3 -I bin/forward "-L:SPEC,-R:SPEC…"`, so the pid stays the stand-in's, which cornercase 0.13.50 needs (a hold's tunnel is up when its ssh's own pid listens on `127.0.0.1:LOCAL`, checked on 0.13.50). `bin/forward` listens on loopback only (it refuses any other bind or target) and connects each client to the same loopback, since "there" is this computer too: so a port that is free "there" is almost always taken "here", and a tunnel lands on another local port (seen as the toast's parenthesis), and `-R` behaves as `-L`, enough for the Browser's reverse tunnel. It prints OpenSSH's own lines when the port is taken (`bind [127.0.0.1]:N: Address already in use`, `Could not request local forwarding.`; for `-R`, `remote port forwarding failed for listen port N`) and `channel 2: open failed: connect failed: …` on a refused connection, and ends on SIGTERM, which is how a hold ends it. It needs `python3` (255 and a line when missing). Rejected: a real sshd (keys, a config and a dependency, and it would add nothing to what the window sees). The forwarders belong to the bench: `status` lists them (pid and specs) and says whether `here/tunnels.json`, `there/tunnels.json` (cornercase's lists, for a TUI window run on the bench) and `window/remembered-tunnels.json` (this window's own, beside `settings.json`) exist, and `down` keeps the folder and lists any that remain, exit 1 (it never kills by name: end them by pid). `ssh/calls.log` holds each call, refused ones too. Files in `ssh/`, touched by hand:
  - `vps.invalid.held`: "Connection refused", as when the machine is off. New sessions are refused at once (a fold, a one-shot command); the link already open drops on `here machines retry vps` on macOS (`unreachable`, ssh's line under it) and comes back with `rm` and another retry. **On Linux that retry did not drop it**: end the link's `cornercase proxy` by its pid (`status` lists the bench's processes; never by name) and it goes `unreachable` the same way. `there restart --yes` leaves the server there stopped: under a hold, start it again with `there start-server`, or the answer is `no cornercase server is running`, not the stale one. The agent dies with the restart.
  - `vps.invalid.slow` (its content is the seconds, 8 when empty): each call sleeps, then refuses if a hold was placed meanwhile.
  - `vps.invalid.missing`: exit 127, `cornercase: not found`.
- **The fake agent.** `--agent` starts a fake `claude` on "there" before the link (#120 needed it so), sends it `spawn` so a child appears (`status --panes` shows its `parent`), and turns on "here"'s agents section, since the fold's arrow lives there (`here config set agents_section false` turns it off). In its pane, type `idle`, `working`, `blocked`, `done`, `error` (each an OSC 7501 state) or `spawn`. `send --pane P --enter spawn` prints `not confirmed: …` though the child appears: read `status`. It fakes no session file, model, context or conversation, so View context reads nothing, and `-p`, `auth` and `mcp` fail at once.
- **The log.** `--log` makes `CORNERCASE_BIN` a wrapper that appends each command line the window runs to `commands.log`, `--version` included (so it shows whether a fold carried `--boot`), quoted so it pastes back into sh. It does not show what goes through the window's running `requests --follow`, such as a pane move (#120).
- **A drop's timing** is read with `drag`, which prints the release's time, and captures after it. In a window of 1280×800 points the agents arrow of the first agent was at (80, 599) and the handle of the left pane at (559, 54); read them from a capture for another size.
- **What the bench is not.** A real sshd, network, `ControlMaster`, terminal (`-t`) or port forwarding; more than one machine; the user's ssh config; Windows (#70); presses without the pointer (#116). To pair two projects, give both the same `origin` (`https://example.invalid/…`).

## Quitting with ⌘Q

Decided from the code (`main.rs` `on_app_quit` and `leave`, `cli.rs` `quit`, `app/machine.rs` `take_back_colours_on_quit`, `panels/browser.rs` `quit`), 2026-11-13, #184. Quitting runs, in this order: the settings and layout flushes (beside `CORNERCASE_GPUI_SETTINGS`); `cornercase --remote <destination> theme --clear` for a machine's tab that was themed (through the window's `ssh`, the bench's stand-in); the Browser panel's own ssh forwards and reverse tunnel, then SIGTERM to the Chrome it launched on `CORNERCASE_GPUI_BROWSER_PROFILE` (SIGKILL after 1.5 s; an adopted one is only asked); the `plugin windows` stream; SIGTERM to every `tunnel hold` child; SIGKILL to every stream child; and `theme --clear` on the window's target. Every command keeps the window's environment except `CORNERCASE_PANE`, `CORNERCASE_SERVER` and `CORNERCASE_REMOTE`, so a local one reaches `CORNERCASE_SOCKET` and a `--remote` one runs the `ssh` on the window's `PATH`. **So a window started from the bench's `env` reaches only the bench's two servers, its own children and its own Chrome**: ⌘Q may be run on it, posted only to the debug window's pid after checking that it is that window (`key <pid> q cmd`, never System Events, never global). `kill` stays the way to stop a window; its reason in AGENTS.md (`theme --clear` taking back the colours the user's own window gave) applies to a window on the user's server, never to one on a server of its own. Run in #184 (W4, see Checked). After it, look at `commands.log` (only `theme --clear` and `--remote vps.invalid theme --clear`), the holds and forwarders (none), the profile's Chrome (none) and `layout.json` (only if the layout changed: it is written then).

## A frame's cost

How #115's frame times and CPU were measured (2026-11-10, release builds, cornercase 0.13.32 and 0.1.57 on a bench of its own; its scripts are not shipped):

- **`ZED_MEASUREMENTS=1`** makes gpui print `frame duration: …` to stderr for each frame it draws: counted per second and as a median, with no code of ours.
- **A release window's size** comes from `layout.json` beside `CORNERCASE_GPUI_SETTINGS` (`{"window":{"display":"<uuid of a previous layout.json>","bounds":{"x":60,"y":40,"width":1512,"height":912},"state":"windowed"}}`; without the display's uuid the size is cut to the screen), since the helper acts only on debug builds; `target/manual-check window <pid>` allows any pid and prints the size.
- **The front** is asked of that pid alone with the accessibility API (`kAXFrontmostAttribute`, as `front` does) and checked with `lsappinfo info -only pid "$(lsappinfo front)"`; another app taking it discards the run.
- **CPU** is the window's `ps -o time=` over 20 s, `sample <pid> 10` for where it goes (symbols are Rust-mangled: match `6window4step`, `prepaint_as_root`, `compute_layout`, `reuse_prepaint`).
- **Never while a build runs** (`pgrep -x cargo; pgrep -x rustc`): the bench waits for it. Others' work on the machine still moves the numbers (load average 2.6–6.5 here), so every cell is measured three times, interleaved, and a run not in front is repeated.
- **Faking working agents** needs no agent: the wrapper put in `CORNERCASE_BIN` runs the real `cornercase` and rewrites the JSON lines of `view` (agent, title, model, status `working` for the first N panes of a list, N from a file), and refuses to run unless `CORNERCASE_SOCKET` is the bench's; "Beyond one window" lists the script's own wrappers for a window and a machine. A pane that redraws three lines ten times a second stands for a working agent's output.
- **A copy of a release's binary** taken out of its `.app` is killed by the system (signature bound to the bundle): `codesign --force -s -` it.
- **The rail, tab strip and status bar step** used the same bench plus cell T (a flow workspace's tab shown: 7 tabs, 2 working).
- **The docked panels step** (#134) added a cell per open panel, started with `CORNERCASE_GPUI_OPEN=<panel>`, all with 5 working and the idle shell shown (B's tab), and TF with the flow's tab (T's): BC Changes (about 40 changed files in the throwaway folder: 30 modified, 4 deleted, 5 untracked), BD the same with the first file's diff open by one press, BF Files with a 477-line Rust file open in the viewer, BO TODO (20 items, 5 done, from `todo add`), BR Recap, BW Browser on its start page, TF Flow. What each shows was looked at in a capture (`screencapture -l <window id>`, the id first on `window`'s line) before it was measured; a panel showing "Reading the run…" or a loading note is not a measure.
  - **Faking a flow and a recap**: the wrapper in `CORNERCASE_BIN` also answers `flow runs`, `flow messages` and `flow files` (a run of the flow workspace's project: an orchestrator and six roles over three stages with a loop, 20 messages, 12 files in a folder of the bench, built from `tests/fixtures/flows/runs.json`) and `recap` (`tests/fixtures/recap/now.json` with its text repeated six times), and `view`'s rewrite also gives the first workspace's seven panes a `role`, which the Flow panel needs (`runs_flow`). Streams stay open until the window dies.
  - **Keys and the press** go through a copy of `tools/manual-check.swift` that accepts the bench's binaries in place of `target/debug/cornercase-gpui` (`proc_pidpath` resolves `/tmp` to `/private/tmp`, so both prefixes): ⇧⌘P, the file's name and Return for BF, one press on the first row at a point read from a BC capture for BD. The helper checks the front and puts the pointer back; an exit 3 means nothing was pressed, and the cell is "not measured".
  - **The rule and the after targets were written before the measure** (`design/widgets.md` has the rule), so a panel could not be chosen by its result.

## A press without the pointer

The issue's "Better still" (#113) asked for a debug-only way to send the window a click. It is **worth building**, and it is filed as #116, but **parked: it waits on gpui**. The HID route works but fights whoever uses the Mac, and teams of agents now check while the user works. #102, #103 and #104 left their presses unchecked, and #107 measured with a hook in an uncommitted copy of the code (`design/machines.md`). The HID route (`tools/manual-check.swift`) stays the way until this lands.

- **What was found** (2026-11-10, gpui 0.2.2, rustc 1.99.0): `Window::dispatch_event` is `pub`, but it returns `DispatchEventResult`, which gpui declares `pub(crate)`. Rust's type privacy refuses any expression of that type outside gpui, so the call does not compile here, and no `#[allow]` lifts it. #116 had assumed the method was usable because it is `pub`. gpui has no other way to hand a real window a mouse event: `dispatch_keystroke` takes keys only, and the test context's `simulate_*` work on the test platform only.
- **What would unblock it:** a gpui release whose `DispatchEventResult` is `pub` (Zed's `main` has it, and 0.2.2 is the newest on crates.io; `main` is a year of API changes away, so moving to it is not a bump). Until then the only way is to patch that one word through `[patch.crates-io]`, in a fork repository or a vendored copy. Neither was taken: a fork needs a new public repository and makes every build, releases included, fetch it; a vendored copy adds 8 MB in 184 files, its warnings are not capped as a dependency's are, and `app_changed` would have to list it. Native events in the window's process (AppKit, Win32, X11; Wayland has none) were rejected as unsafe platform code in three variants.
- **The design to build then:**
  - A debug build (`#[cfg(debug_assertions)] mod steps`, so a release neither reads the variable nor carries the code) started with `CORNERCASE_GPUI_STEPS=<file>` follows that file and plays each line added to it through `Window::dispatch_event`, the call gpui's platform code makes for every real event. No pointer, no front, nothing that can land on another window, and the same code on Linux and Windows.
  - The lines, in points from the window's top-left as the helper takes them: `move X Y`, `press X Y [right]`, `drag X1 Y1 X2 Y2`, `scroll X Y DY [DX]`; blank lines and `#` notes are skipped. Each plays at the helper's pace (hover, press, 12 drag moves holding the button), one step at a time. A line that cannot be read, or whose point is outside the window, becomes an error toast and a line on stderr; each played line is reported on stderr, so a check waits for it instead of sleeping.
  - Polled every 100 ms off the main thread, not watched, for the reason `design/theme.md` gives against `notify`. The lines present at start are not played, so a stale file never replays presses, and a last line waits for its newline. A file that no longer starts with what was read is read again from line 1; a missing one is waited for.
  - The follower belongs to the window (`main.rs`'s `open_window`), not to `Root`, which a server switch replaces. Mouse events hit-test the last drawn frame, so the steps wait between events, and a window macOS reports fully covered stops drawing.
  - Its limit is that the OS never sees those events, so what depends on macOS's own routing (the first click on an inactive window, a drag from the Finder) would still go through the HID route, and keys stay with `CGEvent.postToPid`.

## Checked

2026-11-10, macOS 26.6.2 on Apple silicon: cornercase-gpui 0.1.55 as a debug build (gpui 0.2.2), cornercase 0.13.32 from the release's aarch64 archive with its sha256 checked (the Mac's own cornercase untouched).

- Run, in this order: the download and checksum; the folder and environment; the build and the helper; the start with `CORNERCASE_GPUI_OPEN=Settings`; `status` and `--version`; `size`; Escape; `front`; ⌘F and Escape; ⌘T (`status` showed a second tab); the plain prompt, a capture and a crop; two presses; a scroll; the refusals above; the clean-up (`status` failed afterwards and nothing was left). Then, with two debug windows on two private servers: `front` from one to the other, a key to the window behind, a refused press and scroll, and a press on the window in front.
- Not run: Linux and Windows, the real-server variant, a fake agent or machine, drags, a right press, a press or scroll refused because a window covers the point or because it is under the Dock, and the browser profile variable with a Browser panel open.

2026-11-10, macOS 26.6.2 on Apple silicon: cornercase-gpui 0.1.57 as a debug build, two private servers and an `ssh` stand-in (**Another machine**), "there" on cornercase 0.13.32 (the release's archive, checksum read), "here" on 0.13.32 and then on 0.13.21 (a copy of the installed binary, never pointed at the user's server). The Mac's own cornercase, its server, the ssh configuration and the user's settings were untouched, and what was started was stopped and removed.

- Run: a fake agent with a child on "there"; a machine linked from the window; the fold's arrow and its stale answer, and the older "here" (`design/machines.md`); `drag` (a left-button drag from a pane's handle, the release time printed) for a drop that changes nothing and one that changes the layout, on this computer's tab and on a machine's (`design/terminal.md`).
- Not run: a refused move in the window (no tab could be made to refuse for lack of room), a stale move on a machine's tab, a right-button drag, a drag with the old build, Linux, Windows and a real machine.

2026-11-10, `tools/machine-bench.sh` (#119), macOS 26.6.2 on Apple silicon with cornercase 0.13.32 from the release's aarch64 archive (sha256 checked) as both sides, with the installed 0.13.21 copied as "here" and 0.13.32 as "there" (`linked`), and the reverse (`older`). The installed cornercase and the user's server were untouched: its `status` and the ssh configuration were compared before and after.

- Run: `up --agent --log` made the two servers, linked `vps` and started a fake `claude` with a child (`parent`); `commands.log` held the window's `--version`, `machines --json` and `status --app --follow`; `here --remote vps.invalid status` answered from there, and `calls.log` showed it and the link's `proxy`. A debug window (cornercase-gpui, gpui 0.2.2) started as the summary prints showed `vps` linked in the Machines sheet. The hold plus `machines retry vps` gave `unreachable · Connection refused`, and removing it `linked` again. Two benches side by side did not mix. `down` refused while the window ran, then removed the folder and left no process. The refusals (a folder outside `/tmp`, `/tmp`, a missing folder, a folder `up` did not make, a binary that is not an executable cornercase), a failed `up` and an `up` stopped by SIGTERM left nothing running.
- Linux, in a container (Debian 13.6 arm64, `/bin/sh` = dash, cornercase 0.13.32 aarch64 Linux): the 44 stub tests, `up --agent --log`, `there status --panes`, `here machines`, `--remote`, `status` and `down` passed. There `machines retry vps` under a hold did not drop the open link (above).
- Not run: a real sshd or network, Windows, a window on Linux, a second machine, the Browser panel on a machine, a press-driven fold, an Accounts capture, and the safety-net breaks of the script's tests. The `lsof` look in `down` and the Linux wording above came after this run.

2026-11-10, **A frame's cost**: cornercase-gpui 0.1.39 (its release's binary, re-signed), 0.1.57 and 0.1.58 as release builds, cornercase 0.13.32, macOS 26.6.2 on an M5 MacBook Pro; the cells ran on a private server with the window in front, three runs each.

2026-11-10, **The rail, tab strip and status bar step**: cornercase-gpui 0.1.60 and 0.1.61 as release builds, cornercase 0.13.32, macOS 26.6.2 on an M5 MacBook Pro on battery, cells B, T and C, three interleaved runs each, `sample` of B and T in the first round; the 0.1.61 window was looked at once in a capture (rail, tabs with their orbits, arrows, status bar), not looked at live: hover, clicks, rename, drag, the theme switch, the job spinner, reconnecting, the title row's drag and the backstop's line on stderr.

2026-11-11, **The docked panels step** (#134): cornercase-gpui 0.1.61 (`3bb8b23`) and the 0.1.62 branch as release builds, cornercase 0.13.32, macOS 26.6.2 on an M5 MacBook Pro on AC, cells B, T, BC, BD, BF, BO, BR, BW and TF, three interleaved runs each, `sample` in the first round; every cell was looked at in a capture before it was measured. BW was measured on `main` only (not cached). The live checks (a panel updating on each of its events) were not part of this step.

2026-11-12, **Tunnels** (#106): cornercase-gpui as a debug build (this change, 0.1.68 now; the build said 0.1.66 then), macOS 26.6.2 on Apple silicon, the bench of **Another machine** with cornercase 0.13.51 (the release's aarch64 archive, sha256 checked) as both sides and the stand-in forwarding; then with the installed 0.13.39 (a copy) as "here" and 0.13.51 "there". 0.13.51 is 0.13.50 plus `queue show`, which touches no tunnel file, so the tunnel contract is 0.13.50's. Where upstream keeps the lists: `src/paths.rs` at v0.13.50 (`session_in` is the socket with `.json`, `tunnels` the file `tunnels.json` beside it), which the bench confirmed: a private server wrote `server.json` beside its socket, and a hold wrote no `tunnels.json`; the window's own list is `window/remembered-tunnels.json` beside `settings.json`.

- Run, on 0.13.51 both sides: Ports on "there" (two `python3 -m http.server`); Tunnel on a row (`there tunnels --json` listed a `kind: hold` entry, `calls.log` held only cornercase's own `-N … -L 127.0.0.1:L:localhost:P` line, `pgrep` showed `tunnel hold 8765`, the page answered 200 on the local port, which differed from 8765 because the bench shares one loopback; the toast said so, the row showed `⇄ :L` and Close tunnel, the status bar `⇄1`); Close tunnel (hold and forwarder gone, the port left the window's list); the machine's menu opened from the label with its row and the note that a program could not ask for a tunnel yet (since replaced by orders, #154); killing a forwarder by pid (the hold fell and the window held it again within seconds, on another local port, with no toast); `kill` of the window (no hold and no forwarder left, so a hold ends through its reader; the list stayed) and starting it again (the tunnel came back by itself); a second Tunnel from the row; ⇧⌘K then This Mac (no hold and no forwarder left, the list gone from `tunnels`).
- Run, 0.13.39 here: Ports said `These tunnels are this window's own: cornercase 0.13.50 or newer on this Mac … (it has 0.13.39).`, Open on a port opened the panel's own ssh (`calls.log`: `-N … ControlMaster=no … -L 127.0.0.1:L:localhost:P`, no `tunnel hold`, nothing in `there tunnels`). After `kill` of the window the own forward was left running, the old limit; it was ended by pid.
- Not checked: Open and the start page's cards in the Browser (the Open button was not pressed on 0.13.51), the Copy and system-browser icon on a tunnelled row, the menu's Open in the Browser, Copy, Close and Forget items, a failing hold's toast and the retry's silence beyond the one forwarder killed (the failure toast is a unit test: a held bench also made Ports' own poll fail, which hid the row), `tunnel open` from the machine's side, the Browser after ⇧⌘K (whether it follows the window to the new `Store`), ⌘Q, two windows, a real sshd, Linux and Windows windows. The user's server listed the same workspaces before and after, apart from other teams'.

The fixtures (2026-11-12): the files in `tests/fixtures/tunnels/` come from 0.13.51 on the bench (the view line with its `tunnels`, with the bench's folder renamed, and the hold's line). Not looked at in the window again: the menu's down row, the tooltip stepping aside, the start page's Closing…, and the label with a long host (the bench's `vps.invalid` has a dot the helper did not post then, so the next paragraph used another destination name; `text` types it since #190 and the bench has `there.invalid`, 2026-11-13); they are covered by plain tests where they could be.

⇧⌘K and the menu (2026-11-12, cornercase 0.13.51 both sides, a copy of the stand-in that also accepts the destination `vpsbench`, so ⇧⌘K can be typed with the helper's letter keys): This Mac, Browser with a tab on a local dev server, ⇧⌘K to `vpsbench`: the tab was gone and the machine's workspace opened its dev server through a hold (`Port 8765 on vpsbench is tunnelled…`); the machine's menu then opened with its bottom edge at the label (`Root::open_menu_above`, no height guessed). The menu's heading cut, the generation drop of a late ports sample and the Windows hold test's single deadline are covered by plain tests only (the last runs on Windows CI).

2026-11-13, **A linked machine's ports and tunnels** (#155), cornercase 0.13.54 on both sides (`tools/machine-bench.sh`, a `python3 -m http.server` in a pane of the machine's workspace), the debug window with its settings and Browser profile in files of their own: a machine workspace's ports in Ports (workspace id 3 on both servers), Tunnel to another local port with the toast, `curl` 200, Close from the status bar's menu, the same port on both servers told apart, the tunnel back after a window restart, Stop with the machine named, a cut link (no sample goes out, rows stay, the hold stays up), removal and quit leaving no hold or forwarder, and here 0.13.39 with there 0.13.54 for the gate's words. The user's server listed the same workspaces before and after (apart from other teams' work) and the user's `remembered-tunnels.json` was never written. Not checked: the window on another server that links a machine, an older cornercase on the machine's side, the menu's Open, Forget and the toast's Open, Windows and Linux.

- **A machine's page in the Browser panel (#174)** was looked at in #184, below, apart from what that entry lists as not run.

2026-11-13, macOS 26.6.2 on Apple silicon: **a machine's page in the Browser panel, a program's tunnels and the ways out** (#184). cornercase-gpui as a debug build (gpui 0.2.2) on four builds: 0.1.79 (`00a23d5`) for K1 and K2 and K3 failing; the first fix, `fix: keep the keys in the Browser's address…` (a version of the rule since replaced), for K3 again, K4, A1 to A8, E1 to E3 and P1 to P5 (C); the popup fix, `fix: let a popup opened blank…` (the same keys code as the first fix), for A9 again, W1, W3 and W4 (C) and for A1, A2, A3, A5, A7, P1 to P3, P6 and P6b (U); and the final head (`fix: end the Browser's wait…` and the docs after it) for K2, K3, the edited-address case and K4 again, and for A4 again (C). The UI reviewer also looked at Return with a page shown and from the start page on the final shape of the rule, locally. cornercase 0.13.57 on both sides (0.13.39, from the release's aarch64 archive with its sha256 read, on the older side), Chrome 157.0.8103.52. Real: the servers, the link, the stand-in's ssh and forwards, the holds, takers and orders, Chrome. Made up: the fake agents, the pages, `.slow` and `.held`, `/json/close`, a shell standing for a program's `tunnel open` and `close`, and the bench's `commands.log`. Made by the coder (C) and by the UI reviewer (U), each in a window of their own:

- **K1, K2, K4** seen working (C): ⇧⌘K from the terminal, from a page and from the address; K2, K4 and K3 again on the final head, with captures of the page focused and of the address focused taken before the key, and the same dialog over both. The case the code reviewer raised, an address edited while its page waits, was also run there: the page showed, the typed text stayed in the address and further keys went into it, not into the page. The typed `there.invalid` arrived with its dot, once the dialog had drawn. **K3** (the keys while a typed open on a machine's place waits for its hold, `.slow` 8 s): seen failing on 0.1.79 (C), fixed in 0.1.81 (`design/browser.md`, "While no page is shown"), seen working on the first fix and again on the final head (the dialog opened while "Opening a tunnel to port …" showed). The posted event was never at fault: this is #182's report. **K5** not needed.
- **A1, A2, A3, A5, A6, A8** seen working (C and U: A1, A2, A3, A5); the commands are in `commands.log`. A5's note "Reloaded: claude finished" showed about 2 s after idle (U). A3's chooser lists two agents, but both read "claude · bash" and the button keeps that label after the pick (U): #198. **A7** (the `busy` refusal, `still handing to claude · bash on vps: nothing was sent`, no new command) seen working (C and U), but made with a hand-off that was stuck, not with `.slow`.
- **A4** (an instruction): **seen failing**. Return runs `paste-file` then `send --pane N --enter --`, and the hand-off does not end: on 0.13.57 with an empty input, `status --json` then shows the pane with `draft: true` and `held_sends: 1`, so cornercase holds the send behind the `paste-file`'s own draft; one run ended after about 75 s with cornercase's `not confirmed` error, another had not ended after minutes. Filed as #201. The error's hint (`cornercase read --pane N` without the `--remote`) is cornercase's: usecornercase/cornercase-terminal#807.
- **A9** (a link's popup, then the forwarder of the hold killed): seen failing (C), fixed (`design/machines.md`, "A tab follows its hold"), seen working: both tabs moved to the new port.
- **P1 to P5** seen working (C; U: P1 to P3): the asked tunnel's answer, the toast and `⇄ :L` in Ports, Open from the toast and from the tunnels menu, `tunnel close` leaving the tab, and 31 asks on top of 2 holds giving 32 live holds with the extra ask refused at once with the window's reason. **P6b** (this Mac older, here 0.13.39): seen working (U), start page and Ports say "needs cornercase 0.13.50 or newer on this Mac (it has 0.13.39)", nothing offers Tunnel or Open, nothing opens by itself; the tunnels menu's note was not reachable (`design/machines.md`, "A program on a machine can ask for a tunnel here"). **P6** (the machine older): seen failing (U): with the window's command line newer than the machine, cornercase answers `older` and the window says "did not answer · reconnecting…" for good (#197); the ports rows with no Tunnel could not be shown. The card's error line runs past both edges of a narrow pane: #200.
- **E1 to E3** seen working (C): the tab and the holds ended together when the machine was removed or its destination edited, the keys came back after both, the remembered ports came back silently when it was added or edited back; unreachable then linked kept the tab, the forwarders and the taker. After the restart of a window, a tab of the adopted Chrome showed under the machine's place: the known limit of an adopted Chrome's old tabs (`design/machines.md`), not a new finding.
- **W1, W3, W4** seen working (C). W1: the `vps.invalid` taker ended, a `there.invalid` one ran, the dock was closed. W3: back on this Mac with the `vps.invalid` taker and the remembered holds held again (the Connect dialog lists the current server first, so the This Mac row moves down: read a capture before pressing). W4 (⌘Q, `key <pid> q cmd` after the pid check, with a machine's tab themed): the window exited in under 6 s, the commands were `--remote vps.invalid theme --clear` and `theme --clear`, both servers read `default colours` after, no hold, forwarder, taker or Chrome was left. `layout.json` was not written, as `Keeper::flush` writes it only when the layout changed.
- Escape with a docked panel focused closes the whole panel, also over an open agents menu (C and U): it is `Root::escape`, against `design/terminal.md`: #199.
- **Not run**, for lack of time in this run: W2 (a late claim closed, Chrome stopped with SIGSTOP), E4 (an older side: tunnels end, tabs stay), E5 (a tab whose machine links before the list is read) and K5. #192's wording was not seen.
