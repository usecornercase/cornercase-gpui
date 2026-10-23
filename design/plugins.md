# Plugins

Code: `src/panels/plugins.rs`, `src/panels/plugins/`

- **Like the TUI's Settings › Plugins, with no catalogue**: what is installed or linked, whether it is on, what it asks for and runs, its actions and its log; turning each on and off, linking a folder, installing, updating and removing. Finding plugins is the website's job.
- **The list is read again each time the sheet opens**: `plugin list --json` reads the plugins from the files, `plugin actions --json` asks the server for their actions. Without a server, or with an older one, there are no actions to offer, and the list stays.
- **A plugin's source is kept as JSON** (`{"kind":"link","path"}` or `{"kind":"git","repo","url","ref","subdir","commit","path"}`), so a kind added later still reads; an unknown kind is shown by name.
- **Turning one on is the explicit step**, so it asks first and says plainly what that means: it runs as you, what it asks for is shown, not enforced, and what it runs. Turning off asks nothing. Plugins start off.
- **Removing an installed plugin deletes its copy; unlinking a linked one only forgets it**: both ask, with the difference spelled out, and settings and data stay.
- **The confirmation is this sheet's own small dialog** (`plugins/ask.rs`), since `dialogs::confirm` runs a command and these steps run a closure that marks the plugin busy and reloads.
- **Install and update run in a terminal tab**: installing shows what the plugin runs and asks before anything runs, and a client cannot drive that preview yet (#468 upstream), so `cornercase plugin install …` runs in a new tab of the shown workspace, where cornercase asks itself, and the sheet closes so the tab shows. Words are quoted for a POSIX shell.
- **Actions run on what the window shows**: a `button` action takes no target; otherwise the most precise of its contexts the window shows (pane, tab, workspace, project), and none when it shows none. `plugin run` gets the revision the actions were listed at and refuses a stale one.
- **A failed action exits with 1 but still prints its outcome**, which the toast reads; an error before it ran (a stale revision, no server) prints nothing, and the error is shown instead.
- **Popup actions are not runnable here**: they open in cornercase's own window, and showing them in this app needs #456 upstream. Whether a plugin's service is running needs #469.
- **The log shows the last 60 lines** (`plugin logs ID -n 60`) and is read again with the list.
