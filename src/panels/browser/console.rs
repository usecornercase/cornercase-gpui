use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Write as _;
use std::rc::Rc;

use serde_json::Value;

mod objects;

pub(crate) use objects::{first_line, shown};

const KEPT: usize = 500;
const REQUESTS: usize = 2000;
const FRAMES: usize = 20;
const TEXT: usize = 2000;
const URL: usize = 300;
const HANDED: usize = 20;
const HANDED_TEXT: usize = 600;
const HANDED_FRAMES: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Level {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Filter {
    Errors,
    Warnings,
    #[default]
    All,
}

impl Filter {
    pub(crate) fn shows(self, level: Level) -> bool {
        match self {
            Self::Errors => level == Level::Error,
            Self::Warnings => level == Level::Warning,
            Self::All => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entry {
    pub(crate) level: Level,
    pub(crate) text: String,
    pub(crate) source: Option<String>,
    pub(crate) stack: Vec<String>,
}

impl Entry {
    fn new(level: Level, text: &str) -> Self {
        Self { level, text: cut(text, TEXT), source: None, stack: Vec::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Logged {
    pub(crate) id: u64,
    pub(crate) entry: Rc<Entry>,
    pub(crate) times: u32,
}

struct Request {
    method: String,
    url: String,
}

#[derive(Default)]
pub(crate) struct Log {
    entries: VecDeque<Logged>,
    next: u64,
    errors: usize,
    warnings: usize,
    requests: HashMap<String, Request>,
    reported: HashSet<String>,
    document: Option<String>,
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

impl Log {
    pub(crate) fn apply(&mut self, method: &str, params: &Value, main_frame: &str) {
        match method {
            "Runtime.consoleAPICalled" if text(params, "type") == Some("clear") => self.clear(),
            "Runtime.consoleAPICalled" => self.add(console_entry(params)),
            "Runtime.exceptionThrown" => self.add(Some(exception_entry(&params["exceptionDetails"]))),
            "Log.entryAdded" => self.log_entry(&params["entry"]),
            "Network.requestWillBeSent" => self.request(params, main_frame),
            "Network.responseReceived" => self.response(params),
            "Network.loadingFailed" => self.failed(params),
            "Network.loadingFinished" => self.forget(params),
            "Page.frameNavigated" => self.navigated(&params["frame"]),
            _ => {}
        }
    }

    pub(crate) fn entries(&self) -> impl DoubleEndedIterator<Item = &Logged> {
        self.entries.iter()
    }

    pub(crate) fn errors(&self) -> usize {
        self.errors
    }

    pub(crate) fn warnings(&self) -> usize {
        self.warnings
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.iter().map(|l| l.times as usize).sum()
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.reported.clear();
        (self.errors, self.warnings) = (0, 0);
    }

    fn count(&mut self, level: Level, times: u32, add: bool) {
        let counter = match level {
            Level::Error => &mut self.errors,
            Level::Warning => &mut self.warnings,
            Level::Info => return,
        };
        *counter = if add { *counter + times as usize } else { counter.saturating_sub(times as usize) };
    }

    fn add(&mut self, entry: Option<Entry>) {
        let Some(entry) = entry else { return };
        self.count(entry.level, 1, true);
        if let Some(last) = self.entries.back_mut().filter(|l| *l.entry == entry) {
            last.times += 1;
            return;
        }
        if self.entries.len() >= KEPT
            && let Some(old) = self.entries.pop_front()
        {
            self.count(old.entry.level, old.times, false);
        }
        self.next += 1;
        self.entries.push_back(Logged { id: self.next, entry: Rc::new(entry), times: 1 });
    }

    fn navigated(&mut self, frame: &Value) {
        let loader = text(frame, "loaderId").map(str::to_string);
        if frame.get("parentId").is_none() && loader.is_some() && loader != self.document {
            self.clear();
            self.document = loader;
        }
    }

    fn request(&mut self, p: &Value, main_frame: &str) {
        if starts_document(p, main_frame) {
            self.clear();
            self.document = text(p, "loaderId").map(str::to_string);
        }
        if self.requests.len() >= REQUESTS {
            self.requests.clear();
        }
        let request = &p["request"];
        if let (Some(id), Some(method), Some(url)) =
            (text(p, "requestId"), text(request, "method"), text(request, "url"))
        {
            self.requests.insert(id.to_string(), Request { method: method.to_string(), url: short_url(url) });
        }
    }

    fn forget(&mut self, p: &Value) {
        if let Some(id) = text(p, "requestId") {
            self.requests.remove(id);
        }
    }

    fn report(&mut self, id: &str, entry: Entry) {
        if self.reported.len() >= REQUESTS {
            self.reported.clear();
        }
        self.reported.insert(id.to_string());
        self.add(Some(entry));
    }

    fn response(&mut self, p: &Value) {
        let response = &p["response"];
        let status = response["status"].as_u64().unwrap_or(0);
        let (Some(id), Some(url)) = (text(p, "requestId"), text(response, "url")) else { return };
        if status < 400 || is_favicon(url) {
            return;
        }
        let method = self.requests.get(id).map_or("GET", |r| r.method.as_str());
        let reason = text(response, "statusText").unwrap_or_default();
        let line = format!("{method} {} {status} {reason}", short_url(url));
        self.report(id, Entry::new(Level::Error, line.trim_end()));
    }

    fn failed(&mut self, p: &Value) {
        let Some(id) = text(p, "requestId") else { return };
        let request = self.requests.remove(id);
        let canceled = p["canceled"].as_bool().unwrap_or(false);
        let Some(request) = request.filter(|r| !canceled && !is_favicon(&r.url)) else { return };
        let error = text(p, "errorText").unwrap_or("failed");
        let mut line = format!("{} {} {error}", request.method, request.url);
        if let Some(cors) = p["corsErrorStatus"]["corsError"].as_str() {
            let _ = write!(line, " (CORS: {cors})");
        }
        self.report(id, Entry::new(Level::Error, &line));
    }

    fn log_entry(&mut self, e: &Value) {
        let network = text(e, "source") == Some("network");
        let known = text(e, "networkRequestId").is_some_and(|id| self.reported.contains(id));
        let favicon = text(e, "url").is_some_and(is_favicon);
        if network && (known || favicon) {
            return;
        }
        self.add(log_entry(e));
    }
}

fn starts_document(p: &Value, main_frame: &str) -> bool {
    text(p, "type") == Some("Document")
        && text(p, "frameId") == Some(main_frame)
        && text(p, "requestId").is_some()
        && text(p, "requestId") == text(p, "loaderId")
}

fn is_favicon(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    path.ends_with("/favicon.ico")
}

pub(crate) fn cut(text: &str, at: usize) -> String {
    match text.char_indices().nth(at) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_string(),
    }
}

pub(crate) fn short_url(url: &str) -> String {
    match url.split_once(',') {
        Some((head, _)) if url.starts_with("data:") => format!("{head},…"),
        _ => cut(url, URL),
    }
}

pub(crate) fn location(url: &str, line: Option<u64>, column: Option<u64>) -> Option<String> {
    if url.is_empty() {
        return None;
    }
    let url = short_url(url);
    Some(match (line, column) {
        (Some(line), Some(column)) => format!("{url}:{}:{}", line + 1, column + 1),
        (Some(line), None) => format!("{url}:{}", line + 1),
        _ => url,
    })
}

struct Frame {
    function: String,
    at: String,
}

fn frames(trace: &Value) -> Vec<Frame> {
    let calls = trace["callFrames"].as_array().map(Vec::as_slice).unwrap_or_default();
    calls
        .iter()
        .filter_map(|f| {
            let at = location(text(f, "url")?, f["lineNumber"].as_u64(), f["columnNumber"].as_u64())?;
            Some(Frame { function: text(f, "functionName").unwrap_or_default().to_string(), at })
        })
        .take(FRAMES)
        .collect()
}

fn stack_lines(frames: &[Frame]) -> Vec<String> {
    frames
        .iter()
        .map(|f| if f.function.is_empty() { format!("at {}", f.at) } else { format!("at {} ({})", f.function, f.at) })
        .collect()
}

fn console_level(kind: &str) -> Option<Level> {
    match kind {
        "error" | "assert" => Some(Level::Error),
        "warning" => Some(Level::Warning),
        "endGroup" | "profile" | "profileEnd" => None,
        _ => Some(Level::Info),
    }
}

pub(crate) fn console_entry(p: &Value) -> Option<Entry> {
    let kind = text(p, "type").unwrap_or("log");
    let level = console_level(kind)?;
    let args = p["args"].as_array().map(Vec::as_slice).unwrap_or_default();
    let mut said = objects::formatted(args);
    if kind == "assert" {
        said = if said.is_empty() { "Assertion failed".to_string() } else { format!("Assertion failed: {said}") };
    }
    let frames = frames(&p["stackTrace"]);
    let mut entry = Entry::new(level, &said);
    entry.source = frames.first().map(|f| f.at.clone());
    if level != Level::Info || kind == "trace" {
        entry.stack = stack_lines(&frames);
    }
    Some(entry)
}

pub(crate) fn exception_entry(d: &Value) -> Entry {
    let said = text(d, "text").unwrap_or("Uncaught");
    let exception = &d["exception"];
    let detail = if exception.is_object() { first_line(&shown(exception)) } else { String::new() };
    let message =
        if detail.is_empty() || said.contains(&detail) { said.to_string() } else { format!("{said} {detail}") };
    let frames = frames(&d["stackTrace"]);
    let mut entry = Entry::new(Level::Error, &message);
    entry.source = frames
        .first()
        .map(|f| f.at.clone())
        .or_else(|| location(text(d, "url").unwrap_or_default(), d["lineNumber"].as_u64(), d["columnNumber"].as_u64()));
    entry.stack = stack_lines(&frames);
    entry
}

pub(crate) fn log_entry(e: &Value) -> Option<Entry> {
    let level = match text(e, "level")? {
        "error" => Level::Error,
        "warning" => Level::Warning,
        "info" => Level::Info,
        _ => return None,
    };
    let mut entry = Entry::new(level, text(e, "text")?);
    entry.source = location(text(e, "url").unwrap_or_default(), e["lineNumber"].as_u64(), None);
    entry.stack = stack_lines(&frames(&e["stackTrace"]));
    Some(entry)
}

fn noun(level: Level, many: bool) -> &'static str {
    match (level, many) {
        (Level::Error, false) => "error",
        (Level::Error, true) => "errors",
        (Level::Warning, false) => "warning",
        (Level::Warning, true) => "warnings",
        (Level::Info, false) => "message",
        (Level::Info, true) => "messages",
    }
}

fn headline(url: &str, entries: &[&Logged]) -> String {
    let level = entries.first().map_or(Level::Info, |l| l.entry.level);
    let alike = entries.iter().all(|l| l.entry.level == level);
    let what = match entries.len() {
        1 => format!("this {}", noun(level, false)),
        n if alike => format!("{n} {}", noun(level, true)),
        n => format!("{n} entries"),
    };
    format!("The browser's console at {url} shows {what}:")
}

fn handed(n: usize, logged: &Logged) -> String {
    let entry = &logged.entry;
    let times = if logged.times > 1 { format!(" (×{})", logged.times) } else { String::new() };
    let mut out = format!("{n}. {}{times}", cut(&entry.text, HANDED_TEXT));
    let at = if entry.stack.is_empty() {
        entry.source.iter().map(|s| format!("at {s}")).collect()
    } else {
        entry.stack.clone()
    };
    for line in at.iter().take(HANDED_FRAMES) {
        let _ = write!(out, "\n   {line}");
    }
    if at.len() > HANDED_FRAMES {
        out.push_str("\n   …");
    }
    out
}

pub(crate) fn hand_off(url: &str, entries: &[&Logged]) -> String {
    let kept = &entries[entries.len().saturating_sub(HANDED)..];
    let mut lines = vec![headline(url, entries)];
    lines.extend(kept.iter().enumerate().map(|(n, logged)| handed(n + 1, logged)));
    if entries.len() > kept.len() {
        lines.push(format!("(and {} earlier, left out)", entries.len() - kept.len()));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests;
