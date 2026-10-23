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
        let path = path.strip_prefix(root).expect("source is under the root").to_string_lossy().into_owned();
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

#[test]
fn files_stay_within_the_line_limit() {
    check("over 1,000 lines", LONG_FILES, |s| s.text.lines().count() > MAX_LINES);
}

#[test]
fn sources_have_no_comments() {
    let lines = |s: &Source| s.comments.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ");
    for source in sources().iter().filter(|s| !s.comments.is_empty()) {
        if !COMMENTED_FILES.contains(&source.path.as_str()) {
            panic!("comments in {} (lines {}): rationale goes in design/", source.path, lines(source));
        }
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
