# Files panel

Code: `panels/files.rs`, `panels/files/`

The workspace's files: a tree, a viewer and a search. It follows the TUI's panel (cornercase's `design/files-panel.md`).

## Reading the disk

- **The panel reads the disk itself**, the one exception to going through the command line (#345 upstream, `design/upstream-gaps.md`): a file viewer through `cornercase` would copy every file through a process.
- **Nothing blocks the window**: listings, reads, highlighting and searches run off the main thread, and each answer is dropped when what it answers is no longer shown (another workspace, another file, a newer query).
- **Polling, not watching**: while the panel shows, the open file's size and modification time are checked every second (`CHECK_EVERY`) and the open folders are listed again every second check (`LIST_EVERY`), so what an agent writes appears. A closed panel polls nothing.

## Per workspace

- The panel follows the shown workspace and keeps, per workspace and in memory, the open folders, the open file (marked in the tree once closed) and the query. Coming back to a workspace brings its query back; a workspace whose folder changed starts afresh.
- A folder that went away closes; a root that cannot be listed shows why.
- Closing the viewer scrolls the tree to the file's row.

## The tree (`tree.rs`)

- **Each open folder is listed on its own, one level deep**, then flattened into rows. A huge folder (a home folder opened as a project) costs nothing until it is opened. At most `MAX_ENTRIES` per folder; the panel says when some were left out.
- The `ignore` rules apply (`.gitignore`, `.ignore`, the global and `.git/info/exclude` ones, also from parent folders, without needing a git repository). `.git` is never listed; hidden files are, and the "Hidden" toggle hides them.
- A link to a folder opens like a folder. A folder that is gone fails as `read_dir` would rather than listing nothing.
- Opening a file from a search opens every folder above it (`reveal`), so it shows in the tree.

## The viewer (`disk.rs`, `highlight.rs`)

- **Only regular files are read**: a link to `/dev/zero` or a FIFO would never end.
- The first 16 bytes say whether a file is an image (PNG, JPEG, GIF, WebP, BMP, TIFF; SVG by extension), which may be up to 32 MB; text stops at 8 MB. The read stops one byte past the cap, so a file that grew since the size check cannot get past it.
- A NUL in the first 8,000 bytes makes a file binary. Invalid UTF-8 is replaced, not refused. Tabs are drawn as four spaces, `\r` is dropped and a final newline ends the last line rather than starting an empty one.
- **Colours never flash**: the first read shows plain text at once and the colours after; a re-read after a change answers only once coloured.
- A selected line past the end (the file shrank) selects nothing.
- **Highlighting uses syntect's default grammars**, with scopes mapped to a few kinds drawn in the palette's colours as the TUI does: keyword magenta, function blue, type cyan, string green, number and constant yellow, property and tag red, comment grey in italics. The grammar is found by file name (`Makefile`), then extension, then first line (a shebang); extensions the default set lacks borrow a close relative (TypeScript as JavaScript, TOML as INI, Swift and Kotlin as Java, Vue and Svelte as HTML). Lines are parsed in order so the state carries across them (block comments, strings); a grammar that fails leaves the rest plain.
- Sources over 1 MB stay plain: syntect's pure-Rust regexes are slow on big files.
- A text search's query stays lit in the opened file.

## The search (`search.rs`)

- **One index of paths, reused**: a single `ignore` walk (hidden files included, ignored ones and `.git` not), at most 200,000 paths, reused by new queries for 10 seconds (`INDEX_FRESH`).
- **File names**: nucleo's matcher with fzf's scoring and smart case; on a tie the shorter file name, then the shorter path, so the file a query names comes first. At most 200 results.
- **Text**: ripgrep's searcher, the query a literal with smart case, binary files skipped, on up to 8 threads. At most 1,000 matches: each match takes a slot before it is kept, so the threads together never keep more than the cap. A long line is cut to 400 characters, starting 40 before its first match.
- Enter opens the first result; pressed before the answer came, it opens it when it does.

## Opening elsewhere (`open.rs`)

- **cornercase leaves opening in other apps to each window on purpose** (its `design/opening-in-other-apps.md`: "No request"): only the window knows the apps of the machine it runs on. Detection and command lines follow that design.
- Openers, in a fixed order: Finder, then VS Code, Cursor, Zed and Sublime Text (their command on the `PATH`, which takes `FILE:LINE`, else their bundle in an Applications folder through `open -a`, which takes no line), then the default app. VS Code and Cursor need `-g` for the line.
- Finder reveals a file and opens a folder, but reveals bundle-like folders (`.app`, `.xcodeproj`…), which `open` would launch.
- The default app takes plain files only, never folders or programs; the menu checks this with one `stat` per entry.
