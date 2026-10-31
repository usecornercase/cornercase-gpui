# Security policy

## Reporting a vulnerability

Please do not open a public issue. Report it privately through [GitHub's private vulnerability reporting](https://github.com/usecornercase/cornercase-gpui/security/advisories/new).

Include what you found, how to reproduce it and what an attacker could do with it. We will acknowledge the report, keep you posted while we work on a fix, and credit you in the advisory unless you prefer otherwise.

A vulnerability in cornercase itself (its server, its socket, its command line) belongs to [cornercase-terminal](https://github.com/usecornercase/cornercase-terminal/security/advisories/new).

## Scope

cornercase-gpui is a macOS window that drives cornercase's server through the `cornercase` command line. Things we especially care about:

- tokens typed in Settings leaking to logs, toasts, the screen or other processes (they go on standard input to `cornercase config secret`)
- commands and arguments built from what programs print or from issue, plugin and flow data: `cornercase` and `ssh` arguments, paths and links opened with a click or ⌘-click
- escape sequences from programs that reach the clipboard (OSC 52) or the window
- the Browser panel: its Chrome's DevTools port (only this Mac's loopback should reach it, and another machine's loopback through the reverse ssh tunnel while an agent there uses it), its profile, and the page script's bridge to the window
- files read or previewed from the disk: the Files panel and the preview card of a path a program printed

Only the latest commit on `main` is supported.
