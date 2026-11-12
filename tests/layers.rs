use std::{
    fs,
    path::{Path, PathBuf},
};

const MAX_LINES: usize = 1000;
const WINDOW: &[&str] = &["app", "context_menu", "sidebar", "tabs", "terminal"];

const LONG_FILES: &[&str] = &[];
const COMMENTED_FILES: &[&str] = &[];
const INLINE_TESTS: &[&str] = &[];
const APP_EXCEPTIONS: &[&str] = &[];

struct Source {
    path: String,
    module: String,
    text: String,
    code: String,
    comments: Vec<usize>,
}

impl Source {
    fn read(root: &Path, path: &Path) -> Self {
        let text = fs::read_to_string(path).expect("source is readable");
        let relative = path.strip_prefix(root).expect("source is under the root");
        let parts: Vec<String> =
            relative.components().map(|part| part.as_os_str().to_string_lossy().into_owned()).collect();
        let path = parts.join("/");
        let (code, comments) = Lexer::new(&text).split();
        Self { module: module_of(&path), path, text, code, comments }
    }

    fn in_window(&self) -> bool {
        self.path.starts_with("src/") && WINDOW.contains(&self.module.as_str())
    }
}

fn module_of(path: &str) -> String {
    let inner = path.strip_prefix("src/").unwrap_or(path);
    inner.split(['/', '.']).next().unwrap_or_default().to_owned()
}

struct Lexer<'a> {
    chars: Vec<char>,
    at: usize,
    line: usize,
    code: String,
    comments: Vec<usize>,
    text: &'a str,
}

impl<'a> Lexer<'a> {
    fn new(text: &'a str) -> Self {
        Self { chars: text.chars().collect(), at: 0, line: 1, code: String::new(), comments: Vec::new(), text }
    }

    fn split(mut self) -> (String, Vec<usize>) {
        assert!(!self.text.contains('\r'), "sources use \\n line endings");
        while self.at < self.chars.len() {
            self.step();
        }
        (self.code, self.comments)
    }

    fn peek(&self, offset: usize) -> Option<char> {
        self.chars.get(self.at + offset).copied()
    }

    fn take(&mut self, keep: bool) -> char {
        let c = self.chars[self.at];
        self.at += 1;
        if c == '\n' {
            self.line += 1;
            self.code.push('\n');
        } else if keep {
            self.code.push(c);
        }
        c
    }

    fn step(&mut self) {
        match (self.peek(0), self.peek(1)) {
            (Some('/'), Some('/')) => self.line_comment(),
            (Some('/'), Some('*')) => self.block_comment(),
            (Some('"'), _) => self.string(),
            (Some('r'), Some('"' | '#')) if !self.ident_before() => self.raw_string(),
            (Some('\''), _) => self.quote(),
            _ => {
                self.take(true);
            }
        }
    }

    fn ident_before(&self) -> bool {
        self.at > 0 && (self.chars[self.at - 1].is_alphanumeric() || self.chars[self.at - 1] == '_')
    }

    fn line_comment(&mut self) {
        self.comments.push(self.line);
        while self.peek(0).is_some_and(|c| c != '\n') {
            self.take(false);
        }
    }

    fn block_comment(&mut self) {
        self.comments.push(self.line);
        let mut depth = 0;
        loop {
            match (self.peek(0), self.peek(1)) {
                (Some('/'), Some('*')) => depth += 1,
                (Some('*'), Some('/')) => depth -= 1,
                (None, _) => return,
                _ => {
                    self.take(false);
                    continue;
                }
            }
            self.take(false);
            self.take(false);
            if depth == 0 {
                return;
            }
        }
    }

    fn string(&mut self) {
        self.take(true);
        while let Some(c) = self.peek(0) {
            self.take(false);
            if c == '\\' {
                self.take(false);
            } else if c == '"' {
                self.code.push('"');
                return;
            }
        }
    }

    fn raw_string(&mut self) {
        self.take(true);
        let mut hashes = 0;
        while self.peek(0) == Some('#') {
            hashes += 1;
            self.take(true);
        }
        if self.peek(0) != Some('"') {
            return;
        }
        self.take(true);
        let end: String = std::iter::once('"').chain(std::iter::repeat_n('#', hashes)).collect();
        while self.at < self.chars.len() && !self.chars[self.at..].iter().collect::<String>().starts_with(&end) {
            self.take(false);
        }
        for _ in 0..end.len().min(self.chars.len() - self.at) {
            self.take(true);
        }
    }

    fn quote(&mut self) {
        self.take(true);
        let char_literal = self.peek(0) == Some('\\') || self.peek(1) == Some('\'');
        if !char_literal {
            return;
        }
        while let Some(c) = self.peek(0) {
            self.take(false);
            if c == '\\' {
                self.take(false);
            } else if c == '\'' {
                self.code.push('\'');
                return;
            }
        }
    }
}

fn sources() -> Vec<Source> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut paths = Vec::new();
    for folder in ["src", "tests"] {
        collect(&root.join(folder), &mut paths);
    }
    paths.sort();
    paths.iter().map(|path| Source::read(root, path)).collect()
}

fn collect(dir: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("folder is readable") {
        let path = entry.expect("entry is readable").path();
        if path.is_dir() {
            collect(&path, paths);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            paths.push(path);
        }
    }
}

fn check(name: &str, exceptions: &[&str], breaks: impl Fn(&Source) -> bool) {
    let sources = sources();
    let found: Vec<&str> = sources.iter().filter(|s| breaks(s)).map(|s| s.path.as_str()).collect();
    let new: Vec<&&str> = found.iter().filter(|path| !exceptions.contains(path)).collect();
    let stale: Vec<&&str> = exceptions.iter().filter(|path| !found.contains(path)).collect();
    assert!(new.is_empty(), "{name}: {new:?}");
    assert!(stale.is_empty(), "{name}: remove these fixed exceptions: {stale:?}");
}

fn uses_app(source: &Source) -> bool {
    let code = source.code.replace(char::is_whitespace, "");
    code.contains("crate::app::") || code.contains("crate::app}") || code.contains("crate::app;")
}

fn relative_fixture(source: &Source) -> bool {
    source.path != "src/test_util.rs" && source.code.contains("include_str!(\"")
}

fn makes_a_colour(source: &Source) -> bool {
    let themed = source.path == "src/theme.rs" || source.path.starts_with("src/theme/");
    let tests = source.path.starts_with("tests/") || is_test_file(&source.path);
    if themed || tests {
        return false;
    }
    ["rgb(", "rgba("].iter().any(|call| {
        source.code.match_indices(call).any(|(at, _)| {
            let before = source.code[..at].chars().next_back();
            !before.is_some_and(|c| c.is_alphanumeric() || c == '_') || source.code[..at].ends_with("gpui::")
        })
    })
}

#[test]
fn colours_come_from_the_theme() {
    check("a colour made outside the theme", &[], makes_a_colour);
}

#[test]
fn files_stay_within_the_line_limit() {
    check("over 1,000 lines", LONG_FILES, |s| s.text.lines().count() > MAX_LINES);
}

#[test]
fn sources_have_no_comments() {
    let lines = |s: &Source| s.comments.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ");
    for source in sources().iter().filter(|s| !s.comments.is_empty()) {
        assert!(
            COMMENTED_FILES.contains(&source.path.as_str()),
            "comments in {} (lines {}): rationale goes in design/",
            source.path,
            lines(source)
        );
    }
    check("comments", COMMENTED_FILES, |s| !s.comments.is_empty());
}

#[test]
fn tests_live_in_a_file_of_their_own() {
    check("inline `mod tests {`", INLINE_TESTS, |s| s.code.replace(char::is_whitespace, "").contains("modtests{"));
}

#[test]
fn only_the_window_uses_app() {
    check("uses crate::app outside the window", APP_EXCEPTIONS, |s| !s.in_window() && uses_app(s));
}

#[test]
fn only_the_window_implements_root() {
    check("`impl Root` outside the window", &[], |s| !s.in_window() && s.code.contains("impl Root"));
}

struct Drawing {
    what: &'static str,
    files: &'static [&'static str],
    root: &'static [&'static str],
    side: &'static [&'static str],
    parts: &'static [&'static str],
}

const DRAWINGS: &[Drawing] = &[
    Drawing {
        what: "the sidebar",
        files: &[
            "src/sidebar.rs",
            "src/sidebar/arrange.rs",
            "src/sidebar/rows.rs",
            "src/sidebar/workspace.rs",
            "src/sidebar/tab_rows.rs",
            "src/sidebar/agents.rs",
            "src/sidebar/machine_rows.rs",
            "src/sidebar/machine_agents.rs",
            "src/sidebar/key.rs",
        ],
        root: &["session", "side", "store", "sidebar_width", "pair_width", "parts"],
        side: &["hovered", "rename", "there_folds", "look", "sizes", "machine_place"],
        parts: &["spots", "counted", "store_seen", "sidebar", "orbit_layer"],
    },
    Drawing {
        what: "the rail",
        files: &["src/sidebar/rail.rs", "src/sidebar/rail/key.rs"],
        root: &["session", "side", "store", "parts"],
        side: &["look", "menu", "machine_place"],
        parts: &["rail", "store_seen"],
    },
    Drawing {
        what: "the tab bar",
        files: &["src/tabs.rs", "src/tabs/key.rs"],
        root: &["session", "side", "store", "machine", "focus", "parts"],
        side: &["look", "strip", "rename", "machine_place"],
        parts: &["tabs", "tab_orbits", "tab_spots", "store_seen"],
    },
    Drawing {
        what: "the status bar",
        files: &["src/app/status.rs", "src/app/status/key.rs"],
        root: &["notifier", "store", "conn", "came_up", "host", "session", "machine", "parts", "side"],
        side: &["menu"],
        parts: &["status", "store_seen", "chips_seen"],
    },
];

struct Cached {
    kind: &'static str,
    files: &'static [&'static str],
    except: &'static [&'static str],
    clock: &'static [&'static str],
    badges: &'static [&'static str],
}

const CACHED: &[Cached] = &[
    Cached {
        kind: "Changes",
        files: &["src/panels/changes.rs", "src/panels/changes/", "src/panels/on_machine.rs"],
        except: &[],
        clock: &["src/panels/changes/follow.rs", "src/panels/on_machine.rs"],
        badges: &[],
    },
    Cached {
        kind: "Files",
        files: &["src/panels/files.rs", "src/panels/files/", "src/panels/on_machine.rs"],
        except: &[
            "src/panels/files/finder.rs",
            "src/panels/files/finder/",
            "src/panels/files/peek.rs",
            "src/panels/files/peek/",
        ],
        clock: &["src/panels/files/querying.rs", "src/panels/files/there.rs", "src/panels/on_machine.rs"],
        badges: &[],
    },
    Cached {
        kind: "Todo",
        files: &["src/panels/todo.rs", "src/panels/todo/"],
        except: &[],
        clock: &[],
        badges: &["src/panels/todo.rs"],
    },
    Cached {
        kind: "Recap",
        files: &["src/panels/recap.rs", "src/panels/recap/"],
        except: &[],
        clock: &["src/panels/recap.rs"],
        badges: &[],
    },
    Cached {
        kind: "Flow",
        files: &["src/panels/flows.rs", "src/panels/flows/"],
        except: &[
            "src/panels/flows/checked.rs",
            "src/panels/flows/dialog.rs",
            "src/panels/flows/draft.rs",
            "src/panels/flows/editor.rs",
            "src/panels/flows/editor/",
            "src/panels/flows/editor_view.rs",
            "src/panels/flows/import.rs",
            "src/panels/flows/import/",
            "src/panels/flows/list.rs",
            "src/panels/flows/start.rs",
            "src/panels/flows/start/",
        ],
        clock: &[
            "src/panels/flows/messages.rs",
            "src/panels/flows/diagram.rs",
            "src/panels/flows/merged_view.rs",
            "src/panels/flows/model.rs",
        ],
        badges: &[],
    },
];

fn reads_a_global_but_store(code: &str) -> bool {
    code.match_indices("::get(cx)").any(|(at, _)| !code[..at].ends_with("Store"))
}

fn is_test_file(path: &str) -> bool {
    path.ends_with("/tests.rs") || path.contains("/tests/")
}

fn listed(path: &str, names: &[&str]) -> bool {
    names.iter().any(|name| if name.ends_with('/') { path.starts_with(name) } else { path == *name })
}

fn covers(panel: &Cached, path: &str) -> bool {
    !is_test_file(path) && listed(path, panel.files) && !listed(path, panel.except)
}

fn reads_outside_itself(source: &Source, panel: &Cached) -> Option<&'static str> {
    let code = &source.code;
    let path = source.path.as_str();
    if reads_a_global_but_store(code) || code.contains("try_global") || code.contains("global::<") {
        return Some("a global but Store");
    }
    if code.contains("now()") && !panel.clock.contains(&path) {
        return Some("the wall clock");
    }
    if ["window.modifiers()", "window.mouse_position()", "capslock()"].iter().any(|call| code.contains(call)) {
        return Some("window state that redraws nothing");
    }
    if (code.contains("ui::badge(") || code.contains("motion::orbit(")) && !panel.badges.contains(&path) {
        return Some("an orbit turned at render");
    }
    code.contains("thread_local!").then_some("a thread-local")
}

#[test]
fn each_cached_panel_draws_from_itself_and_the_store() {
    for panel in CACHED {
        check(&format!("the {} panel reads outside itself", panel.kind), &[], |s| {
            covers(panel, &s.path) && reads_outside_itself(s, panel).is_some()
        });
    }
}

#[test]
fn the_dock_caches_the_panels_the_table_checks() {
    let all = sources();
    let panels = all.iter().find(|s| s.path == "src/panels/mod.rs").expect("the panels are in src/panels/mod.rs");
    let start = panels.code.find("fn cached(self) -> bool").expect("PanelKind::cached exists");
    let body = &panels.code[start..];
    let body = &body[..body.find("\n    }").expect("cached ends")];
    let mut kinds: Vec<&str> = words_after(body, "Self::").collect();
    let mut checked: Vec<&str> = CACHED.iter().map(|panel| panel.kind).collect();
    kinds.sort_unstable();
    checked.sort_unstable();
    assert_eq!(kinds, checked, "every cached panel is in CACHED, and every entry of CACHED is cached");
}

fn root_fields(sources: &[Source]) -> Vec<String> {
    let app = sources.iter().find(|s| s.path == "src/app.rs").expect("the window is in src/app.rs");
    let start = app.code.find("pub(crate) struct Root {").expect("Root is declared") + "pub(crate) struct Root {".len();
    let body = &app.code[start..];
    let body = &body[..body.find("\n}").expect("Root ends")];
    body.lines()
        .filter_map(|line| {
            line.trim().trim_start_matches("pub(crate) ").split_once(':').map(|(name, _)| name.trim().to_owned())
        })
        .collect()
}

fn words_after<'a>(code: &'a str, prefix: &str) -> impl Iterator<Item = &'a str> {
    code.match_indices(prefix).map(move |(at, _)| {
        let rest = &code[at + prefix.len()..];
        let end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(rest.len());
        &rest[..end]
    })
}

fn reads_beyond_its_key(source: &Source, fields: &[String]) -> Option<&'static str> {
    let drawing = DRAWINGS.iter().find(|d| d.files.contains(&source.path.as_str()))?;
    let outside = words_after(&source.code, "self.")
        .any(|word| fields.iter().any(|f| f == word) && !drawing.root.contains(&word));
    let side = words_after(&source.code, "self.side.").any(|word| !drawing.side.contains(&word));
    let parts = words_after(&source.code, "self.parts.").any(|word| !drawing.parts.contains(&word));
    let globals = reads_a_global_but_store(&source.code);
    (outside || side || parts || globals).then_some(drawing.what)
}

#[test]
fn each_part_draws_only_what_its_key_holds() {
    let all = sources();
    let fields = root_fields(&all);
    assert!(fields.iter().any(|f| f == "metrics"), "the field list is read from Root");
    for drawing in DRAWINGS {
        check(&format!("{} reads what its key leaves out", drawing.what), &[], |s| {
            reads_beyond_its_key(s, &fields) == Some(drawing.what)
        });
    }
}

#[test]
fn fixtures_are_read_with_the_fixture_macro() {
    check("relative include_str!", &[], relative_fixture);
}

#[test]
fn the_lexer_tells_comments_from_strings() {
    let text = "let a = \"// not\"; // yes\nlet b = r#\"/* no */\"#; /* yes */\nlet c = '\"'; fn f<'a>() {}\n";
    let (code, comments) = Lexer::new(text).split();
    assert_eq!((comments, code.contains("fn f<'a>")), (vec![1, 2], true));
}
