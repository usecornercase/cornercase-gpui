# Changelog

Every pull request that changes the app adds a section here for its new version. The section becomes the notes of the GitHub Release and shows up in the window's update dialog, so write it for users.

## 0.1.62

- **An open panel costs much less while agents work.** The Changes, Files, TODO, Recap and Flow panels are now kept from one frame to the next unless something they show changed, as the sidebar, the tabs and the status bar already are. On a bench of 22 agents with 5 working, at 1512×912, a frame drawn to turn the orbits with the panel open went from 5.7 to 1.9 ms with Changes, 3.3 to 1.7 ms with Files, 4.4 to 2.1 ms with TODO, 2.5 to 1.7 ms with Recap and 4.0 to 2.1 ms with Flow, and the window went from 18.6 to 8.4 %, 12.6 to 7.7 %, 15.6 to 8.2 %, 9.4 to 7.2 % and 14.4 to 9.1 % of a core. One of the targets set before measuring was missed: an open panel still adds 0.5 to 0.9 ms to such a frame, more than the 10 % aimed at. The Browser panel is not kept yet.
- The Flow panel's times (5s, 2m) now count up every second while it shows a run, also when no agent is working.

## 0.1.61

- **The window costs less again while agents work.** The panel icons on the left, the tabs above the panes and the status bar are now kept from one frame to the next unless something they show changed, as the sidebar has been since 0.1.58, and the working orbits in the tabs are drawn over them. On a bench of 22 agents with 5 working, at 1512×912, a frame drawn to turn the orbits went from 1.35 to 0.75 ms with seven tabs on top (1.0 to 0.8 ms with two), and the window from 6.2 to 4.4 % of a core (5.5 to 4.6 %).
- A working tab scrolled out of sight in the tab bar no longer keeps the window drawing its orbit.

## 0.1.60

- **A flow can close its agents once its pull request merges.** Tick Close the agents once the pull request merges when you start it, or on its page in the Flows sheet or the Flow panel while it runs. Once the pull request has merged and none of its agents has worked for 3 minutes, they close as Close agents does: nothing is removed, and Reopen agents, on its page or in the workspace's menu, brings them back. A flow that leaves the commits opens no pull request, so it does not offer it.
- It needs cornercase 0.13.33 or newer; with an older one the choice rests with a line saying so. A flow started on another machine needs it there too, and the message after the start says whether that machine took the choice.

## 0.1.58

- **The window costs less while agents work.** The sidebar is kept from one frame to the next unless something it shows changed, and the working orbits are drawn over it. On a bench of 22 agents with 5 working, at 1512×912, the window went from 12.7 % to 5.9 % of a core (a frame from 3.9 to 1.2 ms), and from 15.5 % to 7.7 % while an output scrolls in the terminal.
- A row's buttons (⋯, +, ✕) are now made only while the pointer is over it, so after a scroll with the pointer still they show once the pointer moves.

## 0.1.57

- **Folding a machine's agent never acts on a server that restarted.** With cornercase 0.13.32 or newer on this computer, the arrow and Collapse/Expand on a machine's agent in the agents section tell that machine which run of its cornercase they were pressed on. If it restarted on the way, nothing is folded or unfolded and the window says `vps restarted: nothing was sent`. With an older cornercase here, folding works as before.
- **A pane dropped where it changes nothing goes back at once.** Dropping a pane on a place that leaves its tab's layout as it was, on this computer or on a machine's tab, flies it back as soon as cornercase answers, instead of after a second and a half.

## 0.1.56

- **Edit a flow in a form that follows its plan, with the plan beside it.** New and Edit read in the plan's order: what the flow is, the orchestrator, each stage with its roles, then what happens when the work is done. Every role is a row with its agent and what it may do. A click opens it, one at a time, and so does a click on its box in the plan. The plan stays beside the form when the window is about 1,220 px wide or more and the plan fits beside it; otherwise Form and Plan switch between them.
- **No more typing role names into stages.** A role's Works in moves it to another stage or a new one, and a stage goes when its last role does. Add a role beside and Add a stage add a role and open it.
- **Loops read as a sentence**: if it reports problems, go back to stage 1 at most 3 times, with the stage picked among the earlier ones and the times stepped. Add a loop back starts from cornercase's own.
- **The check stays in view.** The header says Checking…, Ready to save or how many problems there are, the problems are listed under it, and Save waits while the finished check lists problems. It does not wait while the check is still running or if the check could not run.
- A flow whose stages and roles do not match opens with them put right and says what changed; Cancel asks before dropping that.
- Notes for the orchestrator, and a role's arguments and variables, wait behind a button until used. The orchestrator no longer shows a name to edit, since cornercase always calls it orchestrator.

## 0.1.55

- **Move panes on another machine's tab.** Drag a pane by its handle, or with the right button (with Shift when the program in it reads the mouse), onto another pane, or use Move pane in its menu. If that machine's cornercase restarted in the meantime, nothing moves and the window says so.
- **Mark a machine's agent for review, and switch its account.** On a machine's tab, ⌘R and the pane's menu mark the agent for review, or as reviewed, and Switch account restarts it on another of that machine's own accounts, its own login included.
- **View context on a machine's tab checks which run it reads.** If that machine's cornercase restarted since, the read is refused instead of reading another agent.
- These need cornercase 0.13.30 or newer on this computer. With an older one, the menu's entries rest with a line saying so, and View context reads as before.

## 0.1.54

- **A machine's tab keeps no more than it shows.** Measured over a long session on another machine (its tabs visited hundreds of times, long output, the link cut and back, its cornercase restarted), the window's memory stays flat. A frame that names rows past a pane is no longer held: the window asks for a whole one instead, as cornercase's own window refuses such frames.

## 0.1.53

- **On Windows, plugins' window processes now end with the window when it crashes.** If the window crashed or was ended from Task Manager, `cornercase plugin windows` and the processes your plugins run beside the window kept running, with nothing left to stop them. Windows now ends them with the window, as already happened on macOS and Linux. Quitting stops them as before. If Windows does not allow it, the window says so once.

## 0.1.51

- **The window says when another machine runs an older cornercase, and updates it.** Connecting to a machine whose cornercase is older than this computer's, with `--remote` or ⇧⌘K, now shows a page with both releases, instead of "Lost cornercase" with cornercase's raw message and a retry that could not succeed. **Update cornercase on vps…** asks first, runs the update cornercase names from here and connects again. If that machine's server still runs the old release, **Restart cornercase on vps…** restarts it, after saying that every program in its terminals stops, or you can connect without restarting. This needs cornercase 0.13.23 or newer on this computer.
- **A connection that never came up no longer reads "Lost".** When the window cannot connect at all, it says "Could not connect to cornercase on vps" with the reason, without cornercase's `cornercase start-server: Error:` in front. "Lost" is kept for a link that dropped.
- **Update an old cornercase from the window.** When this computer's cornercase is older than the window needs, its page offers **Update cornercase…**, which asks first, runs `cornercase update` and tries again. When cornercase cannot update itself (Homebrew, a folder it cannot write), the page shows what it says to run.

## 0.1.50

- **Start an agent in default while settings choose another mode.** An issue's Start now offers **default** (no mode arguments) like any other mode when settings choose one such as plan, and the agent starts without it, on this computer and on a machine. It needs cornercase 0.13.28 or newer wherever the agent starts: with an older one, default rests as before, and the line under it says which version offers it and which one runs.
- **Schedule in default too.** Schedule on an issue and the Queue's form send default the same way with cornercase 0.13.28 or newer. With an older cornercase, where it meant the mode settings chose, the Queue's form now rests it with the same line. **as in settings**, which the form starts on, schedules as before.

## 0.1.49

- **Close an agent on another machine's tab.** Right click a pane where an agent runs on one of your machines and choose Close agent…: as here, it first asks that machine what would stop, the agent's tab and the tabs of the agents it started, and what keeps running elsewhere, naming the machine. If that machine's cornercase restarted in the meantime, nothing is closed and it says so.
- **See what fills its context.** View context, in the same menu, reads that agent's context on its machine, without typing into the agent or spending tokens.
- **Switch account waits on cornercase.** On a machine's pane whose agent runs on an account, Switch account shows with a line saying what cornercase still needs for it.

## 0.1.48

- **macOS notifications come from cornercase, not Script Editor.** The downloaded app now posts its desktop notifications in its own name and with its own icon, and System Settings › Notifications lists cornercase on its own, so you can allow, silence or style them there without touching other apps' notifications.
- **macOS asks first, when one is due.** The first time an agent's notice would show a notification, macOS asks whether cornercase may notify you, never when the app opens. If you don't allow them, a toast says where to turn them on, or how to turn the notifications off in cornercase, the next time you come back to the window.
- **A click goes to the agent.** Clicking a notification brings the window forward on the agent's tab, as the toast's Go does, an agent's on one of your machines too. The notification plays no sound of its own: the cue plays as before.
- **A sound or notification that fails now tells you when you come back.** If a sound or a desktop notification fails while the window is behind (on any system), its error toast waits until you return to the window, instead of showing and expiring while you are away. It is still told once per run.
- A build from source, which runs without the app bundle, still notifies through Script Editor.

## 0.1.47

- **Choose the mode on another machine.** Starting an agent on an issue with **Run on** set to a machine now offers that machine's own modes, as its settings list them, with the one it chose already picked. The Mode used to be hidden there. While the machine is asked, or when it cannot answer, the agent starts in the machine's own choice, as before, and the form says why.
- **See what the machine makes of the issue.** The form now shows the prompt that machine's template makes for the issue, and fills the branch with the name it gives it. **Start a flow** on a machine fills the task and the branch with that machine's too. Leave them and the machine uses its own; whatever you type goes as typed.
- **default no longer fails at Start.** While another mode is chosen in settings, cornercase does not start an agent in **default** (no mode arguments), so that choice now rests with a line saying what it waits for, here and on a machine, instead of failing when you press Start.

## 0.1.46

- **Select with the keyboard in the Changes diff.** Click in the Changes panel, then ⇧← ⇧→ select a character more or less and ⇧↑ ⇧↓ a line (Shift with the arrows on Linux and Windows), as in the Files viewer. The selection goes on across hunks and open gaps, over removed and added lines in the order they show, and with nothing selected it starts from the top of what the diff shows. Lines picked by their numbers grow and shrink by whole lines, and the view follows the end you move.
- **It works like a selection made with the mouse.** Copy, Ask agent and Open float beside it, ⌘C copies it and Esc clears it, also on a machine's workspace, where Copy is the one offered. In the terminal and in the filter field the keys do what they did before.

## 0.1.45

- **Close a pane or a tab on another machine's tab.** ⌘W closes the pane and ⇧⌘W the tab you see from a machine, as they do here: a shell closes at once, and a pane or tab running an agent asks first, naming the machine. If that machine's cornercase restarted in the meantime, nothing is closed and a toast says so.
- **A right click on a machine's pane opens its menu**: copy, paste, split, copy its path and close it. Marking an agent for review and moving a pane there show in the menu with what cornercase still needs for them, and ⌘R says the same.

## 0.1.44

- **Select with the keyboard in an open file.** Click in the Files viewer, then ⇧← ⇧→ select a character more or less and ⇧↑ ⇧↓ a line (Shift with the arrows on Linux and Windows). They go on from the end of what you selected, or with nothing selected from the top of what the viewer shows. Lines picked by their numbers grow and shrink by whole lines, and the view follows the end you move.
- **It works like a selection made with the mouse.** Open, Ask agent and Copy float beside it, ⌘C copies it, Esc clears it, and it stays when the file changes; on Linux it also fills the primary selection. In the terminal and in text fields the keys do what they did before.

## 0.1.43

- **Changes and Files say when a machine's link tries again.** While the link to a machine is down, the strip at the top of both panels counts down to the next try, `vps unreachable · reconnecting in 4 s`, as the machine's card and the Machines sheet do, and says `reconnecting…` while the try runs.

## 0.1.42

- **Remove worktree… can delete the branch too.** A worktree's Remove worktree…, in its menu in the sidebar, now asks what the Worktrees sheet asks about that folder: what cornercase finds there (files changed, commits not pushed, a lock, the programs that would stop, an agent at work among them, the teardown), with an **Also delete the branch** box, off by default. Ticked, it says in red which commits the branch would lose. The button says what it will do (Remove, Remove anyway, Unlock and remove) instead of asking you to tick each risk. The default branch and the main checkout's are never deleted.
- **A toast says what went:** the worktree and its branch, or why the branch was kept, a teardown that failed, or a folder that changed while you were asked, which is left as it was.
- **The same on your other machines.** The ✕ of a worktree on a machine in your sidebar asks the same question about that machine's folder, and can delete its branch there too.

## 0.1.41

- **The window opens where you left it.** Its size, its place on your displays and whether it was maximised or full screen come back at every launch, and after an update's restart. If that display is gone, or the place would be off screen, it opens centred as before. On Linux it keeps its size and state, and your desktop places it.
- **The sidebar and the docked panels keep their widths.** The widths you drag the sidebar to (or its two columns), Changes, Files, TODO and Recap, the Browser and the Flow panel are there at the next launch. A double click on the edge brings back the default. In a window too narrow for them, the terminal keeps its room first, and your widths come back at a later launch in a window wide enough for them.

## 0.1.40

- **Prompts take several lines.** In the Queue's form, Start a flow and Start an agent (⌘G), Enter now starts a new line instead of scheduling or starting the agent on half a prompt. ⌘Enter (Ctrl+Enter on Linux and Windows) or the form's button sends it, as the line under the field says. The field starts three lines high, grows to twelve, then scrolls, and a paste keeps its lines. The other fields (times, branches, names, an issue's key) still send on Enter.
- **An issue's task keeps its lines.** Start a flow on an issue fills the task from Settings' prompt template with its line breaks, where they used to become spaces.
- **A prompt may start with a dash.** Schedule and Start an agent no longer refuse a prompt that starts with `-`, such as a list.

## 0.1.39

- **Choose the shell new panes start.** With cornercase 0.13.21 or newer, Settings › UI › shell lists the shells found on the computer cornercase runs on: open the list with its arrow or ↓, and choosing one saves it. Or type any program (a full path, one starting with `~/`, or a name on the `PATH`) and press Enter; cornercase says why when it cannot use it. `default` brings back your usual shell. Shell arguments takes options such as `-l` or `--login`, and changing the shell clears them. New tabs, splits and workspaces start it; panes already open keep theirs.
- **A setting cornercase warns about shows in red**, with its note: a chosen shell that can no longer start (new panes start the default meanwhile), an issue branch template GitLab will not close issues from, or a prefix key that clashes.

## 0.1.38

- **Themes.** Settings › Appearance changes how the window looks: eleven themes, each shown as a small preview, apply with a click, to the window and to its terminals, so Claude Code, Neovim and delta pick a matching look.
- **Make one yours.** Click any colour for a picker. Text that would be hard to read says so, and Fix makes it readable. Hold Peek to see the window behind Settings. Sliders set how see-through the glass, the panes, the cards and the dimming are, and corners can be compact or round.
- **Light and dark.** cornercase light is new, and the window can follow the system, with a theme for each.
- **Share a theme.** Save it to a file, copy it as JSON, or import one. Your themes are files beside the window's settings, and editing one in an editor redraws the window.

## 0.1.37

- **Update notes read as formatted text.** The update dialogs, for cornercase and for this app, now show what changed with its bold, `code`, links, lists and headings, as cornercase's own window does, instead of the raw Markdown with its `**` and backticks. A link in the notes opens in the browser.

## 0.1.36

- **The app updates itself on macOS too.** Install in its update dialog now downloads the release, checks it was signed by cornercase's developer and puts it in place of `cornercase.app`; Restart opens the new version on the same tab, and your terminals and agents keep running. No more opening the DMG and dragging the app to Applications. If the app sits in a folder it cannot write, the dialog still offers the download.
- **Each update says what it updates.** The status bar now reads `↑ cornercase 0.13.19` for cornercase and `↑ cornercase-gpui 0.1.36` for this app, instead of `↑ 0.13.19` and `↑ window 0.1.36`, and so do their installing and restart chips and dialogs.
- **Updating cornercase with Homebrew no longer leaves the update showing.** After `brew upgrade cornercase`, or any install the server did not make itself, the chip turns into `↻ restart cornercase for 0.13.19` within seconds, since the new version is already on disk and only the restart is left. This works for the server on this computer; one on another machine keeps showing the update until it restarts.

## 0.1.35

- **A hunk's Copy keeps tabs.** A hunk copied in the Changes panel now has the file's own tabs, so a Makefile's hunk applies. `patch` takes it as copied; `git apply` takes it once `--- a/FILE` and `+++ b/FILE` go above it, since it refuses any hunk with no file header. A hunk of a CRLF file, or one on a last line with no newline, still does not apply.
- **Free builds no longer gives up in silence.** If the window went behind others while the builds were measured, the question now opens anyway, and with no window to ask in, a message says nothing was freed.
- **The window names more of what waits on cornercase**, in a muted line where you would look for it: managing plugins on another machine in the Plugins sheet, logging in on a signed-out account's row, which agents can remove their workspace under a routine's cleanup box, picking an agent, model and effort in the flow editor, and the machines cornercase's own window has reached in the Connect dialog.

## 0.1.34

- **The agents section lists a machine's agents too.** With the section on, each linked machine's agents are rows, tagged with the machine, after the agents of the project they share with you, and those of a project only on the machine come last. A child sits under the agent that started it on that machine. While the link is down the rows stay, dimmed, and say when they were last seen.
- **A click shows that agent's tab**, live, as a click on its tab row does. Nothing moves on the machine.
- **Folding one folds it on the machine**, with its arrow or Collapse and Expand in its menu, so the machine's own window shows the same. A folded row says how many it hides and whether one of them needs you. If the machine fell, restarted or no longer has that agent, a message says so and nothing is sent.

## 0.1.33

- **Run setup, in a worktree's menu.** It runs the project's setup again in a new tab, to retry after a fix, and the menu offers it when cornercase's own does: on a worktree of a project with a setup that isn't already running it. The status bar says it is running, and a setup that fails says so with its exit status.
- **Import a VS Code workspace.** Import VS Code workspace…, in the Projects `+` and in a group's menu, turns a `.code-workspace` file into a group with each folder as a project, or puts them in that group. A message says how many went in, and how many folders were not found or could not open. On a server on another machine it asks for the file's path there.
- **Adding a project to a group takes one step**, so a failure can't leave it outside the group. A file picker that cannot open now says why instead of doing nothing.

## 0.1.32

- **Starting an agent no longer waits on its folder question.** The new tab opens at once, from `+`, ⌘G, a row's Start agent here and an issue's Start, and if the agent asks whether to trust the folder you answer it right there. Until now, from `+`, ⌘G or a row, what you typed never reached that question, and Starting an agent… stayed in the status bar.
- **Starting work on an issue stays in its form until the tab opens.** The button says Starting… while the issue is read and the worktree made, then the form and the sheet close on the new tab. If the start is refused, the form stays with what you typed. The message now says `claude is starting on #7`, since what happens next shows in the tab.

## 0.1.31

- **Pair a phone from Settings › Remote.** Paired browsers, Telegram phone and Discord phone each have a button that opens a dialog with a QR code to scan, the address under it to copy or open, and what to do next, in cornercase's own words. Closing the dialog drops the code. Once the phone connects, the dialog closes and says so, and the row shows the chat's name.
- **When it cannot pair, the dialog says why**: remote access is off, Tailscale is not ready, or the bot's token is missing, with `Try again` once that is fixed.
- **Pick the server of the Discord forum in Settings › Remote.** When the bot is in several of your servers, they are listed under the forum's line: click one, and the forum is made there.
- This works on another machine's server too, with `--remote`.

## 0.1.30

- **A plugin's button now acts on what this window shows.** Its action gets the project, workspace, tab and folder you are looking at here, not those of cornercase's own window, and a popup opens here without that window, at a size taken from this one. Over a machine's tab a button acts on your tab behind it, and its tooltip says so; a popup goes back to that tab.
- **What a plugin keeps running beside a window now runs beside this one.** The processes plugins declare as `[[window]]` run while the window is on this computer: they stop when it closes or moves to another server, and start again when it comes back. A machine's tab does not count as leaving. Each plugin's card shows them: running with their pid, or stopped with the seconds until they start again.

## 0.1.29

- **A machine whose link dropped says when it tries again.** Its card, its row in the Machines sheet and the tabs of its agents in the sidebar count down, `reconnecting in 4 s`, and say `reconnecting…` while the try runs. When only you can fix it, such as cornercase missing there, it says why instead of promising a try.
- **Closing or renaming a workspace on a machine can no longer hit the wrong one.** If the server there restarted while the action was on its way, it refuses and nothing changes: `vps restarted: nothing was sent`. The same goes for removing a worktree there.

## 0.1.28

- **A Recap's link finds its conversation wherever it went.** After a restart, or after closing an agent and reopening it, a link goes to the pane that holds that conversation now, where it used to stop at the workspace with a note that it could not look. A conversation that is really gone takes you to its workspace or its project, and if that is closed too, it says so.
- **Accounts show up by themselves.** An account added or removed from a shell, by another window or by a program shows within a second in the Accounts sheet, on the panes and in the forms that start agents, with no click and no need to come back to the window.

## 0.1.27

- **The Worktrees sheet removes any worktree on disk.** An open workspace, a closed project's worktree with its teardown, an entry whose folder is gone, a folder whose repository is gone or that git no longer knows: every row can be ticked and removed now.
- **It looks again before it asks.** Remove checks the rows first and then says what is at stake for each one: uncommitted files, programs working in it, commits not pushed, a lock, a teardown. The button says what it will do, such as `Remove anyway` or `Unlock and remove`, and only the folders you were warned about are forced, so one that changes after you were asked stays where it is and the sheet tells you.
- **Also delete the branch.** Tick it in the question to delete the branches too. Commits that would be lost show in red first. The default branch and the main checkout's are never deleted.
- **It says what happened, by name.** Removing runs in the background, each row leaving as it goes, and one message at the end names what went, a branch that was kept, a teardown that failed and a folder that could not be removed.

## 0.1.26

- **Starting an agent on a machine shows its tab at once.** The dialog closes as soon as the tab is open there, even while the agent asks to trust the folder, so you answer it in that tab. The window no longer guesses which new tab is the one it started.
- **Clone on a machine can bring the files git ignores.** The Clone form lists this project's git-ignored files, such as `.env`, with their sizes, and copies the ones you tick into the new clone with their permissions. Nothing is ticked at first. A file over 1 MiB is shown but cannot be copied, and a clone takes 8 MiB in all. **Skip files** clones without them. If the machine could not write one, the toast names it and says why. On Windows the form says these files stay behind, as cornercase does not read them there yet.

## 0.1.25

- **A message held for you can be sent or left to wait from its pane.** When a script or another agent sends a message to the pane you are typing in, the pane's card now says who sent it, as in `1 message waiting from claude (pane 9) · goes after yours`, with two buttons: ` send now ` and ` wait turn `, which then reads as chosen. They work on a machine's tab too, and clicking them leaves your keyboard in the pane.
- **` send now ` takes your unsent line with it.** cornercase sends the message after what you have typed and presses Enter, so the two go as one line. The card says so while you have a line in progress. Keeping it back waits on cornercase (usecornercase/cornercase-terminal#708).

## 0.1.24

- **Copying from Changes keeps tabs.** A selection copied in the Changes panel now has the file's own tabs, on this computer and on a machine's workspace, so a Makefile rule or a Go line pastes as it was. A part of a line selected by characters copies the characters you see, each tab as one tab. A hunk's Copy still writes four spaces.
- **Usage opens on numbers.** The stats of a machine or of `all` show the last numbers at once and then the fresh ones, instead of a spinner while the slowest machine answers. A machine with no numbers yet is left out of `all`'s sum and says `not read yet`.

## 0.1.23

- **This window now needs cornercase 0.13.19 or newer.** That release has the commands that let it do what only cornercase's own window could: pairing a phone, removing any worktree, running a worktree's setup again, importing a VS Code workspace, a plugin's window processes, sending a held message now and more. The next versions of the window turn them on. If yours is older, the window says so and how to update it.

## 0.1.22

- **Changes and Files read a machine's workspace.** With a machine's workspace picked in the sidebar, these two panels show what is on that machine, and say `on vps` at the top. Changes follows its diff live in Uncommitted, Commits and All, with the filter, the gaps and selecting and copying lines. Files browses its tree, opens its files, searches names and text, and ⇧⌘P finds a file there. Nothing on your computer is read for them, even where the same path exists here. A window connected to another server can't reach that server's machines: the panels say so and read nothing.
- **They only read.** Staging, discarding, marking viewed, choosing the base, Ask agent and opening in an editor are not offered on a machine's workspace. Every project stays on the server this window is on, and says which.
- **When the machine goes down** the panels keep what they show and say `vps unreachable · reconnecting…`; opening another file there sends nothing and tells you so, and they pick up by themselves when the link is back. If the server there restarted, they start over, since what they showed may no longer be that workspace.

## 0.1.21

- The macOS release no longer fails when Apple's service drops the connection while it confirms the notarization: it asks again. Nothing changes in the window. Releases that failed this way only delayed their notes, which show here with the next one.

## 0.1.20

- **Run an issue, a flow or a workspace on another machine.** An issue's Start, Start a flow and New workspace have a **Run on** row once you list a machine: this computer, then each machine, with why one that is down cannot be picked. The choice lasts while the dialog is open, and a project needs a git remote to run elsewhere, since that is how the same project is found there.
- **The window asks the machine first.** Picking a machine reads its accounts, which agents it has, its trackers and its flows, and says so while it asks. The account row lists that machine's accounts, Start a flow lists its flows, a tracker it can't reach is named, and an agent that isn't installed or logged in there says so and offers a tab there to fix it. Schedule is hidden: the queue starts work on this computer only.
- **Clone a project there.** When the machine has no clone of the project, Start opens **Clone shop on vps**, with the address, which you can edit, and the folder there. The clone runs on its own while you go back to the dialog, and a toast says how it went. It copies no git-ignored files such as `.env` yet: that needs usecornercase/cornercase-terminal#678.
- **What started is selected here** as soon as the machine lists it, without moving cornercase's own window, also while an agent waits for its trust question to be answered. A flow on a machine is followed through its tab, since the run page reads this computer's runs. Every failure is a toast with what the machine or `ssh` said, and the dialog stays open.

## 0.1.19

- **A machine's tab, live, in this window.** A click on a machine's workspace or tab now draws it as it is there, splits included, and it takes your keys and pastes, the wheel, selecting and dragging its dividers. What you select lands on this computer's clipboard, and so does what a program there copies. Its bell flashes here. The tab bar shows that workspace's tabs, and your sidebar and panels stay live beside it.
- **It stays out of the way of what runs there.** The panes take this window's size while you look at them and give it back when you leave. Programs there are told this window's colours while one of its tabs shows, and those are taken back when you leave. Typing costs one process, as on a tab here.
- **When the link drops, nothing you type goes astray.** The last frame stays, dimmed, under a card that says so and when it tries again. Nothing typed is sent or kept, and a toast says so. It comes back by itself, and if the server there restarted it finds the workspace again by its folder. A workspace closed there takes you back to your tab with a note.
- A click on a path, opening links and pictures do not work on a machine's tab yet: they show nothing rather than open a file here by mistake. Closing panes and tabs there is not offered yet, and a window connected to another server says a machine's tab needs it on this computer.

## 0.1.18

- **Files shows what changed.** The tree's rows end in git's letter, in the colours of the Changes panel, for a changed file and for a folder holding one. In an open file the margin marks added lines in green, changed ones in blue and where lines were removed in red; a click on a mark shows the old lines, and another hides them.
- **Long lines wrap in the Files viewer**, as they already did in the Changes diff, so you can read one whole in a narrow panel, with its number on the first row. Selecting and finding work across the rows. A button in the viewer and on the diff's open file cuts them at the edge instead, and the window remembers your choice. A line of more than about 100 rows, such as a minified bundle, ends in `…`; Copy and Open give the rest.
- **Read a file full screen.** A button in the viewer's title gives the file the terminal's area and the dock's. The same button or Esc puts it back, and your programs keep their size. Ask agent and switching tabs put it back too.

## 0.1.17

- **Tabs past the edge can be reached.** When a workspace has more tabs than the bar holds, `‹` and `›` appear at its ends, the wheel scrolls it, and the tab you are on stays in view.
- **`↓ n more`.** A pane scrolled up says how many lines are below it. A click on it goes back to the bottom.
- **A message that waits for you says so.** When a script or another agent sends a message to the pane you are typing in, cornercase holds it until you are done. The pane's card now reads `1 message waiting · goes after yours`. Send now and Wait turn wait on cornercase (usecornercase/cornercase-terminal#676), and the card says that too.
- **New keys to get around**: ⌘J and ⇧⌘J go to the next and previous workspace, ⌘Y and ⇧⌘Y to the next and previous project, ⌘B to the agent that needs you, and ⌘R marks the agent in the pane for review, or as reviewed. On Linux and Windows ⌘ is Ctrl+Shift and ⇧⌘ is Ctrl+Alt+Shift.
- A tab reads `Opus 5.5 · compacted` after its conversation was compacted, and with Settings › UI › memory on it ends with the memory its panes use.
- The rail counts the changed files on Changes' icon and puts a dot on TODO's while items wait.
- The window now follows Settings › UI's model, context, counts, TODO dot and inactive panes, as cornercase's own does.

## 0.1.16

- **Choose the account an agent starts on.** Once an agent has more than one login, whatever starts it offers a dropdown of its accounts: an issue's Start and Schedule, the Agent button, the Queue's form and Start a flow, one for each agent of the team. It rests on the account in use, named as such; pick another and it applies to that start only. A queued entry you leave alone takes whichever account is in use when it starts, and one whose account has since been removed says so rather than moving to another. If you never pick, nothing changes, and without a second login nothing shows. Settings › UI's account picker turns it off.
- The TODO panel's send menu says which account each agent runs on.
- In the Accounts sheet, the Escape after the one that closes the add form, a rename or a new profile closes the sheet. It used to do nothing until a click.

## 0.1.15

- **A machine's workspaces in your sidebar.** They sit under the project here that is a clone of the same repository, after your own, tagged with the machine, with their tabs and their agents' status, model and context. A project with no clone here gets a row at the end of the list. A project's count and its mark count them too, so an agent waiting on a machine marks the project here. When the machine is down its rows stay, dimmed, with the spinner still, and a tab with an agent says `waiting · seen 2 min ago`. Folding works on them; they can't be dragged and have no `+ Tab`, and a machine's tabs have no buttons.
- **Rename, close or remove a worktree there from its row.** Hovering a machine's workspace shows `✕` and `⋯`. Rename is the same field in the row. `✕` always asks, naming the machine, and for a worktree shows what that machine found: changes not committed, a lock, its teardown. The row says `removing…` until it is gone. With the machine down nothing is sent, and a toast says so. These go through your own `ssh` from this computer, so a window that is itself on another server shows the rows and says plainly that it can't act on them.
- **Its agents' notices reach you here**: the same toast, desktop notification and sound, `claude on vps needs you in shop › notes`, and **Go** picks that workspace.
- **A click picks a machine's workspace or tab.** For now the terminal's place shows a card with where it runs, what its agent is doing and its folder, and what you type there is not sent: drawing its tab here comes next. Clicking anything of yours goes back, and an agent of yours behind the card still tells you when it needs you.
- With projects and workspaces side by side, the projects' column takes its share of the width again; it had shrunk to its longest name.

## 0.1.14

- The window now says what only cornercase's own window can do for now, where you would look for it, and which cornercase issue it waits on: pairing a phone's browser or a Telegram or Discord chat (Settings › Remote), removing the worktrees of closed projects and deleting branches (the Worktrees sheet and the Remove questions), **Run setup** (a worktree's menu), importing a VS Code workspace (the `+` beside Projects) and a plugin's window processes (its card in Plugins).

## 0.1.13

- **Select lines and characters in the Files viewer and the Changes diff.** A click selects a line, a drag or ⇧-click a range of lines (a drag on the numbers keeps to whole lines), a drag on the text selects characters and a double click a word. Open, Ask agent and Copy float beside the selection: ask the workspace's agent about `path:12-30`, copy the text, or open it at that line. In Files, Copy keeps the file's own tabs and a long line's hidden end; in a diff, tabs may come as spaces until cornercase gives the window a line's own text. ⌘C (Ctrl+Shift+C on Linux and Windows) copies the selection too, and Esc clears it.
- **Find in the open file.** ⌘F (Ctrl+Shift+F on Linux and Windows) while the Files viewer has the focus, or its Find button, opens a field over the file: every match lights up, with a count such as `3 of 12`, and Enter and Shift+Enter move between them. A big file stays smooth. Elsewhere ⌘F still opens Search. Opening a result of a text search opens the field with your search in it, so you can go on through the file.
- After Ask agent the keys go to the agent's pane, so you can type your question right away.

## 0.1.12

- **Plugins reach the menus and the rail.** What a plugin offers for a project, a workspace, a tab or a pane shows at the bottom of that row's `⋯` and right-click menu, and of a pane's right-click menu, under the plugin's name. A plugin with a button gets a slot in the rail, after the docked panels: with one action a click runs it, with several it opens a menu of them. How it ended comes back as a toast and a popup opens over the terminal, as from the Plugins sheet. For now a button acts on what cornercase's own window shows and a popup needs that window open (usecornercase/cornercase-terminal#666); the slot's tooltip and the sheet say so.
- A plugin's action that fails now says how (`exit status: 3`) and points at its log, where it only said `cornercase plugin: failed`. Closing a plugin's popup with its `✕` no longer shows an error.

## 0.1.11

- **Changes filters by path.** A field over the file list keeps the files you name: a piece of a path (`order`), a pattern as in `.gitignore` (`*.test.js`, `src/api/**`), `!` to leave files out (`!*.lock`). The summary says how many are left (`3 of 31 files`); Enter gives your keys back to the pane, Escape clears it.
- **Open in…** on a project, a workspace or a pane takes its folder to your file manager, VS Code, Cursor, Zed or Sublime Text, whichever you have. A pane opens the folder its program is in.
- **A project's menu reaches more of the project.** Project settings… opens Settings on it, where a repository can now change the folders Free builds deletes, and Worktrees… lists that project's alone, with a button for every worktree.
- **Import only what you want.** Each section of a shared setup has a tick in the preview; what you untick stays as it is.
- **Processes closes a pane.** It asks first when a program runs there, and an agent's pane goes through Close agent.
- **Ports stops several at once.** Tick the ones to stop, or use Stop all for a workspace's; it asks once, naming each program.
- **A file of a run opens beside the Flow panel.** In the Flow panel a click shows the file in the Files panel, and ‹ Flow goes back, scrolled where you left it.
- **Help.** The rail's `?` now has the docs and the bug report beside the tours, and can reset the tours' progress.
- **The Discord forum's state in Settings › Remote**: ready, or the step it waits for. Choosing among several servers still takes cornercase's own window.

## 0.1.10

- **Usage of every machine.** Once you have machines, the Usage sheet has a row that picks this computer, one machine or all of them, for Limits, Stats and By agent alike.
- Under all, Stats adds the machines up and says that **most at once** is the largest peak of one machine, not a sum.
- Under all, Limits lists each machine's accounts under its name. A Claude Code account logged in on two of them shows once, with `on this computer, vps` under it, and its numbers are the latest reading, never added.
- Old numbers say so: a machine that is down shows the numbers it gave last with `seen 5 min ago`, one whose last answer failed says why and how old they are, and one that never answered says why instead of its rows.

## 0.1.9

- Once an agent has more than one login, the window says which account each agent runs on: after its model in the sidebar and the tab bar, and on a small button at the bottom of its pane. That button, or Switch account in a tab's or a pane's menu, moves the agent to another account in the same conversation, after its turn if it is busy, and tells you what happened. On a shell, the buttons say which account an agent typed there would start on. If you never added an account, nothing changes.

## 0.1.8

- **Search finds settings.** Type in ⌘F and the settings that match are listed after your projects, workspaces, agents and tabs, so those keep their places. Each shows where it lives and its value, such as `UI › sound  [x] on`. Click one or press Enter and Settings opens on that row, scrolled to it and marked for a moment, with nothing changed. A row that doesn't show yet is listed muted with what it needs. Results about accounts, machines, flows and plugins open their own sheets.
- Settings' own search now also finds a setting by the value it shows and by its page, such as `hidden` or `ui sound`.
- Typing while a sheet with no text field is open, such as Accounts, no longer types into the terminal behind it. Keys also keep working after a sheet opens from another one's text field: Escape used to do nothing until a click.

## 0.1.7

- Routines in the Queue: schedule an agent or a flow to start again and again, every so many minutes, hours, days or weeks, every day, every weekday or on a day of the week. Each run gets a fresh worktree from the up-to-date default branch and can remove its workspace when it finishes, if nothing would be lost. A routine shows its next time and its last runs, with why a run failed or kept its workspace, and you can pause it, resume it, start it now, edit it, or remove it with its runs and undo that. It only runs while cornercase's server runs, and only git repositories can have routines.

## 0.1.6

- **See what fills an agent's context.** Right-click a tab running an agent, or an agent's row in the sidebar, and pick **View context**. For Claude Code a dialog shows how full its window is and how that splits between the system prompt, the tools, MCP servers, memory files such as a long `AGENTS.md`, skills and the conversation, with each memory file and MCP server, the skill count and the tools that took the most. Nothing is typed into your agent and no tokens are spent, though its MCP servers start a second time while it reads; **Refresh** reads again. The other agents show their model and how full their context is.

## 0.1.5

- **Machines, a new sheet** (⇧⌘N; Ctrl+Alt+Shift+N on Linux and Windows): the other computers your agents can run on, which cornercase links to through your own `ssh`. It shows this computer and each machine with its link in words, live: `connecting…`, `linked · 0.13.1`, `unreachable · seen 2 min ago` with what `ssh` said under it, or `older` with **Update…**. **Add machine** opens a form that says at once what it would refuse and offers the hosts of your `~/.ssh/config`. Each machine can be edited, its link tried again, or removed, which leaves its server running there. To move the whole window to another server, ⇧⌘K still does that.
- Escape and the shortcuts keep working in a sheet after you answer a question over it; they used to stop until the next click.

## 0.1.4

- **See what would conflict before you merge.** Merge check, in a workspace's menu, says whether its work, committed or not, merges cleanly into its base branch and with the workspaces it shares files with, and which files would conflict and how. It works in a scratch copy, so nothing in your repository changes, and Ask agent hands the conflicts to the workspace's agent.
- **See when two workspaces change the same files.** Turn on Settings › Worktrees › shared files and both rows show `⇄` and how many files they share, and those files carry a `⇄` in the Changes panel. Rest the pointer on a mark to see which files and who else changes them; a click on a row's mark opens the merge check.

## 0.1.3

- Editing a flow in the Flows sheet: a prompt or the notes now take several lines, and a long one scrolls inside its field.
- Stages move up and down with ↑ and ↓, and moving or removing one keeps every loop pointing at the same stage. A loop that would no longer go back to an earlier stage is dropped, and a message says which.
- Removing a role also removes a stage it leaves empty.
- A role's and the orchestrator's arguments and variables can be edited in the form.
- The form ends with the plan of what you are editing, redrawn as you change it.
- A click past the end of a token typed in Settings, followed by a key, no longer closes the window.

## 0.1.2

- **Recap: an agent sums up what your agents did**, in every project, in a panel beside the terminal: the new icon on the rail, or ⇧⌘H (Ctrl+Alt+Shift+H on Linux and Windows). **Now** says where things stand and what waits on you first, **Yesterday** is for your daily (the last day you worked with agents, so on Monday it is Friday) and **7 days** goes through the week, project by project. Every conversation and pull request it names is a link: to the agent's pane, ready for you to answer, or to the pull request on the web. An agent writes it in the background, which can take a minute or more, so you can close the panel meanwhile; ↻ writes it again. Settings › Agents › Recap chooses the agent, its model and the language.
- When the window is too short for every icon on the rail, the panels' icons scroll, so Settings always shows.

## 0.1.1

- The window now needs cornercase 0.13.1 or newer on every platform, the release that brings machines and routines; run `cornercase update` if yours is older, and the window says so when it is.

## 0.1.0

- The window tells you when a new version of it is out and shows what changed; on Windows and Linux it installs it and opens again on the tab you were on.
- First release: a native window for cornercase, drawn on the GPU with GPUI. Downloads for macOS (universal, 12 or later), Windows (x64) and Linux (x86_64). The macOS one is signed and notarized by Apple, so macOS only asks once whether to open an app downloaded from the internet; the Windows and Linux ones are not signed yet. It needs cornercase 0.12.234 or newer, 0.12.287 or newer on Windows.
