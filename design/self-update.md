# Updating the window

Code: `src/self_update.rs`, `src/self_update/` (`release`, `fetch`, `install`, `relaunch`, `dialog`), `main.rs`'s `--version` and quit path, `release.yml`'s `CORNERCASE_GPUI_RELEASE`, `platform/probe.rs`. The model is cornercase's own updater (`design/updates.md` in `usecornercase/cornercase-terminal`); `design/notifications.md` has the server's update, which is a separate chip and dialog.

- **What it does**: at start (after 10 s, so it never competes with finding the binary) and every hour, a release build asks GitHub's `releases/latest`. A newer release shows `↑ window X` in the status bar, whose dialog lists every `## X.Y.Z` section of the release's `CHANGELOG.md` asset after the running version (the release body stands in when the asset is missing or lacks the release). The chip says "window" because cornercase's own `↑ X` sits beside it and the app calls itself "cornercase" elsewhere.
- **Which builds check**: release builds ask GitHub; a debug build asks nothing unless `CORNERCASE_GPUI_RELEASES_URL` is set, which is then the URL asked (and may be plain `http://`, for a local fake release). Release builds ignore the variable, so the environment cannot turn off https-only downloads. So `cargo run` and the tests never call GitHub. **The `check_updates` setting is cornercase's own, reused on purpose**: one switch for "do not call GitHub for updates" is what a user who turned it off expects of the window too, and the window has no settings of its own for this. It is read (`config --json`, a boolean, on by default) before each check; a config that cannot be read checks anyway, since a newer window may be what fixes the connection.
- **Notes are retried**: a release whose `CHANGELOG.md` could not be downloaded shows the release body, and the next hourly check asks for the changelog again; the notes are kept only once it came down. Its download has a 20 s overall limit, so a trickling response cannot hold the check open.
- **A failed install offers Download** of the same asset, since a retry would fail the same way when the folder lets files be made but not renamed.
- **Failures are silent**: a muted line in the dialog, never a toast (an offline laptop would get one an hour). 404 means nothing is published (true until 0.1.0 is tagged), not an error; 429, and a 403 with `x-ratelimit-remaining: 0` or `retry-after`, say GitHub's limit of 60 checks an hour was reached. The last release found stays after a failure.
- **Not a status-bar job**: nobody started the hourly check, and the bar would flash "Checking…" every hour. The install, which the user starts, is a job (`store::run_local_busy`), with a spinner on the chip and `ui::busy_button` in the dialog.
- **The updater is created once in `main.rs`**, not per `Root`: `Root::switch` builds a new `Store`, and an install must survive a switch. It calls `Store::get(cx)` whenever it needs the session or the jobs.

## How a copy updates

Decided off the main thread when a newer release is found (`install::way`, pure; the folder probe is passed in):

| The copy | The dialog offers |
|---|---|
| Built by the release workflow, Windows or Linux x86_64, folder writable | Install, then Restart window |
| macOS bundle | Download the DMG |
| A folder it cannot write | Download, naming the folder |
| No asset for this machine (Linux or Windows arm64) | The release page |
| A build from source | Copy command: `cargo install --locked --git … --tag vX.Y.Z` |

- **Only builds made by the release workflow replace themselves** (`CORNERCASE_GPUI_RELEASE=1`, read with `option_env!`): a build from source could hold more than the release. Rejected: guessing from the path (`.cargo/bin`, `target/`), which a copied build fools.
- **Another window already installed it**: the program at the path read at start is asked its `--version`; if it already is the release, the chip and the dialog offer the restart, not a second download (as `notifications.md` says of cornercase).
- **The program's path is read at start**: on Linux `/proc/self/exe` reads `… (deleted)` once the file is replaced, and the relaunch must start the new file at the old path.

## Install (Windows and Linux)

On the background executor: the archive and its `.sha256` downloaded and compared in memory first, so the staging folder, which each start sweeps once over an hour old (a crash leaves it), is made only when the download is done: it sits beside the exe (same volume, so the renames are atomic, removed whatever happens); the archive and its `.sha256` were downloaded (256 MiB at most, and a stall of 30 s ends it, with no limit on the total time so a slow connection can finish; `https_only` unless the URL asked was `http://`); the checksum compared; the archive unpacked with the system's `tar` (Windows: `System32\tar.exe` by its full path, since a bare `tar` may find a GNU tar beside the exe that cannot read zips; Linux: `tar -xzf`); the binary found at the root or one folder down; the new binary run with `--version`, which must print `cornercase-gpui <the release>` (so a binary that cannot run here, one needing a newer glibc say, never replaces a working one); then the swap.

- **Linux** renames the new file over the running one, which Unix allows.
- **Windows** cannot overwrite or delete a running exe but can rename it, as cornercase's updater does: the exe becomes `cornercase-gpui.old.exe` (`.old.1.exe`… when an older copy still runs, say in another window), the new file takes its name, and if that fails the old one is moved back. Each start removes the old copies nothing runs any more; a delete that fails means one still runs.
- **HTTP is `ureq` 3**, blocking, on the background executor like every other job. Rejected: gpui's `http_client` (a trait without a client in 0.2.2; Zed's needs tokio), `curl` as a child (not on every Linux desktop, and tests could not run without it), the `self_update` crate (no DMG, its own HTTP stack, and we need our own states), Sparkle and WinSparkle (a tool per platform, appcasts and keys we do not have).
- **TLS**: rustls with the **ring** provider and the system's roots (`RootCerts::PlatformVerifier`: the Keychain, Windows' store, `/etc/ssl`), so corporate roots work. Not ureq's default `rustls` feature, which brings `webpki-roots` under CDLA-Permissive-2.0, a licence the notices check does not accept; not `aws-lc-rs`, which needs cmake and NASM on Windows. `Cargo.lock` lists `webpki-root-certs` through `rustls-platform-verifier`, but only for wasm32, which no target builds. `HTTPS_PROXY` and `ALL_PROXY` are honoured by ureq.
- **The checksum comes from the same release**, so it catches a broken or cut download, not a bad release. A file the window downloads itself carries no Mark of the Web or quarantine flag, so SmartScreen and Gatekeeper do not ask again: until signatures (#23, #24) the checksum is the only gate. It is not a signature: whoever can publish the release can publish a matching checksum. This is the trust model of cornercase's updater.

## Relaunch

**Restart window** stores the program and the arguments (`--tab <shown tab>` and the current server's `--remote`, `--remote-command`, from `cli::target()`, since a switch may have changed it), then quits. In `on_app_quit` a relaunch runs `cli::kill_all()` **without** `theme --clear`: the new window gives the same palette at once, so clearing first would only flash every terminal, and over ssh `cli::quit` does not wait for the clear, which could land after the new window's `start-server --theme -`. Then it starts the new program with stdin null and stdout and stderr inherited. There is no confirmation: the server keeps every program, and the dialog says so.

## Limits

- The window's size and place are not kept across the relaunch. What only the window holds (open panels and dialogs, the Browser panel's Chrome) starts again.
- A relaunch that fails to start ends the window with a message box and standard error (the server and its programs keep running).
- The asset names are a contract (`design/releases.md`): renaming one breaks the update of every installed copy.
- Nothing is signed. The checksum is the only check.

## Waits on

- **Replacing `cornercase.app` on macOS**: mounting the DMG, copying the bundle, checking its signature, swapping and relaunching with `open`. It cannot be checked from Windows, and the condition "signed with the same identity" cannot hold while builds are ad-hoc signed, whose identity changes with every build (#23). Until then the dialog downloads the DMG. A follow-up issue.
- **Signatures** (#23 macOS, #24 Windows).
- **Package managers and installers** (#24, #25, #26): a package's folder that cannot be written (`/usr/bin`, a mounted AppImage) falls back to Download already. Scoop and winget put the zip's exe in a folder the user can write, so #26 must add their paths and commands before they ship, or the window would replace a file their manager owns. An AppImage (#25) would replace `$APPIMAGE`.

## Checked

2026-11-05, Windows 11, a debug build, cornercase 0.13.1: the real endpoint answered 404 through ureq and the platform verifier and read as "nothing published"; the install ran against a local fake release (a flat zip, its `.sha256`, served over HTTP, stand-in programs printing `cornercase-gpui 0.0.1` and `0.2.0`): a wrong checksum was refused and left the old program, a right one was unpacked with `System32\tar.exe`, verified with `--version`, swapped, left `cornercase-gpui.old.exe` and the staging folder was gone, and `remove_old_copies` removed the old copy. **Not checked**: the chip and the dialog in a running window, the relaunch, a real release (0.1.0 is not tagged), the Linux install (unit tests on CI only), macOS.
